//! Runs a [`Table`] on a background thread and connects clients to it, either
//! through in-memory channels (your own seat) or over TCP (everyone else).
//! Messages are JSON, one per line. A UDP responder on the same port answers
//! LAN discovery broadcasts so people on the local network can find the table.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::card::Rng;
use crate::protocol::{Announcement, ClientMsg, DISCOVERY_REQUEST, ServerMsg};
use crate::table::{ClientId, Table, TableSettings};

/// Longest line a server accepts from a client.
const MAX_CLIENT_LINE: u64 = 16 * 1024;
/// Longest line a client accepts from a server.
const MAX_SERVER_LINE: u64 = 8 * 1024 * 1024;
const TICK: Duration = Duration::from_millis(20);

/// What reaches the table thread.
enum Inbound {
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

/// The result of reading one line of the protocol.
enum Parsed<T> {
    Known(T),
    /// Valid JSON, but a message this version does not know (from a newer program).
    Unknown,
    /// Not a protocol message at all.
    Garbage,
}

fn parse_message<T: serde::de::DeserializeOwned>(line: &[u8]) -> Parsed<T> {
    match serde_json::from_slice::<T>(line) {
        Ok(m) => Parsed::Known(m),
        Err(_) => match serde_json::from_slice::<serde_json::Value>(line) {
            Ok(serde_json::Value::Object(_)) => Parsed::Unknown,
            _ => Parsed::Garbage,
        },
    }
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

/// One client's link to a table, local or remote.
pub struct Connection {
    outgoing: Outgoing,
    /// Messages from the table.
    pub incoming: Receiver<ServerMsg>,
}

enum Outgoing {
    Local {
        id: ClientId,
        inbound: Sender<Inbound>,
    },
    Tcp(Sender<ClientMsg>),
}

impl Connection {
    pub fn send(&self, msg: ClientMsg) {
        match &self.outgoing {
            Outgoing::Local { id, inbound } => {
                let _ = inbound.send(Inbound::Message(*id, msg));
            }
            Outgoing::Tcp(tx) => {
                let _ = tx.send(msg);
            }
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        if let Outgoing::Local { id, inbound } = &self.outgoing {
            let _ = inbound.send(Inbound::Disconnected(*id));
        }
        // A TCP connection closes when its writer thread sees the channel close.
    }
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

/// Reads one line of at most `limit` bytes. `Ok(None)` at the end of the stream.
fn read_line(reader: &mut impl BufRead, limit: u64, buf: &mut Vec<u8>) -> io::Result<Option<()>> {
    buf.clear();
    let n = reader.by_ref().take(limit).read_until(b'\n', buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.last() != Some(&b'\n') && n as u64 >= limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "line too long"));
    }
    Ok(Some(()))
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

/// Connects to a table over TCP. `address` is "host" or "host:port".
pub fn connect(address: &str, default_port: u16, timeout: Duration) -> io::Result<Connection> {
    let address = address.trim();
    let with_port = if address
        .rsplit_once(':')
        .is_some_and(|(_, p)| p.parse::<u16>().is_ok())
        && !address.ends_with(']')
    {
        address.to_string()
    } else {
        format!("{address}:{default_port}")
    };
    let addrs: Vec<SocketAddr> = with_port.to_socket_addrs()?.collect();
    let mut last_err = io::Error::new(io::ErrorKind::NotFound, "address not found");
    let mut stream = None;
    for a in addrs {
        match TcpStream::connect_timeout(&a, timeout) {
            Ok(s) => {
                stream = Some(s);
                break;
            }
            Err(e) => last_err = e,
        }
    }
    let stream = stream.ok_or(last_err)?;
    stream.set_nodelay(true)?;
    let read_half = stream.try_clone()?;
    let mut write_half = stream;
    let (out_tx, out_rx) = mpsc::channel::<ClientMsg>();
    let (in_tx, in_rx) = mpsc::channel::<ServerMsg>();

    thread::spawn(move || {
        for msg in out_rx {
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
        loop {
            match read_line(&mut reader, MAX_SERVER_LINE, &mut buf) {
                Ok(Some(())) => match parse_message::<ServerMsg>(&buf) {
                    Parsed::Known(m) => {
                        if in_tx.send(m).is_err() {
                            break;
                        }
                    }
                    // Something a newer table sends that this version does not need.
                    Parsed::Unknown => {}
                    Parsed::Garbage => {
                        let _ = in_tx.send(ServerMsg::Goodbye(
                            "The table sent something this version cannot read.".to_string(),
                        ));
                        break;
                    }
                },
                Ok(None) | Err(_) => {
                    let _ = in_tx.send(ServerMsg::Goodbye(
                        "The connection to the table was lost.".to_string(),
                    ));
                    break;
                }
            }
        }
    });
    Ok(Connection {
        outgoing: Outgoing::Tcp(out_tx),
        incoming: in_rx,
    })
}

/// A table found on the local network.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    pub address: SocketAddr,
    pub info: Announcement,
}

/// Broadcasts a discovery request and collects answers for `wait`.
pub fn discover(port: u16, wait: Duration) -> Vec<Found> {
    let mut found = Vec::new();
    let Ok(socket) = UdpSocket::bind(("0.0.0.0", 0)) else {
        return found;
    };
    let _ = socket.set_broadcast(true);
    let _ = socket.send_to(DISCOVERY_REQUEST, ("255.255.255.255", port));
    // Also ask this computer directly, for a table hosted on the same machine.
    let _ = socket.send_to(DISCOVERY_REQUEST, ("127.0.0.1", port));
    let deadline = Instant::now() + wait;
    let mut buf = [0u8; 2048];
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        if left.is_zero() || socket.set_read_timeout(Some(left)).is_err() {
            break;
        }
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            break;
        };
        if let Ok(info) = serde_json::from_slice::<Announcement>(&buf[..n]) {
            let address = SocketAddr::new(from.ip(), info.port);
            // The same table may answer on the loopback and the LAN address.
            let duplicate = found.iter().any(|f: &Found| {
                f.info == info && (f.address.ip().is_loopback() || from.ip().is_loopback())
            });
            if !duplicate && !found.iter().any(|f| f.address == address) {
                found.push(Found { address, info });
            }
        }
    }
    found
}

/// This computer's address on the local network, if it can be found.
/// (Connecting a UDP socket sends nothing; it only picks the outgoing interface.)
pub fn local_ip() -> Option<std::net::IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("192.0.2.1", 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    if ip.is_unspecified() { None } else { Some(ip) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Action, State};
    use crate::protocol::TableView;

    fn hello(c: &Connection, name: &str) {
        c.send(ClientMsg::hello(name));
    }

    /// Waits for a table view that satisfies `pred`.
    fn wait_for(c: &Connection, pred: impl Fn(&TableView) -> bool) -> TableView {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            assert!(!left.is_zero(), "timed out waiting for the table");
            if let Ok(ServerMsg::Table(v)) = c.incoming.recv_timeout(left)
                && pred(&v)
            {
                return *v;
            }
        }
    }

    #[test]
    fn a_host_and_a_network_player_play_together() {
        let settings = TableSettings {
            seats: 3,
            speed_ms: 20,
            ..TableSettings::default()
        };
        let server = Server::start(settings, Some(0), 42).expect("the server starts");
        let port = server.port.unwrap();
        let host = server.connect_local(true);
        hello(&host, "Host");
        let guest = connect("127.0.0.1", port, Duration::from_secs(5)).expect("connects");
        hello(&guest, "Guest");
        // Both say they are ready; the game starts with one bot.
        thread::sleep(Duration::from_millis(100));
        host.send(ClientMsg::Ready);
        guest.send(ClientMsg::Ready);
        let v = wait_for(&guest, |v| v.hand_no >= 1);
        assert_eq!(v.seats.len(), 3);
        let me = v.you.expect("the guest has a seat");
        assert_eq!(v.seats[me].name, "Guest");
        // Play a few hands: both people fold or check whenever it is their turn.
        let mut hands = 0;
        let deadline = Instant::now() + Duration::from_secs(30);
        while hands < 3 && Instant::now() < deadline {
            for c in [&host, &guest] {
                while let Ok(m) = c.incoming.try_recv() {
                    if let ServerMsg::Table(v) = m {
                        if let Some(l) = v.legal {
                            c.send(ClientMsg::Act {
                                action: if l.can_check {
                                    Action::Check
                                } else {
                                    Action::Fold
                                },
                                turn: v.turn,
                            });
                        }
                        if v.state == State::HandOver && !v.you_ready {
                            c.send(ClientMsg::Ready);
                        }
                        hands = hands.max(v.hand_no);
                        for (i, s) in v.seats.iter().enumerate() {
                            if Some(i) != v.you && !s.cards.is_empty() {
                                assert!(
                                    v.state == State::HandOver || v.state == State::Runout,
                                    "cards are only ever revealed at the end"
                                );
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        assert!(hands >= 3, "only {hands} hands were played");
    }

    #[test]
    fn discovery_finds_a_table_on_this_computer() {
        let settings = TableSettings {
            name: "Kitchen table".to_string(),
            ..TableSettings::default()
        };
        let server = Server::start(settings, Some(0), 1).expect("the server starts");
        let port = server.port.unwrap();
        thread::sleep(Duration::from_millis(100));
        let found = discover(port, Duration::from_millis(800));
        assert!(
            found
                .iter()
                .any(|f| f.info.table == "Kitchen table" && f.info.port == port),
            "found {found:?}"
        );
    }

    #[test]
    fn garbage_and_huge_lines_are_rejected() {
        let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
        let port = server.port.unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(b"this is not json\n").unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buf = [0u8; 16];
        // The server hangs up.
        let n = s.read(&mut buf).unwrap_or(0);
        assert_eq!(n, 0);
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let big = vec![b'x'; (MAX_CLIENT_LINE + 10) as usize];
        let _ = s.write_all(&big);
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let n = s.read(&mut buf).unwrap_or(0);
        assert_eq!(n, 0);
    }

    #[test]
    fn unknown_messages_are_answered_not_fatal() {
        let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
        let port = server.port.unwrap();
        let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut w = s.try_clone().unwrap();
        let mut r = BufReader::new(s);
        let mut line = String::new();
        let mut send = |text: &str| {
            w.write_all(text.as_bytes()).unwrap();
            w.write_all(b"\n").unwrap();
        };
        send(&serde_json::to_string(&ClientMsg::hello("Future")).unwrap());
        r.read_line(&mut line).unwrap();
        assert!(line.contains("Welcome"), "{}", line);
        // A request from some future version of holdem.
        send(r#"{"Teleport":{"to":"Mars"}}"#);
        let mut got_notice = false;
        for _ in 0..5 {
            line.clear();
            if r.read_line(&mut line).unwrap_or(0) == 0 {
                break;
            }
            if line.contains("Notice") {
                got_notice = true;
                break;
            }
        }
        assert!(got_notice, "the table explains instead of hanging up");
        // The connection still works.
        send(r#""Ready""#);
        line.clear();
        assert!(r.read_line(&mut line).unwrap() > 0);
    }

    #[test]
    fn a_greeting_the_table_cannot_read_gets_an_explanation() {
        let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
        let port = server.port.unwrap();
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(b"{\"Greetings\":{\"protocol\":99}}\n").unwrap();
        let mut r = BufReader::new(s);
        let mut line = String::new();
        r.read_line(&mut line).unwrap();
        assert!(
            line.contains("Goodbye") && line.contains("version"),
            "{}",
            line
        );
    }

    #[test]
    fn local_connections_leave_cleanly() {
        let mut server = Server::start(TableSettings::default(), None, 1).expect("starts");
        assert_eq!(server.port, None);
        let c = server.connect_local(true);
        hello(&c, "Solo");
        c.send(ClientMsg::Ready);
        let v = wait_for(&c, |v| v.hand_no == 1);
        assert_eq!(v.you, Some(0));
        drop(c);
        server.stop();
        assert!(!server.is_running());
    }

    #[test]
    fn addresses_get_the_default_port() {
        let server = Server::start(TableSettings::default(), Some(0), 1).unwrap();
        let port = server.port.unwrap();
        assert!(connect(&format!("127.0.0.1:{port}"), 1, Duration::from_secs(2)).is_ok());
        assert!(connect("127.0.0.1", port, Duration::from_secs(2)).is_ok());
        assert!(connect("localhost", port, Duration::from_secs(2)).is_ok());
    }
}
