//! Starting, joining and leaving games, and what happens as time passes.

use std::sync::mpsc::{self};
use std::thread;
use std::time::{Duration, Instant};

use holdem::bot;
use holdem::game::{GameEvent, Street};
use holdem::net::{self, Server};
use holdem::protocol::ClientMsg;
use holdem::table::TableSettings;

use super::{App, Hint, Screen, hint_key, opponents_in};
use crate::session::{Kind, Session};
use crate::sound::Cue;

impl App {
    pub(super) fn table_settings(&self, name: &str) -> TableSettings {
        let st = &self.setup;
        let mut t = self.config.table_settings(name, self.speed().ms, st.host);
        t.seats = st.seats;
        t.fill_with_bots = !st.host || st.fill_with_bots;
        t.rules = st.rules.clone();
        t.fluctuation = st.fluctuation.then(|| self.config.fluctuation());
        t.allow_win_chance = st.allow_win_chance;
        t
    }

    pub(super) fn seed(&mut self) -> u64 {
        match self.seed {
            Some(s) => s,
            None => self.hint_rng.next_u64(),
        }
    }

    pub(super) fn start_game(&mut self) {
        let host = self.setup.host;
        let name = if host {
            self.config.server.name.clone()
        } else {
            "Solo table".to_string()
        };
        let settings = self.table_settings(&name);
        let port = host.then_some(self.config.server.port);
        let seed = self.seed();
        let server = match Server::start(settings, port, seed) {
            Ok(s) => s,
            Err(e) => {
                self.notify(format!(
                    "Could not open port {}: {}. Is another table running? The port can be changed in holdem.toml.",
                    self.config.server.port, e
                ));
                return;
            }
        };
        let conn = server.connect_local(true);
        let player = self.join.name.clone();
        let session = Session::new(
            if host { Kind::Host } else { Kind::Solo },
            conn,
            Some(server),
            &player,
            self.config.client.log_lines,
        );
        if !host {
            session.send(ClientMsg::Ready);
        }
        self.enter_session(session);
        self.screen = if host { Screen::Lobby } else { Screen::Table };
    }

    fn enter_session(&mut self, session: Session) {
        self.session = Some(session);
        self.raise = None;
        self.hint = None;
        self.toast = None;
        self.all_in_armed = false;
        self.my_turn = false;
        self.last_pot = 0;
        self.discovery = None;
    }

    pub(super) fn connect_to(&mut self, address: String) {
        if self.connecting.is_some() {
            return;
        }
        let port = self.config.server.port;
        let timeout = Duration::from_secs(u64::from(self.config.client.join.connect_seconds));
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(net::connect(&address, port, timeout));
        });
        self.connecting = Some(rx);
        self.join.status = "Connecting…".to_string();
    }

    /// Leaves the current game and shows the main menu.
    pub(super) fn return_to_menu(&mut self) {
        self.session = None;
        self.screen = Screen::Menu;
        self.raise = None;
        self.hint = None;
        self.show_log = false;
        self.show_profile = false;
        self.all_in_armed = false;
    }

    /// Reads the network, updates the screen and plays sounds. Called often.
    pub fn tick(&mut self) {
        self.poll_connecting();
        self.poll_discovery();
        let paused = self.overlay_open();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.set_paused(paused);
        let update = session.poll();
        if let Some(reason) = session.closed.take() {
            self.return_to_menu();
            self.notify(reason);
            return;
        }
        if !update.notices.is_empty() {
            // A refused action can be tried again.
            self.answered = None;
        }
        for n in update.notices {
            self.notify(n);
        }
        if update.changed {
            self.follow_session();
        }
        self.play_events(&update.events);
        self.on_view();
    }

    /// Switches screens to match what the table is doing.
    fn follow_session(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let next = match (&session.view, &session.lobby) {
            (Some(v), _) if v.game_over => Screen::Over,
            (Some(_), _) => Screen::Table,
            (None, Some(_)) => Screen::Lobby,
            (None, None) => self.screen,
        };
        if next != self.screen {
            if next == Screen::Over {
                self.raise = None;
                self.all_in_armed = false;
            }
            self.screen = next;
        }
        // The raise box only makes sense while it is still your turn.
        if self.raise.is_some()
            && self
                .view()
                .is_none_or(|v| v.legal.is_none_or(|l| !l.can_raise))
        {
            self.raise = None;
        }
    }

    /// Chime on your turn, flash a growing pot, refresh the win-chance hint.
    fn on_view(&mut self) {
        let Some(v) = self.view() else {
            return;
        };
        let my_turn = v.legal.is_some();
        let pot = v.pot;
        let key = hint_key(v).filter(|_| self.hints_on());
        let hint_needed = key.is_some_and(|k| self.hint.as_ref().is_none_or(|h| h.key != k));
        let me = v.you.map(|s| v.seats[s].cards.clone()).unwrap_or_default();
        let board = v.board.clone();
        let opponents = opponents_in(v);
        if my_turn && !self.my_turn {
            self.sound.play(Cue::Turn, 120);
        }
        self.my_turn = my_turn;
        if pot > self.last_pot {
            self.pot_flash = Some(Instant::now());
        }
        self.last_pot = pot;
        if hint_needed && let Some(key) = key {
            let samples = self.config.client.hint_samples;
            let win = bot::equity(&me, &board, opponents, samples, &mut self.hint_rng);
            self.hint = Some(Hint {
                key,
                win,
                opponents,
            });
        }
    }

    fn play_events(&mut self, events: &[GameEvent]) {
        let Some(v) = self.view() else {
            return;
        };
        let you = v.you;
        let won = you.is_some_and(|s| v.seats[s].payout > 0);
        let champion = v
            .standings
            .first()
            .is_some_and(|s| s.is_human && Some(&s.name) == self.session.as_ref().map(|s| &s.name));
        let mut cues = Vec::new();
        for e in events {
            let cue = match *e {
                GameEvent::HandStarted => Some(Cue::Deal),
                GameEvent::Fold(_) => Some(Cue::Fold),
                GameEvent::Check(_) => Some(Cue::Check),
                GameEvent::Call(_) => Some(Cue::Chip),
                GameEvent::Bet(_) | GameEvent::Rebuy(_) => Some(Cue::Bet),
                GameEvent::AllIn(_) => Some(Cue::AllIn),
                GameEvent::Street(Street::Flop) => Some(Cue::Flop),
                GameEvent::Street(_) => Some(Cue::Card),
                GameEvent::HandSettled => Some(if won { Cue::Win } else { Cue::Pot }),
                GameEvent::Eliminated(seat) => Some(if Some(seat) == you {
                    Cue::Lose
                } else {
                    Cue::Bust
                }),
                GameEvent::Joined(_) | GameEvent::Left(_) => Some(Cue::Tick),
                GameEvent::TournamentOver => champion.then_some(Cue::Fanfare),
            };
            if let Some(c) = cue {
                cues.push(c);
            }
        }
        for (k, c) in cues.into_iter().enumerate() {
            self.sound
                .play(c, k as u64 * u64::from(self.config.client.sound.gap_ms));
        }
    }

    fn poll_connecting(&mut self) {
        let Some(rx) = self.connecting.as_ref() else {
            return;
        };
        let Ok(result) = rx.try_recv() else {
            return;
        };
        self.connecting = None;
        match result {
            Ok(conn) => {
                let name = self.join.name.clone();
                self.join.status.clear();
                self.enter_session(Session::new(
                    Kind::Guest,
                    conn,
                    None,
                    &name,
                    self.config.client.log_lines,
                ));
                self.screen = Screen::Lobby;
            }
            Err(e) => self.join.status = format!("Could not connect: {e}"),
        }
    }

    fn poll_discovery(&mut self) {
        if self.screen != Screen::Join {
            self.discovery = None;
            return;
        }
        if let Some(rx) = self.discovery.as_ref()
            && let Ok(found) = rx.try_recv()
        {
            self.join.found = found;
            self.discovery = None;
            let rows = 2 + self.join.found.len();
            if self.join.row >= rows {
                self.join.row = rows - 1;
            }
        }
        let due = self.last_discovery.is_none_or(|t| {
            t.elapsed() >= Duration::from_secs(u64::from(self.config.client.join.discovery_seconds))
        });
        if self.discovery.is_none() && due {
            let port = self.config.server.port;
            let wait = Duration::from_millis(u64::from(self.config.client.join.discovery_wait_ms));
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(net::discover(port, wait));
            });
            self.discovery = Some(rx);
            self.last_discovery = Some(Instant::now());
        }
    }
}
