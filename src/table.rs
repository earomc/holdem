//! The authoritative table: it owns the [`Game`], seats people and bots,
//! makes the bots move at the chosen speed, enforces time limits, handles
//! players joining, leaving, rebuying and coming and going ("fluctuation"),
//! and sends every connected client its own view of the table.
//!
//! The table knows nothing about sockets or threads: messages come in through
//! [`Table::handle`], time passes through [`Table::tick`], and views go out
//! through each client's channel in [`Table::flush`]. `net.rs` runs it on a
//! thread behind TCP; single-player games use it through in-memory channels.

use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::bot::{self, Personality};
use crate::card::Rng;
use crate::config::Config;
use crate::eval;
use crate::game::{Action, Game, GameEvent, MAX_PLAYERS, MIN_PLAYERS, Rules, State};
use crate::protocol::{
    APP_VERSION, Announcement, ClientMsg, LobbyPlayer, LobbyView, MIN_PROTOCOL_VERSION,
    PROTOCOL_VERSION, SeatView, ServerMsg, TableView, clean_name, negotiate,
};

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

    // ------------------------------------------------------------- connections

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
        let seated = c.seat.is_some() && self.game.is_some();
        if !seated {
            self.clients.remove(&id);
        }
        // A seated player keeps the seat until the hand ends: they fold when it
        // is their turn and leave the table before the next hand.
        self.dirty = true;
    }

    fn hello(&mut self, id: ClientId, name: &str, version: u32, min_version: Option<u32>) {
        let agreed = match negotiate(version, min_version) {
            Ok(v) => v,
            Err(mismatch) => {
                if let Some(c) = self.clients.get_mut(&id) {
                    let _ = c.outbox.send(ServerMsg::Goodbye(mismatch.to_client()));
                    c.drop = true;
                }
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
                unique = format!("{} {}", wanted, k);
                k += 1;
            }
            if let Some(c) = self.clients.get_mut(&id) {
                c.name = unique;
                c.joined = true;
            }
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
                "The table did not understand a request: it runs an older holdem ({}).",
                APP_VERSION
            )));
        } else {
            // Not even the greeting could be read: say which version we are and hang up.
            let _ = c.outbox.send(ServerMsg::Goodbye(format!(
                "This table runs holdem {} (protocol version {} to {}) and could not read your program's greeting. Please use matching versions of holdem.",
                APP_VERSION, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION
            )));
            c.drop = true;
        }
    }

    // ------------------------------------------------------------- messages

    pub fn handle(&mut self, id: ClientId, msg: ClientMsg, now: Instant) {
        if let ClientMsg::Hello {
            name,
            version,
            min_version,
            ..
        } = &msg
        {
            self.hello(id, name, *version, *min_version);
            return;
        }
        let Some(client) = self.clients.get(&id) else {
            return;
        };
        if !client.joined {
            return;
        }
        let seat = client.seat;
        let owner = client.owner;
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::Act { action, turn } => self.act(id, seat, action, turn),
            ClientMsg::Ready => self.ready(id, seat, now),
            ClientMsg::Rebuy(yes) => {
                if let (Some(g), Some(s)) = (self.game.as_mut(), seat) {
                    if yes {
                        if let Err(e) = g.rebuy(s) {
                            self.notice(id, e);
                        }
                    } else {
                        g.decline_rebuy(s);
                    }
                }
            }
            ClientMsg::Pause(pause) => {
                if owner && self.connected_people() <= 1 {
                    self.set_paused(pause, now);
                } else if pause {
                    self.notice(id, "The game cannot be paused while other people play.");
                }
            }
            ClientMsg::SetSpeed(ms) => {
                if owner {
                    self.settings.speed_ms = ms.clamp(20, 10_000);
                }
            }
        }
        self.dirty = true;
        self.collect_events();
    }

    /// Pauses or resumes. The time limits stand still while paused: they
    /// are moved on by however long the pause lasted.
    fn set_paused(&mut self, pause: bool, now: Instant) {
        match (self.paused, pause) {
            (None, true) => self.paused = Some(now),
            (Some(since), false) => {
                let lasted = now.duration_since(since);
                self.turn.1 += lasted;
                if let Some(t) = self.hand_over_since.as_mut() {
                    *t += lasted;
                }
                self.next_step += lasted;
                self.paused = None;
            }
            _ => {}
        }
    }

    fn act(&mut self, id: ClientId, seat: Option<usize>, action: Action, turn: u64) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        if turn != g.turn_id() {
            // Meant for an earlier decision (a key pressed twice, or crossed with a deal).
            return;
        }
        if g.state != State::Betting || Some(g.to_act) != seat {
            self.notice(id, "It is not your turn.");
            return;
        }
        if let Err(e) = g.act(action) {
            self.notice(id, e);
        }
    }

    fn ready(&mut self, id: ClientId, seat: Option<usize>, now: Instant) {
        if let Some(c) = self.clients.get_mut(&id) {
            c.ready = match self.game.as_ref() {
                // In the lobby Enter toggles; elsewhere it confirms.
                None => !c.ready,
                Some(_) => true,
            };
        }
        match self.game.as_ref().map(|g| g.state) {
            None => self.try_start(now),
            Some(State::HandOver) => {
                // Saying "next hand" while a rebuy is offered declines it.
                if let (Some(g), Some(s)) = (self.game.as_mut(), seat) {
                    g.decline_rebuy(s);
                }
            }
            Some(State::GameOver) => self.lobby_when_everyone_is_done(),
            _ => {}
        }
    }

    /// After the final standings, back to the lobby once everyone still
    /// connected has seen them (also when the last one not ready leaves).
    fn lobby_when_everyone_is_done(&mut self) {
        let mut people = self.clients.values().filter(|c| c.joined && c.connected);
        if people.all(|c| c.ready) {
            self.back_to_lobby();
        }
    }

    // ------------------------------------------------------------- game flow

    fn back_to_lobby(&mut self) {
        self.game = None;
        self.paused = None;
        self.clients.retain(|_, c| c.connected);
        for c in self.clients.values_mut() {
            c.seat = None;
            c.spectator = false;
            c.ready = false;
            c.log_seq = 0;
            c.events.clear();
        }
        self.dirty = true;
    }

    fn try_start(&mut self, now: Instant) {
        let people: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| c.joined && c.connected)
            .map(|(id, _)| *id)
            .collect();
        if people.is_empty() || !people.iter().all(|id| self.clients[id].ready) {
            return;
        }
        let seats = self.settings.seats;
        let humans = people.len().min(seats);
        let total = if self.settings.fill_with_bots {
            seats
        } else {
            humans
        };
        if total < MIN_PLAYERS {
            for id in &people {
                self.notice(
                    *id,
                    "At least two players are needed: wait for someone to join, or turn on bots.",
                );
                if let Some(c) = self.clients.get_mut(id) {
                    c.ready = false;
                }
            }
            return;
        }
        let rules = self.settings.rules.clone();
        let mut g = Game::empty(rules, seats, Rng::seeded(self.rng.next_u64()));
        g.bot_samples = self.settings.bot_samples.max(1);
        g.stats_min_stack_bb = self.settings.stats_min_stack_bb;
        let chips = g.rules.starting_chips;
        for (seat, id) in people.iter().take(humans).enumerate() {
            let name = self.clients[id].name.clone();
            g.seat_player(seat, &name, true, Personality::default(), chips)
                .expect("the seat is free");
            if let Some(c) = self.clients.get_mut(id) {
                c.seat = Some(seat);
            }
        }
        if self.settings.fill_with_bots {
            let mut taken: Vec<String> = self.clients.values().map(|c| c.name.clone()).collect();
            let bots = bot::roster_from(&self.settings.bot_names, seats - humans, &mut self.rng);
            for (k, (_, style)) in bots.into_iter().enumerate() {
                let name = bot::unused_name(&self.settings.bot_names, &taken, &mut self.rng);
                taken.push(name.clone());
                g.seat_player(humans + k, &name, false, style, chips)
                    .expect("the seat is free");
            }
        }
        g.take_events();
        for c in self.clients.values_mut() {
            c.log_seq = 0;
            c.events.clear();
        }
        self.game = Some(g);
        self.start_next_hand(now);
    }

    /// Seat changes between hands, then the deal.
    fn start_next_hand(&mut self, now: Instant) {
        if self.game.as_ref().is_some_and(|g| g.hand_no > 0) {
            self.reseat();
        }
        let Some(g) = self.game.as_mut() else {
            return;
        };
        g.start_hand();
        for c in self.clients.values_mut() {
            c.ready = false;
        }
        self.hand_over_since = None;
        self.next_step = now + self.delay();
        self.dirty = true;
        self.collect_events();
    }

    fn delay(&self) -> Duration {
        Duration::from_millis(self.settings.speed_ms as u64)
    }

    fn client_at(&self, seat: usize) -> Option<(&ClientId, &Client)> {
        self.clients.iter().find(|(_, c)| c.seat == Some(seat))
    }

    /// People leaving, bots coming and going, and waiting people sitting down.
    fn reseat(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        // People whose connection dropped leave the table.
        let gone: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| !c.connected)
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            if let Some(c) = self.clients.remove(&id)
                && let Some(s) = c.seat
            {
                let _ = g.remove_player(s);
            }
        }
        // Seats of people who were knocked out are free for others now; they watch.
        for c in self.clients.values_mut() {
            if let Some(s) = c.seat
                && g.players[s].out
            {
                c.seat = None;
                c.spectator = true;
            }
        }
        let starting = g.rules.starting_chips;
        if let Some(f) = self.settings.fluctuation.clone() {
            for s in 0..g.players.len() {
                let p = &g.players[s];
                if !p.is_human
                    && !p.out
                    && !p.rebuy_pending
                    && g.alive_count() >= 3
                    && self.rng.chance(f.leave_chance)
                {
                    let _ = g.remove_player(s);
                }
            }
        }
        // People waiting for a seat get one; if the table is full, a bot makes room.
        let waiting: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| c.joined && c.connected && c.seat.is_none() && !c.spectator)
            .map(|(id, _)| *id)
            .collect();
        for id in waiting {
            let free =
                (0..g.players.len()).find(|&s| g.players[s].out && !g.players[s].rebuy_pending);
            let seat = match free {
                Some(s) => Some(s),
                None => {
                    let bots: Vec<usize> = (0..g.players.len())
                        .filter(|&s| {
                            !g.players[s].is_human
                                && !g.players[s].out
                                && !g.players[s].rebuy_pending
                        })
                        .collect();
                    if bots.is_empty() {
                        None
                    } else {
                        let s = bots[self.rng.below(bots.len() as u64) as usize];
                        let _ = g.remove_player(s);
                        Some(s)
                    }
                }
            };
            if let Some(s) = seat {
                let name = self.clients[&id].name.clone();
                if g.seat_player(s, &name, true, Personality::default(), starting)
                    .is_ok()
                    && let Some(c) = self.clients.get_mut(&id)
                {
                    c.seat = Some(s);
                }
            }
        }
        if let Some(f) = self.settings.fluctuation.clone() {
            for s in 0..g.players.len() {
                if g.players[s].out && !g.players[s].rebuy_pending && self.rng.chance(f.join_chance)
                {
                    let taken: Vec<String> = g
                        .players
                        .iter()
                        .map(|p| p.name.clone())
                        .chain(self.clients.values().map(|c| c.name.clone()))
                        .collect();
                    let (name, style) =
                        bot::newcomer(&self.settings.bot_names, &taken, &mut self.rng);
                    let _ = g.seat_player(s, &name, false, style, starting);
                }
            }
        }
    }

    /// Moves events from the game into every client's queue.
    fn collect_events(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        let events = g.take_events();
        if events.is_empty() {
            return;
        }
        for c in self.clients.values_mut() {
            c.events.extend_from_slice(&events);
            // A client that is not reading its queue must not make us run out of memory.
            if c.events.len() > 500 {
                let extra = c.events.len() - 500;
                c.events.drain(0..extra);
            }
        }
        self.dirty = true;
    }

    /// People whose "next hand" vote counts: seated and connected. (Someone
    /// knocked out in the last hand still votes once, so they see how it ended.)
    fn voters(&self) -> Vec<ClientId> {
        self.clients
            .iter()
            .filter(|(_, c)| c.connected && c.seat.is_some())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Advances time: bots move, timers run out, the next hand is dealt.
    pub fn tick(&mut self, now: Instant) {
        if self.paused.is_some() {
            if self.connected_people() > 1 {
                // Someone else came in: they must not wait for a window to close.
                self.set_paused(false, now);
                self.dirty = true;
            } else {
                return;
            }
        }
        let Some(state) = self.game.as_ref().map(|g| g.state) else {
            return;
        };
        match state {
            State::Betting => self.tick_betting(now),
            State::Runout => {
                if now >= self.next_step
                    && let Some(g) = self.game.as_mut()
                {
                    g.runout_step();
                    self.next_step =
                        now + self.delay() * self.settings.runout_pace_percent.max(10) / 100;
                }
            }
            State::HandOver => {
                if self.hand_over_since.is_none() {
                    self.hand_over_since = Some(now);
                    self.bot_rebuys();
                    self.dirty = true;
                }
                let voters = self.voters();
                let everyone_ready = voters.iter().all(|id| self.clients[id].ready);
                let timed_out = self.settings.next_hand_seconds > 0
                    && self.hand_over_since.is_some_and(|t| {
                        now.duration_since(t)
                            >= Duration::from_secs(self.settings.next_hand_seconds as u64)
                    });
                if everyone_ready || timed_out {
                    self.start_next_hand(now);
                }
            }
            State::GameOver => self.lobby_when_everyone_is_done(),
        }
        self.collect_events();
    }

    fn tick_betting(&mut self, now: Instant) {
        let people = self.connected_people();
        let Some(g) = self.game.as_ref() else {
            return;
        };
        let seat = g.to_act;
        let key = (g.hand_no, g.action_count);
        if self.turn.0 != key {
            self.turn = (key, now);
        }
        if !g.players[seat].is_human {
            if now >= self.next_step
                && let Some(g) = self.game.as_mut()
            {
                g.bot_move();
                self.next_step = now + self.delay();
            }
            return;
        }
        let connected = self.client_at(seat).is_some_and(|(_, c)| c.connected);
        let limit = if !connected {
            Some(Duration::from_secs(self.settings.away_seconds as u64))
        } else if self.settings.turn_seconds > 0 && people > 1 {
            Some(Duration::from_secs(self.settings.turn_seconds as u64))
        } else {
            None
        };
        if let Some(limit) = limit
            && now.duration_since(self.turn.1) >= limit
        {
            let action = if g.legal(seat).can_check {
                Action::Check
            } else {
                Action::Fold
            };
            if connected && let Some((id, _)) = self.client_at(seat) {
                self.notice(*id, "Time is up: you were checked or folded automatically.");
            }
            if let Some(g) = self.game.as_mut() {
                let _ = g.act(action);
                self.next_step = now + self.delay();
            }
        }
    }

    fn bot_rebuys(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        for s in 0..g.players.len() {
            if g.players[s].rebuy_pending && !g.players[s].is_human {
                if bot::wants_rebuy(&g.players[s].personality, &mut self.rng) {
                    let _ = g.rebuy(s);
                } else {
                    g.decline_rebuy(s);
                }
            }
        }
    }

    // ------------------------------------------------------------- views

    fn seconds_left(&self, now: Instant) -> (Option<u32>, Option<u32>) {
        let Some(g) = self.game.as_ref() else {
            return (None, None);
        };
        let left = |since: Instant, total: u32| {
            let end = since + Duration::from_secs(total as u64);
            end.saturating_duration_since(now).as_secs() as u32
        };
        let turn = if g.state == State::Betting
            && g.players[g.to_act].is_human
            && self.settings.turn_seconds > 0
            && self.connected_people() > 1
        {
            Some(left(self.turn.1, self.settings.turn_seconds))
        } else {
            None
        };
        let next = match (g.state, self.hand_over_since) {
            (State::HandOver, Some(t)) if self.settings.next_hand_seconds > 0 => {
                Some(left(t, self.settings.next_hand_seconds))
            }
            _ => None,
        };
        (turn, next)
    }

    /// Sends fresh views to every client if anything changed.
    pub fn flush(&mut self, now: Instant) {
        let seconds = self.seconds_left(now);
        if seconds != self.shown_seconds {
            self.shown_seconds = seconds;
            self.dirty = true;
        }
        self.clients.retain(|_, c| !c.drop);
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            if !self.clients[&id].joined {
                continue;
            }
            let msg = match self.game.is_some() {
                false => ServerMsg::Lobby(self.lobby_view(id)),
                true => ServerMsg::Table(Box::new(self.table_view(id, seconds))),
            };
            let _ = self.clients[&id].outbox.send(msg);
        }
    }

    fn lobby_view(&self, id: ClientId) -> LobbyView {
        LobbyView {
            table: self.settings.name.clone(),
            players: self
                .clients
                .iter()
                .filter(|(_, c)| c.joined && c.connected)
                .map(|(other, c)| LobbyPlayer {
                    name: c.name.clone(),
                    ready: c.ready,
                    you: *other == id,
                })
                .collect(),
            seats: self.settings.seats,
            fill_with_bots: self.settings.fill_with_bots,
            fluctuation: self.settings.fluctuation.is_some(),
            rules: self.settings.rules.clone(),
            win_chance_allowed: self.settings.allow_win_chance,
        }
    }

    fn table_view(&mut self, id: ClientId, seconds: (Option<u32>, Option<u32>)) -> TableView {
        let voters = self.voters();
        let ready_count = voters.iter().filter(|v| self.clients[*v].ready).count();
        let people = self.connected_people();
        let waiting: Vec<String> = self
            .clients
            .values()
            .filter(|c| c.joined && c.connected && c.seat.is_none() && !c.spectator)
            .map(|c| c.name.clone())
            .collect();
        let connected_seats: Vec<(usize, bool)> = self
            .clients
            .values()
            .filter_map(|c| c.seat.map(|s| (s, c.connected)))
            .collect();
        let (paused, speed_ms) = (self.paused.is_some(), self.settings.speed_ms);
        let g = self.game.as_ref().expect("a game is running");
        let client = self.clients.get_mut(&id).expect("the client exists");
        let you = client.seat;
        let (new_log, next_seq) = g.log_since(client.log_seq);
        let new_log = new_log.to_vec();
        client.log_seq = next_seq;
        let events = std::mem::take(&mut client.events);

        let seats: Vec<SeatView> = g
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let visible = (Some(i) == you || p.shown) && p.in_hand;
                SeatView {
                    name: p.name.clone(),
                    is_human: p.is_human,
                    connected: connected_seats
                        .iter()
                        .find(|(s, _)| *s == i)
                        .map(|(_, c)| *c)
                        .unwrap_or(true),
                    chips: p.chips,
                    bet: p.bet,
                    in_hand: p.in_hand,
                    folded: p.folded,
                    all_in: p.all_in,
                    out: p.out,
                    vacant: p.vacant,
                    place: g.place_of(i),
                    last_action: p.last_action.clone(),
                    cards: if visible { p.hole.clone() } else { Vec::new() },
                    hidden_cards: !visible && p.in_hand && !p.folded && p.hole.len() == 2,
                    payout: p.payout,
                    rebuy_pending: p.rebuy_pending,
                    rebuys: p.rebuys,
                }
            })
            .collect();
        let your_hand = you.and_then(|s| {
            let p = &g.players[s];
            if p.hole.len() != 2 || !p.in_hand {
                None
            } else if g.board.len() >= 3 {
                g.made_hand(s)
            } else {
                Some(eval::describe_hole(p.hole[0], p.hole[1]))
            }
        });
        let betting = g.state == State::Betting;
        TableView {
            you,
            seats,
            board: g.board.clone(),
            state: g.state,
            street: g.street,
            pot: g.pot(),
            pots: g.pots_now(),
            last_pot: g.last_pot,
            hand_no: g.hand_no,
            level: g.level,
            sb: g.sb,
            bb: g.bb,
            ante: g.ante,
            rules: g.rules.clone(),
            button: g.button,
            sb_seat: g.sb_seat,
            sb_dead: g.sb_dead,
            bb_seat: g.bb_seat,
            straddle_seat: g.straddle_seat,
            to_act: if betting { Some(g.to_act) } else { None },
            current_bet: g.current_bet,
            legal: match you {
                Some(s) if betting && g.to_act == s => Some(g.legal(s)),
                _ => None,
            },
            turn: g.turn_id(),
            your_hand,
            result_lines: g.result_lines.clone(),
            new_log,
            events,
            rebuy_offer: match you {
                Some(s) if g.players[s].rebuy_pending => Some(g.rules.rebuy_stack()),
                _ => None,
            },
            last_hand: g.state == State::HandOver && g.tournament_over(),
            game_over: g.state == State::GameOver,
            standings: if g.state == State::GameOver {
                g.standings()
            } else {
                Vec::new()
            },
            your_stats: you.map(|s| g.players[s].stats).unwrap_or_default(),
            turn_seconds_left: seconds.0,
            next_hand_seconds_left: seconds.1,
            ready: (ready_count, voters.len()),
            you_ready: client.ready,
            waiting,
            paused,
            speed_ms,
            owner: client.owner,
            multiplayer: people > 1,
            win_chance_allowed: self.settings.allow_win_chance,
            stats_min_stack_bb: self.settings.stats_min_stack_bb,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, channel};

    struct Peer {
        id: ClientId,
        rx: Receiver<ServerMsg>,
        view: Option<TableView>,
    }

    impl Peer {
        /// Reads everything sent so far and keeps the newest table view.
        fn read(&mut self) -> Vec<ServerMsg> {
            let msgs: Vec<ServerMsg> = self.rx.try_iter().collect();
            for m in &msgs {
                if let ServerMsg::Table(v) = m {
                    self.view = Some((**v).clone());
                }
            }
            msgs
        }
    }

    fn join(t: &mut Table, id: ClientId, name: &str, owner: bool, now: Instant) -> Peer {
        let (tx, rx) = channel();
        t.connect(id, tx, owner);
        t.handle(id, ClientMsg::hello(name), now);
        Peer { id, rx, view: None }
    }

    /// Runs the clock forward in small steps, letting `play` answer for people.
    fn run(t: &mut Table, now: &mut Instant, peers: &mut [Peer], steps: usize, play: bool) {
        for _ in 0..steps {
            *now += Duration::from_millis(100);
            t.tick(*now);
            t.flush(*now);
            for p in peers.iter_mut() {
                p.read();
                let Some(v) = p.view.clone() else { continue };
                if play && let Some(l) = v.legal {
                    // Cautious: check when free, call only small bets.
                    let a = if l.can_check {
                        Action::Check
                    } else if l.to_call <= l.stack / 20 {
                        Action::Call
                    } else {
                        Action::Fold
                    };
                    t.handle(
                        p.id,
                        ClientMsg::Act {
                            action: a,
                            turn: v.turn,
                        },
                        *now,
                    );
                }
                if play && v.state == State::HandOver && !v.you_ready && v.you.is_some() {
                    t.handle(p.id, ClientMsg::Ready, *now);
                }
            }
        }
    }

    fn settings() -> TableSettings {
        TableSettings {
            speed_ms: 100,
            seats: 4,
            ..TableSettings::default()
        }
    }

    #[test]
    fn single_player_game_starts_and_plays_hands() {
        let mut now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(1));
        let mut me = join(&mut t, 1, "Me", true, now);
        let msgs = me.read();
        assert!(matches!(msgs[0], ServerMsg::Welcome { .. }));
        t.handle(1, ClientMsg::Ready, now);
        t.flush(now);
        me.read();
        let v = me.view.clone().expect("the game started");
        assert_eq!(v.you, Some(0));
        assert_eq!(v.seats.len(), 4);
        assert_eq!(v.seats[0].cards.len(), 2, "you see your own cards");
        for s in &v.seats[1..] {
            assert!(
                s.cards.is_empty() && s.hidden_cards,
                "bots' cards stay hidden"
            );
        }
        let mut peers = [me];
        run(&mut t, &mut now, &mut peers, 600, true);
        let g = t.game().unwrap();
        assert!(g.hand_no >= 3, "only {} hands in a minute", g.hand_no);
        assert_eq!(g.chips_in_play(), g.total_chips());
        assert!(!peers[0].view.as_ref().unwrap().multiplayer);
    }

    #[test]
    fn single_player_waits_for_enter_between_hands() {
        let mut now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(2));
        let me = join(&mut t, 1, "Me", true, now);
        t.handle(1, ClientMsg::Ready, now);
        let mut peers = [me];
        // Play the hand but never say "next hand".
        for _ in 0..300 {
            now += Duration::from_millis(100);
            t.tick(now);
            t.flush(now);
            peers[0].read();
            if let Some(l) = peers[0].view.as_ref().and_then(|v| v.legal) {
                let a = if l.can_check {
                    Action::Check
                } else {
                    Action::Fold
                };
                t.handle(
                    1,
                    ClientMsg::Act {
                        action: a,
                        turn: peers[0].view.as_ref().unwrap().turn,
                    },
                    now,
                );
            }
        }
        let g = t.game().unwrap();
        assert_eq!(g.hand_no, 1);
        assert_eq!(g.state, State::HandOver);
        t.handle(1, ClientMsg::Ready, now);
        t.tick(now);
        assert_eq!(t.game().unwrap().hand_no, 2);
    }

    #[test]
    fn two_people_never_see_each_others_cards() {
        let mut now = Instant::now();
        let s = TableSettings {
            fill_with_bots: false,
            ..settings()
        };
        let mut t = Table::new(s, Rng::seeded(3));
        let a = join(&mut t, 1, "Ann", true, now);
        let b = join(&mut t, 2, "Bob", false, now);
        t.handle(1, ClientMsg::Ready, now);
        assert!(t.game().is_none(), "waits for everyone to be ready");
        t.handle(2, ClientMsg::Ready, now);
        assert_eq!(
            t.game().unwrap().players.iter().filter(|p| !p.out).count(),
            2
        );
        let mut peers = [a, b];
        for _ in 0..200 {
            run(&mut t, &mut now, &mut peers, 1, true);
            for p in peers.iter() {
                let Some(v) = &p.view else { continue };
                for (i, seat) in v.seats.iter().enumerate() {
                    if Some(i) != v.you && !seat.cards.is_empty() {
                        // Only cards turned up at a showdown or an all-in may be seen.
                        let g = t.game().unwrap();
                        assert!(g.players[i].shown, "{} saw hidden cards", p.id);
                    }
                }
            }
        }
        assert!(peers[0].view.as_ref().unwrap().multiplayer);
    }

    #[test]
    fn a_dropped_player_folds_and_leaves_and_can_come_back() {
        let mut now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(4));
        let a = join(&mut t, 1, "Ann", true, now);
        let _b = join(&mut t, 2, "Bob", false, now);
        t.handle(1, ClientMsg::Ready, now);
        t.handle(2, ClientMsg::Ready, now);
        let bob_seat = t.clients[&2].seat.unwrap();
        t.disconnect(2);
        let mut peers = [a];
        // Bob is folded for when it is his turn, and leaves before hand 2.
        while t.game().unwrap().hand_no < 2 {
            run(&mut t, &mut now, &mut peers, 1, true);
        }
        let g = t.game().unwrap();
        assert!(g.players[bob_seat].vacant, "Bob left the table");
        assert!(g.history.iter().any(|f| f.name == "Bob" && f.left));
        assert_eq!(g.chips_in_play(), g.total_chips());
        // Bob comes back and is seated again at the next hand.
        let _b2 = join(&mut t, 3, "Bob", false, now);
        while t.game().unwrap().hand_no < 3 {
            run(&mut t, &mut now, &mut peers, 1, true);
        }
        let g = t.game().unwrap();
        assert!(
            g.players
                .iter()
                .any(|p| p.name == "Bob" && p.is_human && !p.out)
        );
        assert_eq!(t.clients[&3].seat, Some(bob_seat));
    }

    #[test]
    fn a_player_who_reconnects_during_the_hand_keeps_the_seat() {
        let now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(10));
        let _a = join(&mut t, 1, "Ann", true, now);
        let _b = join(&mut t, 2, "Bob", false, now);
        t.handle(1, ClientMsg::Ready, now);
        t.handle(2, ClientMsg::Ready, now);
        let seat = t.clients[&2].seat;
        t.disconnect(2);
        let _b2 = join(&mut t, 7, "Bob", false, now);
        assert_eq!(t.clients[&7].seat, seat);
        assert!(!t.clients.contains_key(&2));
    }

    #[test]
    fn people_joining_a_running_game_get_a_seat_next_hand() {
        let mut now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(5));
        let a = join(&mut t, 1, "Ann", true, now);
        t.handle(1, ClientMsg::Ready, now);
        // Table is full of bots (4 seats): one bot makes room.
        let mut c = join(&mut t, 9, "Cy", false, now);
        assert!(c.read().iter().any(|m| matches!(m, ServerMsg::Notice(_))));
        let mut peers = [a, c];
        run(&mut t, &mut now, &mut peers, 400, true);
        let g = t.game().unwrap();
        assert!(g.players.iter().any(|p| p.name == "Cy" && p.is_human));
        assert_eq!(g.players.len(), 4);
    }

    #[test]
    fn an_answer_to_an_earlier_turn_is_ignored() {
        let mut now = Instant::now();
        let s = TableSettings {
            fill_with_bots: false,
            ..settings()
        };
        let mut t = Table::new(s, Rng::seeded(13));
        let mut a = join(&mut t, 1, "Ann", true, now);
        let mut b = join(&mut t, 2, "Bob", false, now);
        t.handle(1, ClientMsg::Ready, now);
        t.handle(2, ClientMsg::Ready, now);
        t.flush(now);
        a.read();
        b.read();
        // Heads-up: whoever acts first calls, and then sends the same call again.
        let (first, first_peer) = if a.view.as_ref().unwrap().legal.is_some() {
            (1, &a)
        } else {
            (2, &b)
        };
        let old_turn = first_peer.view.as_ref().unwrap().turn;
        let call = ClientMsg::Act {
            action: Action::Call,
            turn: old_turn,
        };
        t.handle(first, call.clone(), now);
        let after_one = t.game().unwrap().action_count;
        t.handle(first, call, now);
        assert_eq!(
            t.game().unwrap().action_count,
            after_one,
            "the repeat changes nothing"
        );
        assert_ne!(t.game().unwrap().turn_id(), old_turn);
        now += Duration::from_millis(10);
        t.tick(now);
    }

    #[test]
    fn duplicate_names_get_a_number() {
        let now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(6));
        let _a = join(&mut t, 1, "Ann", true, now);
        let mut b = join(&mut t, 2, "Ann", false, now);
        let msgs = b.read();
        assert!(
            msgs.iter()
                .any(|m| matches!(m, ServerMsg::Welcome { name, .. } if name == "Ann 2"))
        );
    }

    #[test]
    fn wrong_versions_are_turned_away_with_a_reason() {
        let now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(7));
        let hello = |version, min_version| ClientMsg::Hello {
            name: "X".into(),
            version,
            min_version,
            app: "test".into(),
        };
        // A client from before protocol 3 (holdem 0.3).
        let (tx, rx) = channel();
        t.connect(1, tx, false);
        t.handle(1, hello(2, None), now);
        match rx.try_recv() {
            Ok(ServerMsg::Goodbye(why)) => assert!(why.contains("too old"), "{}", why),
            other => panic!("expected a goodbye, got {:?}", other),
        }
        // A client that only speaks newer versions.
        let (tx, rx) = channel();
        t.connect(2, tx, false);
        t.handle(
            2,
            hello(PROTOCOL_VERSION + 2, Some(PROTOCOL_VERSION + 1)),
            now,
        );
        match rx.try_recv() {
            Ok(ServerMsg::Goodbye(why)) => assert!(why.contains("older holdem"), "{}", why),
            other => panic!("expected a goodbye, got {:?}", other),
        }
        t.flush(now);
        assert!(t.clients.is_empty());
        // A newer client that still speaks our version is welcome, at our version.
        let (tx, rx) = channel();
        t.connect(3, tx, false);
        t.handle(
            3,
            hello(PROTOCOL_VERSION + 2, Some(MIN_PROTOCOL_VERSION)),
            now,
        );
        match rx.try_recv() {
            Ok(ServerMsg::Welcome { version, .. }) => assert_eq!(version, PROTOCOL_VERSION),
            other => panic!("expected a welcome, got {:?}", other),
        }
    }

    #[test]
    fn the_win_chance_permission_reaches_every_view() {
        let now = Instant::now();
        let s = TableSettings {
            allow_win_chance: false,
            ..settings()
        };
        let mut t = Table::new(s, Rng::seeded(12));
        let mut me = join(&mut t, 1, "Ann", true, now);
        t.flush(now);
        let lobby = me
            .read()
            .into_iter()
            .find_map(|m| match m {
                ServerMsg::Lobby(l) => Some(l),
                _ => None,
            })
            .unwrap();
        assert!(!lobby.win_chance_allowed);
        t.handle(1, ClientMsg::Ready, now);
        t.flush(now);
        me.read();
        assert!(!me.view.unwrap().win_chance_allowed);
    }

    #[test]
    fn fluctuation_changes_the_table_and_keeps_the_chips_right() {
        let mut now = Instant::now();
        let s = TableSettings {
            seats: 6,
            fluctuation: Some(Fluctuation {
                leave_chance: 0.2,
                join_chance: 0.5,
            }),
            ..settings()
        };
        let mut t = Table::new(s, Rng::seeded(8));
        let a = join(&mut t, 1, "Ann", true, now);
        t.handle(1, ClientMsg::Ready, now);
        let mut peers = [a];
        let first: Vec<String> = t
            .game()
            .unwrap()
            .players
            .iter()
            .map(|p| p.name.clone())
            .collect();
        run(&mut t, &mut now, &mut peers, 1500, true);
        let g = t.game().unwrap();
        assert!(g.hand_no >= 8);
        let names: Vec<String> = g.players.iter().map(|p| p.name.clone()).collect();
        assert_ne!(first, names, "somebody left or joined");
        assert!(g.history.iter().any(|f| f.left));
        assert_eq!(g.chips_in_play(), g.total_chips());
    }

    #[test]
    fn the_owner_can_pause_a_single_player_game() {
        let mut now = Instant::now();
        let mut t = Table::new(settings(), Rng::seeded(9));
        let _a = join(&mut t, 1, "Ann", true, now);
        t.handle(1, ClientMsg::Ready, now);
        t.handle(1, ClientMsg::Pause(true), now);
        let before = t.game().unwrap().action_count;
        for _ in 0..50 {
            now += Duration::from_millis(200);
            t.tick(now);
        }
        assert_eq!(t.game().unwrap().action_count, before);
        t.handle(1, ClientMsg::Pause(false), now);
        t.handle(1, ClientMsg::SetSpeed(20), now);
        assert_eq!(t.settings.speed_ms, 20);
    }

    #[test]
    fn a_pause_does_not_eat_into_the_time_to_act() {
        let mut now = Instant::now();
        let mut s = settings();
        s.turn_seconds = 30;
        let mut t = Table::new(s, Rng::seeded(2));
        let mut host = join(&mut t, 1, "Host", true, now);
        t.handle(1, ClientMsg::Ready, now);
        // Let the bots play until it is the host's turn.
        let deadline = now + Duration::from_secs(600);
        while t
            .game()
            .is_none_or(|g| g.state != State::Betting || g.to_act != 0)
        {
            assert!(now < deadline, "never the host's turn");
            now += Duration::from_millis(100);
            t.tick(now);
            if t.game().is_some_and(|g| g.state == State::HandOver) {
                t.handle(1, ClientMsg::Ready, now);
            }
        }
        let decision = t.game().unwrap().turn_id();
        t.handle(1, ClientMsg::Pause(true), now);
        // A long pause; then a guest arrives, which resumes the game.
        now += Duration::from_secs(120);
        let _guest = join(&mut t, 2, "Guest", false, now);
        t.tick(now);
        assert!(t.paused.is_none(), "a second person ends the pause");
        now += Duration::from_secs(5);
        t.tick(now);
        assert_eq!(
            t.game().unwrap().turn_id(),
            decision,
            "the host still has time to act"
        );
        t.flush(now);
        host.read();
        let left = host.view.unwrap().turn_seconds_left.unwrap();
        assert!((24..=25).contains(&left), "{} seconds left", left);
    }
}
