//! The client side: connecting to a table, and finding tables on the network.

use std::io::{self, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::mpsc::{self};
use std::thread::{self};
use std::time::{Duration, Instant};

use super::{Connection, Outgoing, Parsed, parse_message, read_line};
use crate::protocol::{Announcement, ClientMsg, DISCOVERY_REQUEST, ServerMsg};

/// Longest line a client accepts from a server.
pub(super) const MAX_SERVER_LINE: u64 = 8 * 1024 * 1024;

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
            // A server with several tables answers once per table.
            let same = |f: &Found| f.address == address && f.info.table == info.table;
            if !duplicate && !found.iter().any(same) {
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
