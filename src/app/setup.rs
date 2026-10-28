//! Keys on the setup screen and the join screen.

use ratatui::crossterm::event::KeyCode;

use holdem::game::{MAX_PLAYERS, MIN_PLAYERS};
use holdem::protocol::MAX_NAME;

use super::{App, MAX_ADDRESS, Screen, SetupRow, cycle, step_choice};
use crate::keys::{self, Cmd, Place};
use crate::sound::Cue;

impl App {
    pub(super) fn setup_key(&mut self, code: KeyCode) {
        let rows = self.setup.rows();
        let n = rows.len();
        let row = rows[self.setup.row.min(n - 1)];
        // On the name row, letters are typed: Q, ? and H too.
        if row == SetupRow::Name
            && let KeyCode::Char(c) = code
            && !c.is_control()
        {
            if self.join.name.chars().count() < MAX_NAME {
                self.join.name.push(c);
            }
            return;
        }
        match keys::command(Place::Setup, code) {
            Some(Cmd::Up) => {
                self.setup.row = cycle(self.setup.row, n, -1);
                self.sound.play(Cue::Tick, 0);
            }
            Some(Cmd::Down) => {
                self.setup.row = cycle(self.setup.row, n, 1);
                self.sound.play(Cue::Tick, 0);
            }
            Some(Cmd::Left) => self.setup_change(row, -1),
            Some(Cmd::Right) => self.setup_change(row, 1),
            Some(Cmd::Erase) if row == SetupRow::Name => {
                self.join.name.pop();
            }
            Some(Cmd::Select) if self.join.name.trim().is_empty() => {
                self.notify("Type your name first.".to_string());
                self.setup.row = 0;
            }
            Some(Cmd::Select) => self.start_game(),
            Some(Cmd::Save) => self.save_settings(),
            Some(Cmd::Back) => self.screen = Screen::Menu,
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }

    fn setup_change(&mut self, row: SetupRow, delta: i32) {
        let c = &self.config.client.setup;
        let s = &mut self.setup;
        let r = &mut s.rules;
        match row {
            // Typed, not stepped.
            SetupRow::Name => return,
            SetupRow::Seats => {
                s.seats =
                    (s.seats as i32 + delta).clamp(MIN_PLAYERS as i32, MAX_PLAYERS as i32) as usize
            }
            SetupRow::FillWithBots => s.fill_with_bots = !s.fill_with_bots,
            SetupRow::Chips => {
                r.starting_chips = step_choice(&c.starting_chips, r.starting_chips, delta)
            }
            SetupRow::Blind => r.small_blind = step_choice(&c.small_blinds, r.small_blind, delta),
            SetupRow::Levels => {
                r.hands_per_level = step_choice(&c.hands_per_level, r.hands_per_level, delta)
            }
            SetupRow::HandLimit => r.hand_limit = step_choice(&c.hand_limits, r.hand_limit, delta),
            SetupRow::Antes => r.antes = !r.antes,
            SetupRow::Straddle => r.straddle = !r.straddle,
            SetupRow::Rebuys => {
                r.rebuy_levels = step_choice(&c.rebuy_levels, r.rebuy_levels, delta)
            }
            SetupRow::Button => r.dead_button = !r.dead_button,
            SetupRow::Showdown => r.muck_losers = !r.muck_losers,
            SetupRow::Fluctuation => s.fluctuation = !s.fluctuation,
            SetupRow::WinChance => s.allow_win_chance = !s.allow_win_chance,
        }
        self.sound.play(Cue::Tick, 0);
    }

    pub(super) fn join_key(&mut self, code: KeyCode) {
        if self.connecting.is_some() {
            if keys::command(Place::Join, code) == Some(Cmd::Back) {
                // The attempt finishes in the background; its result is ignored.
                self.connecting = None;
                self.join.status = "Cancelled.".to_string();
            }
            return;
        }
        let rows = 2 + self.join.found.len();
        match keys::command(Place::Join, code) {
            Some(Cmd::Up) => self.join.row = cycle(self.join.row, rows, -1),
            Some(Cmd::Down) => self.join.row = cycle(self.join.row, rows, 1),
            Some(Cmd::Back) => self.screen = Screen::Menu,
            Some(Cmd::Select) => self.join_selected(),
            Some(Cmd::Erase) => match self.join.row {
                0 => {
                    self.join.name.pop();
                }
                1 => {
                    self.join.address.pop();
                }
                _ => {}
            },
            Some(cmd) => self.common(cmd),
            None => {
                if let KeyCode::Char(c) = code
                    && !c.is_control()
                {
                    match self.join.row {
                        0 if self.join.name.chars().count() < MAX_NAME => self.join.name.push(c),
                        1 if self.join.address.chars().count() < MAX_ADDRESS => {
                            self.join.address.push(c)
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn join_selected(&mut self) {
        if self.join.name.trim().is_empty() {
            self.join.status = "Type your name first.".to_string();
            self.join.row = 0;
        } else if self.join.row >= 2 {
            let f = &self.join.found[self.join.row - 2];
            match f.info.compatibility() {
                Ok(_) => {
                    let (address, table) = (f.address.to_string(), f.info.table.clone());
                    self.connect_to(address, Some(table));
                }
                Err(m) => self.join.status = m.to_client_about_table(&f.info.table),
            }
        } else if self.join.address.trim().is_empty() {
            self.join.status = "Type the address of the table (for example 192.168.1.20 or \
                                myhost.net:7777), or pick a table found on your network."
                .to_string();
            self.join.row = 1;
        } else {
            let address = self.join.address.clone();
            self.connect_to(address, None);
        }
    }

    /// Keys on the screen that lists a server's tables.
    pub(super) fn choose_key(&mut self, code: KeyCode) {
        let Some(tables) = self.session.as_ref().and_then(|s| s.choices.clone()) else {
            return;
        };
        let n = tables.len().max(1);
        match keys::command(Place::Choose, code) {
            Some(Cmd::Up) => self.choose_row = cycle(self.choose_row, n, -1),
            Some(Cmd::Down) => self.choose_row = cycle(self.choose_row, n, 1),
            Some(Cmd::Select) => {
                if let (Some(t), Some(s)) = (tables.get(self.choose_row), self.session.as_mut()) {
                    s.choose(&t.table);
                }
            }
            Some(Cmd::Back) => {
                // Hang up and go back to the addresses.
                self.session = None;
                self.screen = Screen::Join;
            }
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }
}
