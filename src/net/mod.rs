//! Runs a [`Table`](crate::table::Table) on a background thread and connects clients to it, either
//! through in-memory channels (your own seat) or over TCP (everyone else).
//! Messages are JSON, one per line. A UDP responder on the same port answers
//! LAN discovery broadcasts so people on the local network can find the table.
//!
//! `server` runs the table and accepts players; `client` connects to a table and
//! finds tables nearby. Both speak through a [`Connection`].

use std::io::{self, BufRead, Read};
use std::sync::mpsc::{Receiver, Sender};

use crate::protocol::{ClientMsg, ServerMsg};
use crate::table::ClientId;

mod client;
mod server;
#[cfg(test)]
mod tests;

pub use client::{Found, connect, discover, local_ip};
use server::Inbound;
pub use server::Server;

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
