//! The authoritative table: it owns the [`Game`], seats people and bots,
//! makes the bots move at the chosen speed, enforces time limits, handles
//! players joining, leaving, rebuying and coming and going ("fluctuation"),
//! and sends every connected client its own view of the table.
//!
//! The table knows nothing about sockets or threads: messages come in through
//! [`Table::handle`], time passes through [`Table::tick`], and views go out
//! through each client's channel in [`Table::flush`]. `net.rs` runs it on a
//! thread behind TCP; single-player games use it through in-memory channels.
//!
//! `Table`'s methods are split by concern: `connections` (connecting, hello,
//! leaving), `messages` (what clients ask for), `flow` (games, hands, seats and
//! time passing) and `views` (what each client is sent).

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::time::Instant;

use crate::card::Rng;
use crate::config::Config;
use crate::game::{Game, GameEvent, MAX_PLAYERS, MIN_PLAYERS, Rules};
use crate::protocol::{Announcement, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ServerMsg};

mod connections;
mod flow;
mod messages;
#[cfg(test)]
mod tests;
mod views;

pub type ClientId = u64;

/// Bots leave and new ones sit down between hands.
#[derive(Clone, Debug, PartialEq)]
pub struct Fluctuation {
    /// Chance per hand that a given bot leaves.
    pub leave_chance: f32,
    /// Chance per hand that an empty seat gets a new bot.
    pub join_chance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TableSettings {
    pub name: String,
    pub seats: usize,
    /// Seats not taken by people are given to bots.
    pub fill_with_bots: bool,
    pub rules: Rules,
    pub fluctuation: Option<Fluctuation>,
    pub bot_names: Vec<String>,
    /// Pause between bot moves.
    pub speed_ms: u32,
    /// Time a person has to act when several people play; 0 = unlimited.
    pub turn_seconds: u32,
    /// The next hand starts by itself after this long; 0 = wait for everyone.
    pub next_hand_seconds: u32,
    /// Seconds before a disconnected player's turn is folded for them.
    pub away_seconds: u32,
    /// Pause between the streets of an all-in run-out, in percent of `speed_ms`.
    pub runout_pace_percent: u32,
    /// Players may turn on the win-chance hint.
    pub allow_win_chance: bool,
    /// Monte Carlo samples per bot decision (more is stronger and slower).
    pub bot_samples: u32,
    /// Hands started with fewer big blinds do not count for the style estimate.
    pub stats_min_stack_bb: u32,
}

impl Default for TableSettings {
    /// A table with the default settings, not set up for network play (no
    /// time limits). The values come from [`Config::default`].
    fn default() -> TableSettings {
        let c = Config::default();
        c.table_settings(&c.server.name, c.speed(&c.server.speed).ms, false)
    }
}

struct Client {
    name: String,
    outbox: Sender<ServerMsg>,
    /// The person running this table (may pause and change speed).
    owner: bool,
    connected: bool,
    /// Has introduced itself with a valid Hello.
    joined: bool,
    seat: Option<usize>,
    /// Lost their seat (eliminated or left): watches until the next game.
    spectator: bool,
    ready: bool,
    log_seq: u64,
    events: Vec<GameEvent>,
    /// Remove this client at the next flush (after a goodbye).
    drop: bool,
}

pub struct Table {
    pub settings: TableSettings,
    rng: Rng,
    clients: BTreeMap<ClientId, Client>,
    game: Option<Game>,
    /// When the owner paused the game (only possible while they play alone).
    paused: Option<Instant>,
    next_step: Instant,
    /// The decision currently awaited and when it began (for time limits).
    turn: ((u32, u64), Instant),
    hand_over_since: Option<Instant>,
    dirty: bool,
    shown_seconds: (Option<u32>, Option<u32>),
}

impl Table {
    pub fn new(mut settings: TableSettings, rng: Rng) -> Table {
        settings.seats = settings.seats.clamp(MIN_PLAYERS, MAX_PLAYERS);
        settings.speed_ms = settings.speed_ms.clamp(20, 10_000);
        let now = Instant::now();
        Table {
            settings,
            rng,
            clients: BTreeMap::new(),
            game: None,
            paused: None,
            next_step: now,
            turn: ((0, 0), now),
            hand_over_since: None,
            dirty: true,
            shown_seconds: (None, None),
        }
    }

    pub fn game(&self) -> Option<&Game> {
        self.game.as_ref()
    }

    /// Summary for LAN discovery.
    pub fn announcement(&self, port: u16) -> Announcement {
        Announcement {
            table: self.settings.name.clone(),
            port,
            players: self.clients.values().filter(|c| c.joined).count(),
            seats: self.settings.seats,
            playing: self.game.is_some(),
            version: PROTOCOL_VERSION,
            min_version: Some(MIN_PROTOCOL_VERSION),
        }
    }

    fn connected_people(&self) -> usize {
        self.clients
            .values()
            .filter(|c| c.joined && c.connected)
            .count()
    }

    fn notice(&self, id: ClientId, text: impl Into<String>) {
        if let Some(c) = self.clients.get(&id) {
            let _ = c.outbox.send(ServerMsg::Notice(text.into()));
        }
    }
}
