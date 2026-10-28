//! The settings window, and saving the settings to the settings file.

use ratatui::crossterm::event::KeyCode;

use holdem::config::{self, Config};
use holdem::protocol::ClientMsg;

use super::{App, SettingsRow, cycle};
use crate::keys::{self, Cmd, Place};
use crate::sound::Cue;

impl App {
    fn change_volume(&mut self, delta: i32) {
        if self.sound.unavailable().is_some() {
            return;
        }
        let step = i32::from(self.config.client.sound.volume_step);
        let v = (i32::from(self.sound.volume()) + delta * step).clamp(0, 100);
        self.sound.set_volume(v as u8);
        // A sample at the new level, so the change can be heard.
        self.sound.play(Cue::Bet, 0);
    }

    /// The settings as they are now: the loaded ones with everything the
    /// setup screen and the settings window changed.
    pub fn settings_as_config(&self) -> Config {
        let mut c = self.config.clone();
        let client = &mut c.client;
        // An unchanged name stays as it was, so an empty one keeps meaning the
        // computer's login name.
        if self.join.name.trim() != client.resolved_name() {
            client.name = self.join.name.trim().to_string();
        }
        if !self.speed().name.eq_ignore_ascii_case(&client.speed) {
            client.speed = self.speed().name.clone();
        }
        client.win_chance_hint = self.show_odds;
        client.sound.volume = self.sound.volume();
        let s = &self.setup;
        let table = &mut c.tables[s.table];
        table.seats = s.seats;
        table.fill_with_bots = s.fill_with_bots;
        table.allow_win_chance = s.allow_win_chance;
        table.fluctuation.enabled = s.fluctuation;
        table.rules = s.rules.clone();
        c
    }

    /// Saves the current settings to the settings file (see [`config::save`]).
    pub(super) fn save_settings(&mut self) {
        let Some(path) = self.config_path.clone().or_else(config::user_file) else {
            self.notify("There is no settings folder to save in: start holdem with --config FILE.");
            return;
        };
        let wanted = self.settings_as_config();
        match config::save(&path, &wanted, self.setup.table) {
            Ok(report) if report.changed.is_empty() => {
                if path.exists() {
                    self.notify(format!("Already saved in {}", path.display()));
                } else {
                    self.notify("Nothing to save: every setting is at its default.");
                }
            }
            Ok(report) => {
                let n = report.changed.len();
                self.notify(format!(
                    "Saved {} setting{} to {}",
                    n,
                    if n == 1 { "" } else { "s" },
                    path.display()
                ));
                self.config = wanted;
                self.config_note = format!("Settings: {}", path.display());
                self.config_path = Some(path);
            }
            Err(e) => self.notify(format!("Could not save the settings: {e}")),
        }
    }

    pub(super) fn change_speed(&mut self, delta: i32) {
        self.speed = cycle(self.speed, self.config.speeds.len(), delta);
        let ms = self.speed().ms;
        if let Some(s) = self.session.as_ref()
            && s.is_owner()
        {
            s.send(ClientMsg::SetSpeed(ms));
        }
    }

    pub(super) fn settings_key(&mut self, code: KeyCode) {
        let n = SettingsRow::ALL.len();
        match keys::command(Place::SettingsWindow, code) {
            Some(Cmd::Up) => self.settings_row = cycle(self.settings_row, n, -1),
            Some(Cmd::Down) => self.settings_row = cycle(self.settings_row, n, 1),
            Some(Cmd::Left) => self.change_setting(-1),
            Some(Cmd::Right) => self.change_setting(1),
            Some(Cmd::Close) => self.show_settings = false,
            Some(Cmd::Save) => self.save_settings(),
            _ => {}
        }
    }

    pub(super) fn change_setting(&mut self, delta: i32) {
        match SettingsRow::ALL[self.settings_row] {
            SettingsRow::Speed => self.change_speed(delta),
            SettingsRow::Volume => self.change_volume(delta),
            SettingsRow::WinChance => {
                if self.hints_allowed() {
                    self.show_odds = !self.show_odds;
                } else {
                    self.notify("This table does not allow the win-chance hint.");
                }
            }
        }
    }
}
