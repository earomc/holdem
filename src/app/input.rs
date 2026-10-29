//! Keys: routing each press to the open window or the screen, and the menu,
//! lobby, final standings and the windows on top.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use holdem::protocol::ClientMsg;

use super::{App, InfoTab, LeaveChoice, MenuItem, PAGE, Screen, cycle};
use crate::help;
use crate::keys::{self, Cmd, Place};
use crate::session::Kind;
use crate::sound::Cue;

impl App {
    pub fn on_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return;
        }
        let code = key.code;
        if self.leave.is_some() {
            self.leave_key(code);
        } else if self.show_help {
            self.help_key(code);
        } else if self.info.is_some() {
            self.info_key(code);
        } else if self.show_settings {
            self.settings_key(code);
        } else {
            match self.screen {
                Screen::Menu => self.menu_key(code),
                Screen::Setup => self.setup_key(code),
                Screen::Join => self.join_key(code),
                Screen::Choose => self.choose_key(code),
                Screen::Lobby => self.lobby_key(code),
                Screen::Table => self.table_key(code),
                Screen::Over => self.over_key(code),
                Screen::Achievements => self.achievements_key(code),
            }
        }
        // Answer at once instead of waiting for the next tick.
        self.tick();
    }

    /// The windows and commands most screens share.
    pub(super) fn common(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Help => {
                self.show_help = true;
                self.help_page = 0;
                self.help_scroll = 0;
            }
            Cmd::Log => self.show_tab(self.info_tab),
            Cmd::Settings => {
                self.show_settings = true;
                self.settings_row = 0;
            }
            Cmd::Profile => self.show_tab(InfoTab::Style),
            Cmd::Leave => self.leave = Some(0),
            _ => {}
        }
    }

    fn leave_key(&mut self, code: KeyCode) {
        let Some(selected) = self.leave else {
            return;
        };
        let n = LeaveChoice::ALL.len();
        match keys::command(Place::LeaveWindow, code) {
            Some(Cmd::Up) => self.leave = Some(cycle(selected, n, -1)),
            Some(Cmd::Down) => self.leave = Some(cycle(selected, n, 1)),
            Some(Cmd::Select) => self.choose_leave(LeaveChoice::ALL[selected]),
            Some(Cmd::MainMenu) => self.choose_leave(LeaveChoice::MainMenu),
            Some(Cmd::Quit) => self.choose_leave(LeaveChoice::Quit),
            Some(Cmd::Stay) => self.choose_leave(LeaveChoice::Stay),
            _ => {}
        }
    }

    fn choose_leave(&mut self, choice: LeaveChoice) {
        self.leave = None;
        match choice {
            LeaveChoice::MainMenu => self.return_to_menu(),
            LeaveChoice::Quit => self.quit = true,
            LeaveChoice::Stay => {}
        }
    }

    fn help_key(&mut self, code: KeyCode) {
        let pages = help::page_count();
        let max = self.help_max.get();
        match keys::command(Place::HelpWindow, code) {
            Some(Cmd::Right) => {
                self.help_page = cycle(self.help_page, pages, 1);
                self.help_scroll = 0;
            }
            Some(Cmd::Left) => {
                self.help_page = cycle(self.help_page, pages, -1);
                self.help_scroll = 0;
            }
            Some(Cmd::Down) => self.help_scroll = (self.help_scroll + 1).min(max),
            Some(Cmd::Up) => self.help_scroll = self.help_scroll.saturating_sub(1),
            Some(Cmd::PageDown) => self.help_scroll = (self.help_scroll + PAGE).min(max),
            Some(Cmd::PageUp) => self.help_scroll = self.help_scroll.saturating_sub(PAGE),
            Some(Cmd::Close) => self.show_help = false,
            _ => {}
        }
    }

    /// Opens the game info window on `tab`, which the next L opens on too.
    pub(super) fn show_tab(&mut self, tab: InfoTab) {
        self.info = Some(tab);
        self.info_tab = tab;
        if tab != InfoTab::Style {
            self.side_tab = tab;
        }
        self.log_scroll = 0;
    }

    fn info_key(&mut self, code: KeyCode) {
        let Some(tab) = self.info else {
            return;
        };
        let step = |delta: i32| {
            let i = InfoTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
            InfoTab::ALL[cycle(i, InfoTab::ALL.len(), delta)]
        };
        // The logs scroll up into the past: the offset counts lines above the newest.
        let max = self.log_max.get();
        match keys::command(Place::InfoWindow, code) {
            Some(Cmd::Right) => self.show_tab(step(1)),
            Some(Cmd::Left) => self.show_tab(step(-1)),
            Some(Cmd::Profile) => self.show_tab(InfoTab::Style),
            Some(Cmd::Up) => self.log_scroll = (self.log_scroll + 1).min(max),
            Some(Cmd::Down) => self.log_scroll = self.log_scroll.saturating_sub(1),
            Some(Cmd::PageUp) => self.log_scroll = (self.log_scroll + PAGE).min(max),
            Some(Cmd::PageDown) => self.log_scroll = self.log_scroll.saturating_sub(PAGE),
            Some(Cmd::First) => self.log_scroll = max,
            Some(Cmd::Last) => self.log_scroll = 0,
            Some(Cmd::Close) => self.info = None,
            _ => {}
        }
    }

    fn menu_key(&mut self, code: KeyCode) {
        let n = MenuItem::ALL.len();
        match keys::command(Place::Menu, code) {
            Some(Cmd::Up) => {
                self.menu_row = cycle(self.menu_row, n, -1);
                self.sound.play(Cue::Tick, 0);
            }
            Some(Cmd::Down) => {
                self.menu_row = cycle(self.menu_row, n, 1);
                self.sound.play(Cue::Tick, 0);
            }
            Some(Cmd::Select) => match MenuItem::ALL[self.menu_row] {
                item @ (MenuItem::Solo | MenuItem::Host) => {
                    self.setup.host = item == MenuItem::Host;
                    // Start below your name and symbol, which are usually right
                    // already, so that Q still goes straight back.
                    self.setup.row = 2;
                    self.screen = Screen::Setup;
                }
                MenuItem::Join => {
                    self.screen = Screen::Join;
                    self.join.status.clear();
                    self.last_discovery = None;
                }
                MenuItem::Achievements => {
                    self.screen = Screen::Achievements;
                    self.achievements_scroll = 0;
                }
                MenuItem::Settings => self.common(Cmd::Settings),
                MenuItem::Help => self.common(Cmd::Help),
                MenuItem::Quit => self.quit = true,
            },
            Some(Cmd::Quit) => self.quit = true,
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }

    fn achievements_key(&mut self, code: KeyCode) {
        let max = self.achievements_max.get();
        match keys::command(Place::Achievements, code) {
            Some(Cmd::Down) => self.achievements_scroll = (self.achievements_scroll + 1).min(max),
            Some(Cmd::Up) => self.achievements_scroll = self.achievements_scroll.saturating_sub(1),
            Some(Cmd::PageDown) => {
                self.achievements_scroll = (self.achievements_scroll + PAGE).min(max)
            }
            Some(Cmd::PageUp) => {
                self.achievements_scroll = self.achievements_scroll.saturating_sub(PAGE)
            }
            Some(Cmd::Back) => self.screen = Screen::Menu,
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }

    fn lobby_key(&mut self, code: KeyCode) {
        match keys::command(Place::Lobby, code) {
            Some(Cmd::Ready) => self.send(ClientMsg::Ready),
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }

    fn over_key(&mut self, code: KeyCode) {
        match keys::command(Place::Over, code) {
            Some(Cmd::Select) => {
                let solo = self.session.as_ref().is_some_and(|s| s.kind == Kind::Solo);
                if solo {
                    self.return_to_menu();
                } else {
                    self.send(ClientMsg::Ready);
                }
            }
            Some(cmd) => self.common(cmd),
            None => {}
        }
    }
}
