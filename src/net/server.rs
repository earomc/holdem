//! The server side: the tables' threads, accepting players over TCP (and
//! sending each to the table they chose), and answering LAN discovery.

use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::{Connection, Outgoing, Parsed, parse_message, read_line};
use crate::card::Rng;
use crate::protocol::{Announcement, ClientMsg, DISCOVERY_REQUEST, ServerMsg, negotiate};
use crate::table::{ClientId, Table, TableEvent, TableSettings};

/// Longest line a server accepts from a client.
pub(super) const MAX_CLIENT_LINE: u64 = 16 * 1024;

pub(super) const TICK: Duration = Duration::from_millis(20);

/// The first protocol version in which a server may have several tables.
const TABLES_VERSION: u32 = 4;

/// What reaches a table's thread.
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

/// One table of a server: its thread's inbox and what it announces.
#[derive(Clone)]
struct Slot {
    name: String,
    inbound: Sender<Inbound>,
    announcement: Arc<Mutex<Option<Announcement>>>,
}

impl Slot {
    fn announcement(&self) -> Option<Announcement> {
        self.announcement.lock().ok().and_then(|a| a.clone())
    }
}

/// Running tables, all reachable through one port. Dropping it stops every
/// table and closes every connection.
pub struct Server {
    tables: Arc<Vec<Slot>>,
    stop: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
    threads: Vec<JoinHandle<()>>,
    /// The TCP port, when the tables accept network players.
    pub port: Option<u16>,
}

impl Server {
    /// Starts one table. With `port`, it also listens for network players on
    /// that port (0 picks a free one) and answers LAN discovery.
    pub fn start(settings: TableSettings, port: Option<u16>, seed: u64) -> io::Result<Server> {
        Server::start_inner(vec![settings], port, seed, None)
    }

    /// Starts several tables behind one port; players choose one when they
    /// join. Also reports what happens at each table, with its name (people
    /// joining and leaving, games starting and ending).
    pub fn start_tables(
        tables: Vec<TableSettings>,
        port: Option<u16>,
        seed: u64,
    ) -> io::Result<(Server, Receiver<(String, TableEvent)>)> {
        let (log, events) = mpsc::channel();
        Ok((Server::start_inner(tables, port, seed, Some(log))?, events))
    }

    fn start_inner(
        tables: Vec<TableSettings>,
        port: Option<u16>,
        seed: u64,
        log: Option<Sender<(String, TableEvent)>>,
    ) -> io::Result<Server> {
        assert!(!tables.is_empty(), "a server has at least one table");
        let stop = Arc::new(AtomicBool::new(false));
        let next_id = Arc::new(AtomicU64::new(1));
        let mut threads = Vec::new();

        let listener = match port {
            Some(port) => {
                let listener = TcpListener::bind(("0.0.0.0", port))?;
                listener.set_nonblocking(true)?;
                Some(listener)
            }
            None => None,
        };
        let bound_port = match &listener {
            Some(l) => Some(l.local_addr()?.port()),
            None => None,
        };

        let mut slots = Vec::new();
        for (k, settings) in tables.into_iter().enumerate() {
            let (tx, rx) = mpsc::channel::<Inbound>();
            let name = settings.name.clone();
            let table = Table::new(settings, Rng::seeded(seed.wrapping_add(k as u64)));
            let announce_port = bound_port.unwrap_or(0);
            // Known from the start, so the very first client can be offered the list.
            let announcement = Arc::new(Mutex::new(Some(table.announcement(announce_port))));
            slots.push(Slot {
                name,
                inbound: tx,
                announcement: announcement.clone(),
            });
            let (stop2, log) = (stop.clone(), log.clone());
            threads.push(thread::spawn(move || {
                table_loop(table, rx, stop2, announcement, announce_port, log)
            }));
        }
        let tables = Arc::new(slots);

        if let (Some(listener), Some(port)) = (listener, bound_port) {
            let (tables2, stop2, ids) = (tables.clone(), stop.clone(), next_id.clone());
            threads.push(thread::spawn(move || {
                accept_loop(listener, tables2, stop2, ids)
            }));
            // Discovery is a convenience: if the UDP port is busy, play goes on without it.
            if let Ok(socket) = UdpSocket::bind(("0.0.0.0", port)) {
                let (stop3, tables3) = (stop.clone(), tables.clone());
                threads.push(thread::spawn(move || {
                    discovery_loop(socket, stop3, tables3)
                }));
            }
        }
        Ok(Server {
            tables,
            stop,
            next_id,
            threads,
            port: bound_port,
        })
    }

    /// Connects a client to the first table through memory. `owner` may pause
    /// and change speed.
    pub fn connect_local(&self, owner: bool) -> Connection {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (outbox, incoming) = mpsc::channel();
        let inbound = self.tables[0].inbound.clone();
        let _ = inbound.send(Inbound::Connected { id, outbox, owner });
        Connection {
            outgoing: Outgoing::Local { id, inbound },
            incoming,
        }
    }

    /// What the first table currently looks like from outside (players, game running...).
    pub fn status(&self) -> Option<Announcement> {
        self.tables[0].announcement()
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
    log: Option<Sender<(String, TableEvent)>>,
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
        for event in table.take_notes() {
            if let Some(log) = &log {
                let _ = log.send((table.settings.name.clone(), event));
            }
        }
        if let Ok(mut a) = announcement.lock() {
            *a = Some(table.announcement(port));
        }
    }
    // Dropping the table drops every client's outbox, which closes their connections.
}

fn accept_loop(
    listener: TcpListener,
    tables: Arc<Vec<Slot>>,
    stop: Arc<AtomicBool>,
    next_id: Arc<AtomicU64>,
) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let id = next_id.fetch_add(1, Ordering::Relaxed);
                // A connection that fails this early never reached a table.
                let _ = serve_client(stream, id, tables.clone(), stop.clone());
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50))
            }
            Err(_) => thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// The table a `Hello` asks for, if it can be decided: the one it names, the
/// only one, or the first for programs that cannot choose (protocol 3).
/// `None` means the client should be offered the list.
fn table_for(tables: &[Slot], hello: &ClientMsg) -> Option<usize> {
    let ClientMsg::Hello {
        version,
        min_version,
        table,
        ..
    } = hello
    else {
        return Some(0);
    };
    if let Some(i) = table
        .as_deref()
        .and_then(|name| tables.iter().position(|t| t.name == name))
    {
        return Some(i);
    }
    let can_choose = negotiate(*version, *min_version).is_ok_and(|v| v >= TABLES_VERSION);
    if tables.len() == 1 || !can_choose {
        Some(0)
    } else {
        None
    }
}

/// The server's tables, for a client to choose from.
fn table_list(tables: &[Slot]) -> ServerMsg {
    ServerMsg::Tables(tables.iter().filter_map(Slot::announcement).collect())
}

/// Reads from a new client until it has a table (see [`table_for`]): offers
/// the list if needed, then hands the client and its greeting to that table
/// and returns the table's inbox. `None` if the client left first.
fn route(
    tables: &[Slot],
    id: ClientId,
    outbox: Sender<ServerMsg>,
    reader: &mut impl BufRead,
    buf: &mut Vec<u8>,
    stop: &AtomicBool,
) -> Option<Sender<Inbound>> {
    // The client's `Hello`, once it has been offered the list.
    let mut offered: Option<ClientMsg> = None;
    while !stop.load(Ordering::Relaxed) {
        if !matches!(read_line(reader, MAX_CLIENT_LINE, buf), Ok(Some(()))) {
            return None;
        }
        let msg = parse_message::<ClientMsg>(buf);
        let (table, first) = match (offered.take(), msg) {
            (None, Parsed::Known(hello @ ClientMsg::Hello { .. })) => {
                match table_for(tables, &hello) {
                    Some(i) => (i, Inbound::Message(id, hello)),
                    None => {
                        let _ = outbox.send(table_list(tables));
                        offered = Some(hello);
                        continue;
                    }
                }
            }
            // Anything else first: the first table answers it, as always.
            (None, Parsed::Known(other)) => (0, Inbound::Message(id, other)),
            (None, Parsed::Unknown) => (0, Inbound::NotUnderstood(id)),
            (None, Parsed::Garbage) => (0, Inbound::Garbage(id)),
            (Some(hello), msg) => {
                let chosen = match msg {
                    Parsed::Known(ClientMsg::ChooseTable(name)) => {
                        tables.iter().position(|t| t.name == name)
                    }
                    _ => None,
                };
                let Some(i) = chosen else {
                    let _ = outbox.send(ServerMsg::Notice(
                        "There is no table by that name here: choose one from the list."
                            .to_string(),
                    ));
                    let _ = outbox.send(table_list(tables));
                    offered = Some(hello);
                    continue;
                };
                // The table greets the client as if it had asked for it.
                (i, Inbound::Message(id, hello))
            }
        };
        let inbound = tables[table].inbound.clone();
        // From now on the table holds the outbox, so it can close the connection.
        inbound
            .send(Inbound::Connected {
                id,
                outbox,
                owner: false,
            })
            .ok()?;
        inbound.send(first).ok()?;
        return Some(inbound);
    }
    None
}

/// Two threads per network client: one reads its messages, one writes ours.
fn serve_client(
    stream: TcpStream,
    id: ClientId,
    tables: Arc<Vec<Slot>>,
    stop: Arc<AtomicBool>,
) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    // A client that stops reading must not block the table forever.
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let read_half = stream.try_clone()?;
    let mut write_half = stream;
    let (outbox, outgoing) = mpsc::channel::<ServerMsg>();

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
        let Some(tx) = route(&tables, id, outbox, &mut reader, &mut buf, &stop) else {
            return;
        };
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

fn discovery_loop(socket: UdpSocket, stop: Arc<AtomicBool>, tables: Arc<Vec<Slot>>) {
    let _ = socket.set_read_timeout(Some(Duration::from_millis(200)));
    let mut buf = [0u8; 64];
    while !stop.load(Ordering::Relaxed) {
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        if &buf[..n] != DISCOVERY_REQUEST {
            continue;
        }
        // One answer per table, so each appears on its own on the join screen.
        for slot in tables.iter() {
            if let Some(reply) = slot
                .announcement()
                .and_then(|a| serde_json::to_vec(&a).ok())
            {
                let _ = socket.send_to(&reply, from);
            }
        }
    }
}
