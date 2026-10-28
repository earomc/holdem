//! The server side: the table's thread, accepting players over TCP, and
//! answering LAN discovery.

use std::io::{self, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{Connection, Outgoing, Parsed, parse_message, read_line};
use crate::card::Rng;
use crate::protocol::{Announcement, ClientMsg, DISCOVERY_REQUEST, ServerMsg};
use crate::table::{ClientId, Table, TableSettings};

/// Longest line a server accepts from a client.
pub(super) const MAX_CLIENT_LINE: u64 = 16 * 1024;

pub(super) const TICK: Duration = Duration::from_millis(20);

/// What reaches the table thread.
pub(super) enum Inbound {
    Connected {
        id: ClientId,
        outbox: Sender<ServerMsg>,
        owner: bool,
    },
    Message(ClientId, ClientMsg),
    Disconnected(ClientId),
    /// A line the client sent was not JSON at all.
    Garbage(ClientId),
    /// A well-formed message of a kind this version does not know.
    NotUnderstood(ClientId),
}

/// A running table. Dropping it stops the table and closes every connection.
pub struct Server {
    inbound: Sender<Inbound>,
    stop: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
    threads: Vec<JoinHandle<()>>,
    /// The TCP port, when the table accepts network players.
    pub port: Option<u16>,
    announcement: Arc<Mutex<Option<Announcement>>>,
}

impl Server {
    /// Starts a table. With `port`, it also listens for network players on that
    /// port (0 picks a free one) and answers LAN discovery.
    pub fn start(settings: TableSettings, port: Option<u16>, seed: u64) -> io::Result<Server> {
        let (tx, rx) = mpsc::channel::<Inbound>();
        let stop = Arc::new(AtomicBool::new(false));
        let next_id = Arc::new(AtomicU64::new(1));
        let mut threads = Vec::new();
        let announcement = Arc::new(Mutex::new(None::<Announcement>));

        let mut bound_port = None;
        if let Some(port) = port {
            let listener = TcpListener::bind(("0.0.0.0", port))?;
            let port = listener.local_addr()?.port();
            bound_port = Some(port);
            listener.set_nonblocking(true)?;
            let (tx2, stop2, ids) = (tx.clone(), stop.clone(), next_id.clone());
            threads.push(thread::spawn(move || {
                accept_loop(listener, tx2, stop2, ids)
            }));
            // Discovery is a convenience: if the UDP port is busy, play goes on without it.
            if let Ok(socket) = UdpSocket::bind(("0.0.0.0", port)) {
                let (stop3, shared) = (stop.clone(), announcement.clone());
                threads.push(thread::spawn(move || discovery_loop(socket, stop3, shared)));
            }
        }

        let table = Table::new(settings, Rng::seeded(seed));
        let (stop2, announce_port, shared) =
            (stop.clone(), bound_port.unwrap_or(0), announcement.clone());
        threads.push(thread::spawn(move || {
            table_loop(table, rx, stop2, shared, announce_port)
        }));
        Ok(Server {
            inbound: tx,
            stop,
            next_id,
            threads,
            port: bound_port,
            announcement,
        })
    }

    /// Connects a client through memory. `owner` may pause and change speed.
    pub fn connect_local(&self, owner: bool) -> Connection {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (outbox, incoming) = mpsc::channel();
        let _ = self.inbound.send(Inbound::Connected { id, outbox, owner });
        Connection {
            outgoing: Outgoing::Local {
                id,
                inbound: self.inbound.clone(),
            },
            incoming,
        }
    }

    /// What the table currently looks like from outside (players, game running...).
    pub fn status(&self) -> Option<Announcement> {
        self.announcement.lock().ok().and_then(|a| a.clone())
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::Relaxed)
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn table_loop(
    mut table: Table,
    rx: Receiver<Inbound>,
    stop: Arc<AtomicBool>,
    announcement: Arc<Mutex<Option<Announcement>>>,
    port: u16,
) {
    while !stop.load(Ordering::Relaxed) {
        match rx.recv_timeout(TICK) {
            Ok(first) => {
                let now = Instant::now();
                for msg in std::iter::once(first).chain(rx.try_iter()) {
                    match msg {
                        Inbound::Connected { id, outbox, owner } => {
                            table.connect(id, outbox, owner)
                        }
                        Inbound::Message(id, m) => table.handle(id, m, now),
                        Inbound::Disconnected(id) => table.disconnect(id),
                        Inbound::Garbage(id) => table.disconnect(id),
                        Inbound::NotUnderstood(id) => table.not_understood(id),
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let now = Instant::now();
        table.tick(now);
        table.flush(now);
        if let Ok(mut a) = announcement.lock() {
            *a = Some(table.announcement(port));
        }
    }
    // Dropping the table drops every client's outbox, which closes their connections.
}

fn accept_loop(
    listener: TcpListener,
    tx: Sender<Inbound>,
    stop: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let id = next_id.fetch_add(1, Ordering::Relaxed);
                if serve_client(stream, id, tx.clone(), stop.clone()).is_err() {
                    let _ = tx.send(Inbound::Disconnected(id));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(_) => thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// Two threads per network client: one reads its messages, one writes ours.
fn serve_client(
    stream: TcpStream,
    id: ClientId,
    tx: Sender<Inbound>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    // A client that stops reading must not block the table forever.
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let read_half = stream.try_clone()?;
    let mut write_half = stream;
    let (outbox, outgoing) = mpsc::channel::<ServerMsg>();
    tx.send(Inbound::Connected {
        id,
        outbox,
        owner: false,
    })
    .map_err(|_| io::Error::other("the table has stopped"))?;

    thread::spawn(move || {
        for msg in outgoing {
            let Ok(mut line) = serde_json::to_vec(&msg) else {
                continue;
            };
            line.push(b'\n');
            if write_half.write_all(&line).is_err() {
                break;
            }
        }
        let _ = write_half.shutdown(std::net::Shutdown::Both);
    });

    thread::spawn(move || {
        let mut reader = BufReader::new(read_half);
        let mut buf = Vec::new();
        while !stop.load(Ordering::Relaxed) {
            match read_line(&mut reader, MAX_CLIENT_LINE, &mut buf) {
                Ok(Some(())) => match parse_message::<ClientMsg>(&buf) {
                    Parsed::Known(m) => {
                        if tx.send(Inbound::Message(id, m)).is_err() {
                            break;
                        }
                    }
                    Parsed::Unknown => {
                        if tx.send(Inbound::NotUnderstood(id)).is_err() {
                            break;
                        }
                    }
                    Parsed::Garbage => {
                        let _ = tx.send(Inbound::Garbage(id));
                        break;
                    }
                },
                Ok(None) | Err(_) => {
                    let _ = tx.send(Inbound::Disconnected(id));
                    break;
                }
            }
        }
    });
    Ok(())
}

fn discovery_loop(
    socket: UdpSocket,
    stop: Arc<AtomicBool>,
    announcement: Arc<Mutex<Option<Announcement>>>,
) {
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let mut buf = [0u8; 64];
    while !stop.load(Ordering::Relaxed) {
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if &buf[..n] != DISCOVERY_REQUEST {
            continue;
        }
        let reply = announcement
            .lock()
            .ok()
            .and_then(|a| a.as_ref().and_then(|a| serde_json::to_vec(a).ok()));
        if let Some(reply) = reply {
            let _ = socket.send_to(&reply, from);
        }
    }
}
