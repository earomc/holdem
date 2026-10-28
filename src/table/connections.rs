//! People connecting, saying hello (name and protocol version), and leaving.

use std::sync::mpsc::Sender;

use super::{Client, ClientId, Table, TableEvent};
use crate::protocol::{
    APP_VERSION, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ServerMsg, clean_name, negotiate,
};

impl Table {
    pub fn connect(&mut self, id: ClientId, outbox: Sender<ServerMsg>, owner: bool) {
        self.clients.insert(
            id,
            Client {
                name: String::new(),
                outbox,
                owner,
                connected: true,
                joined: false,
                seat: None,
                spectator: false,
                ready: false,
                log_seq: 0,
                events: Vec::new(),
                drop: false,
            },
        );
    }

    pub fn disconnect(&mut self, id: ClientId) {
        let Some(c) = self.clients.get_mut(&id) else {
            return;
        };
        c.connected = false;
        let left = c.joined.then(|| c.name.clone());
        let seated = c.seat.is_some() && self.game.is_some();
        if !seated {
            self.clients.remove(&id);
        }
        if let Some(name) = left {
            let people = self.connected_people();
            self.note(TableEvent::Left { name, people });
        }
        // A seated player keeps the seat until the hand ends: they fold when it
        // is their turn and leave the table before the next hand.
        self.dirty = true;
    }

    pub(super) fn hello(
        &mut self,
        id: ClientId,
        name: &str,
        version: u32,
        min_version: Option<u32>,
    ) {
        let agreed = match negotiate(version, min_version) {
            Ok(v) => v,
            Err(mismatch) => {
                let reason = mismatch.to_client();
                if let Some(c) = self.clients.get_mut(&id) {
                    let _ = c.outbox.send(ServerMsg::Goodbye(reason.clone()));
                    c.drop = true;
                }
                self.note(TableEvent::TurnedAway { reason });
                return;
            }
        };
        let wanted = clean_name(name);
        // Someone coming back after a dropped connection gets their seat back.
        let returning = self
            .clients
            .iter()
            .find(|(other, c)| **other != id && !c.connected && c.joined && c.name == wanted)
            .map(|(other, _)| *other);
        if let Some(old) = returning
            && let Some(prev) = self.clients.remove(&old)
            && let Some(c) = self.clients.get_mut(&id)
        {
            c.name = prev.name;
            c.seat = prev.seat;
            c.spectator = prev.spectator;
            c.joined = true;
            let name = c.name.clone();
            self.note(TableEvent::Returned { name });
        } else {
            let taken: Vec<String> = self
                .clients
                .iter()
                .filter(|(other, c)| **other != id && c.joined)
                .map(|(_, c)| c.name.clone())
                .chain(self.game.iter().flat_map(|g| {
                    g.players
                        .iter()
                        .filter(|p| !p.vacant)
                        .map(|p| p.name.clone())
                }))
                .collect();
            let mut unique = wanted.clone();
            let mut k = 2;
            while taken.contains(&unique) {
                unique = format!("{wanted} {k}");
                k += 1;
            }
            if let Some(c) = self.clients.get_mut(&id) {
                c.name = unique.clone();
                c.joined = true;
            }
            let people = self.connected_people();
            self.note(TableEvent::Joined {
                name: unique,
                people,
            });
        }
        if let Some(c) = self.clients.get(&id) {
            let _ = c.outbox.send(ServerMsg::Welcome {
                name: c.name.clone(),
                table: self.settings.name.clone(),
                version: agreed,
                app: APP_VERSION.to_string(),
            });
        }
        if self.game.is_some()
            && let Some(c) = self.clients.get(&id)
            && c.seat.is_none()
        {
            self.notice(
                id,
                "A game is running: you will get a seat at the next hand.",
            );
        }
        self.dirty = true;
    }

    /// A message of a kind this version does not know (from a newer program).
    pub fn not_understood(&mut self, id: ClientId) {
        let Some(c) = self.clients.get_mut(&id) else {
            return;
        };
        if c.joined {
            let _ = c.outbox.send(ServerMsg::Notice(format!(
                "The table did not understand a request: it runs an older holdem ({APP_VERSION})."
            )));
        } else {
            // Not even the greeting could be read: say which version we are and hang up.
            let _ = c.outbox.send(ServerMsg::Goodbye(format!(
                "This table runs holdem {APP_VERSION} (protocol version {MIN_PROTOCOL_VERSION} to {PROTOCOL_VERSION}) and could not read your program's greeting. Please use matching versions of holdem."
            )));
            c.drop = true;
        }
    }
}
