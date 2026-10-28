//! Front-end state and keyboard handling. Nothing here draws; see `ui.rs` and
//! `popups.rs`. What each key means comes from `keys.rs`.

use std::cell::Cell;
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use holdem::bot;
use holdem::card::Rng;
use holdem::config::{Config, SpeedConfig};
use holdem::game::{Action, GameEvent, MAX_PLAYERS, MIN_PLAYERS, Rules, State, Street, chips_text};
use holdem::net::{self, Connection, Found, Server};
use holdem::protocol::{ClientMsg, MAX_NAME, TableView};
use holdem::table::TableSettings;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::help;
use crate::keys::{self, Cmd, Place};
use crate::session::{Kind, Session};
use crate::sound::{Cue, Sound};

/// Declares a list of choices: an enum, all its values in order, and a label each.
macro_rules! choices {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $label:expr),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Debug)]
        pub enum $name { $($variant),+ }
        impl $name {
            pub const ALL: &[$name] = &[$($name::$variant),+];
            pub fn label(self) -> &'static str {
                match self { $($name::$variant => $label),+ }
            }
        }
    };
}

choices!(
    /// The main menu.
    MenuItem {
        Solo => "Play against bots",
        Host => "Host a network game",
        Join => "Join a network game",
        Settings => "Settings",
        Help => "Help",
        Quit => "Quit",
    }
);

choices!(
    /// The settings window.
    SettingsRow {
        Speed => "Game speed",
        Volume => "Sound volume",
        WinChance => "Win-chance hint",
    }
);

choices!(
    /// The leave window.
    LeaveChoice {
        MainMenu => "Back to the main menu",
        Quit => "Quit the application",
        Stay => "Keep playing",
    }
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Menu,
    Setup,
    Join,
    Lobby,
    Table,
    Over,
}

/// The rows of the game setup screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupRow {
    /// Your name at the table, typed in place (the same one the join screen uses).
    Name,
    Seats,
    FillWithBots,
    Chips,
    Blind,
    Levels,
    HandLimit,
    Antes,
    Straddle,
    Rebuys,
    Button,
    Showdown,
    Fluctuation,
    WinChance,
}

pub const SETUP_ROWS: [SetupRow; 14] = [
    SetupRow::Name,
    SetupRow::Seats,
    SetupRow::FillWithBots,
    SetupRow::Chips,
    SetupRow::Blind,
    SetupRow::Levels,
    SetupRow::HandLimit,
    SetupRow::Antes,
    SetupRow::Straddle,
    SetupRow::Rebuys,
    SetupRow::Button,
    SetupRow::Showdown,
    SetupRow::Fluctuation,
    SetupRow::WinChance,
];

/// Table settings chosen on the setup screen.
#[derive(Clone, Debug)]
pub struct Setup {
    /// Setting up a hosted network game rather than a solo game.
    pub host: bool,
    pub row: usize,
    pub seats: usize,
    pub fill_with_bots: bool,
    pub rules: Rules,
    pub fluctuation: bool,
    /// Players may turn on the win-chance hint.
    pub allow_win_chance: bool,
}

impl Setup {
    /// Rows shown for this kind of game (solo games always fill seats with bots).
    pub fn rows(&self) -> Vec<SetupRow> {
        SETUP_ROWS
            .iter()
            .copied()
            .filter(|r| self.host || *r != SetupRow::FillWithBots)
            .collect()
    }
}

/// The join screen: your name, an address, and tables found on the network.
#[derive(Clone, Debug, Default)]
pub struct Join {
    pub name: String,
    pub address: String,
    /// 0 = name, 1 = address, 2.. = found tables.
    pub row: usize,
    pub found: Vec<Found>,
    pub status: String,
}

/// The "raise to" amount being typed by the player.
#[derive(Clone, Debug)]
pub struct RaisePrompt {
    pub text: String,
    /// The next digit typed replaces the prefilled amount instead of extending it.
    pub fresh: bool,
    pub error: Option<String>,
}

/// A cached win-chance estimate for your hand as it stands.
#[derive(Clone, Debug)]
pub struct Hint {
    /// What the estimate depends on (see [`hint_key`]).
    key: HintKey,
    pub win: f32,
    pub opponents: usize,
}

enum Outcome {
    Keep,
    Close,
    Submit(u32),
}

pub struct App {
    pub screen: Screen,
    pub config: Config,
    /// Which settings file is in use, for the main menu.
    pub config_note: String,
    /// Index into `MenuItem::ALL`.
    pub menu_row: usize,
    pub setup: Setup,
    pub join: Join,
    pub session: Option<Session>,
    /// Index into `config.game.speeds`.
    pub speed: usize,
    /// Your wish to see the win-chance hint (shown only where the table allows it).
    pub show_odds: bool,
    pub sound: Sound,
    pub raise: Option<RaisePrompt>,

    pub show_help: bool,
    pub help_page: usize,
    pub help_scroll: usize,
    /// Largest useful help scroll offset, written by the renderer.
    pub help_max: Cell<usize>,
    pub show_log: bool,
    /// Lines scrolled up from the newest log entry.
    pub log_scroll: usize,
    pub log_max: Cell<usize>,
    pub show_settings: bool,
    /// Index into `SettingsRow::ALL`.
    pub settings_row: usize,
    pub show_profile: bool,
    /// The leave window, with the highlighted choice (index into `LeaveChoice::ALL`).
    pub leave: Option<usize>,

    /// The first press of A arms the all-in; a second press pushes.
    pub all_in_armed: bool,
    toast: Option<(String, Instant)>,
    pub quit: bool,
    pub hint: Option<Hint>,
    /// When the pot last grew, to make it flash.
    pub pot_flash: Option<Instant>,
    /// The decision we already sent an answer for.
    answered: Option<u64>,
    last_pot: u32,
    my_turn: bool,
    seed: Option<u64>,
    hint_rng: Rng,
    discovery: Option<Receiver<Vec<Found>>>,
    last_discovery: Option<Instant>,
    connecting: Option<Receiver<io::Result<Connection>>>,
}

/// Lines moved by PgUp and PgDn in the help and the log.
const PAGE: usize = 10;
/// Longest address the join screen accepts, in characters.
const MAX_ADDRESS: usize = 100;
/// Longest amount the raise box accepts (9 digits always fit in a u32).
const MAX_AMOUNT_DIGITS: usize = 9;

/// What a win-chance estimate depends on: the hand, how many board cards are
/// out, and how many opponents are still in.
type HintKey = (u32, usize, usize);

/// The estimate the view calls for, if any: while the hand is being played and
/// you are still in it (on your turn or not, and also when you are all-in).
fn hint_key(v: &TableView) -> Option<HintKey> {
    let me = &v.seats[v.you?];
    let playing = matches!(v.state, State::Betting | State::Runout);
    if !playing || !me.in_hand || me.folded || me.cards.len() != 2 {
        return None;
    }
    Some((v.hand_no, v.board.len(), opponents_in(v)))
}

/// Opponents still in the hand (at least 1).
fn opponents_in(v: &TableView) -> usize {
    v.seats
        .iter()
        .enumerate()
        .filter(|(i, s)| Some(*i) != v.you && s.in_hand && !s.folded)
        .count()
        .max(1)
}

/// Moves `index` by `delta` within `0..len`, wrapping around at both ends.
fn cycle(index: usize, len: usize, delta: i32) -> usize {
    (index as i32 + delta).rem_euclid(len as i32) as usize
}

/// Moves `current` to the next value in `choices` (starting from the nearest one
/// when `current` is not in the list, for values that came from the config file).
fn step_choice(choices: &[u32], current: u32, delta: i32) -> u32 {
    match choices.iter().position(|&c| c == current) {
        Some(i) => choices[cycle(i, choices.len(), delta)],
        None => {
            let nearest = (0..choices.len())
                .min_by_key(|&i| choices[i].abs_diff(current))
                .unwrap_or(0);
            choices[nearest]
        }
    }
}

/// The total bet that raises by `frac` of the pot after calling, kept within the legal range.
pub fn pot_raise_to(view: &TableView, frac: f32) -> u32 {
    let Some(legal) = view.legal else {
        return 0;
    };
    let to = view.current_bet + ((view.pot + legal.to_call) as f32 * frac) as u32;
    to.clamp(legal.min_to, legal.max_to)
}

impl App {
    pub fn new(config: Config, seed: Option<u64>, sound: Sound) -> App {
        let mut sound = sound;
        sound.set_volume(config.client.sound.volume);
        let setup = Setup {
            host: false,
            row: 0,
            seats: config.game.seats,
            fill_with_bots: config.game.fill_with_bots,
            rules: config.game.rules.clone(),
            fluctuation: config.game.fluctuation.enabled,
            allow_win_chance: config.game.allow_win_chance,
        };
        let join = Join {
            name: config.client.resolved_name(),
            address: String::new(),
            // Start on the name, so it can be typed straight away.
            row: 0,
            ..Join::default()
        };
        App {
            screen: Screen::Menu,
            speed: config.speed_index(&config.client.speed).unwrap_or(0),
            show_odds: config.client.win_chance_hint,
            config,
            config_note: String::new(),
            menu_row: 0,
            setup,
            join,
            session: None,
            sound,
            raise: None,
            show_help: false,
            help_page: 0,
            help_scroll: 0,
            help_max: Cell::new(0),
            show_log: false,
            log_scroll: 0,
            log_max: Cell::new(0),
            show_settings: false,
            settings_row: 0,
            show_profile: false,
            leave: None,
            all_in_armed: false,
            toast: None,
            quit: false,
            hint: None,
            pot_flash: None,
            answered: None,
            last_pot: 0,
            my_turn: false,
            seed,
            hint_rng: Rng::from_entropy(),
            discovery: None,
            last_discovery: None,
            connecting: None,
        }
    }

    // ------------------------------------------------------------------ status

    /// Shows a short message that disappears by itself after a few seconds.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    pub fn toast_text(&self) -> Option<&str> {
        match &self.toast {
            Some((text, at))
                if at.elapsed() < Duration::from_millis(self.config.client.message_ms as u64) =>
            {
                Some(text.as_str())
            }
            _ => None,
        }
    }

    pub fn view(&self) -> Option<&TableView> {
        self.session.as_ref().and_then(|s| s.view.as_ref())
    }

    /// The chosen game speed. (`speed` is always a valid index: it starts from
    /// a checked configuration and only moves with `cycle`.)
    pub fn speed(&self) -> &SpeedConfig {
        &self.config.game.speeds[self.speed]
    }

    /// Values the help and the style window quote: the style estimate's
    /// thresholds, from the table if there is one.
    pub fn help_facts(&self) -> help::Facts {
        help::Facts {
            style_min_hands: self.config.client.style_min_hands,
            style_min_stack_bb: self
                .view()
                .map_or(self.config.game.style_min_stack_bb, |v| {
                    v.stats_min_stack_bb
                }),
        }
    }

    /// The table allows the win-chance hint (always true before a game).
    pub fn hints_allowed(&self) -> bool {
        match self.session.as_ref() {
            Some(s) => match (&s.view, &s.lobby) {
                (Some(v), _) => v.win_chance_allowed,
                (None, Some(l)) => l.win_chance_allowed,
                _ => true,
            },
            None => true,
        }
    }

    /// The hint is wanted and allowed.
    pub fn hints_on(&self) -> bool {
        self.show_odds && self.hints_allowed()
    }

    /// The win-chance hint to show now: wanted, allowed, and computed for
    /// exactly the hand as it stands (never an estimate from an earlier street).
    pub fn current_hint(&self) -> Option<&Hint> {
        let key = hint_key(self.view()?)?;
        self.hint
            .as_ref()
            .filter(|h| self.hints_on() && h.key == key)
    }

    pub fn volume_label(&self) -> String {
        match self.sound.unavailable() {
            Some(reason) => reason.to_string(),
            None if self.sound.volume() == 0 => "Off".to_string(),
            None => format!("{}%", self.sound.volume()),
        }
    }

    /// True while a window that pauses a solo game is open.
    pub fn overlay_open(&self) -> bool {
        self.show_help
            || self.show_log
            || self.show_settings
            || self.show_profile
            || self.leave.is_some()
    }

    pub fn connecting(&self) -> bool {
        self.connecting.is_some()
    }

    fn change_volume(&mut self, delta: i32) {
        if self.sound.unavailable().is_some() {
            return;
        }
        let step = self.config.client.sound.volume_step as i32;
        let v = (self.sound.volume() as i32 + delta * step).clamp(0, 100);
        self.sound.set_volume(v as u8);
        // A sample at the new level, so the change can be heard.
        self.sound.play(Cue::Bet, 0);
    }

    fn change_speed(&mut self, delta: i32) {
        self.speed = cycle(self.speed, self.config.game.speeds.len(), delta);
        let ms = self.speed().ms;
        if let Some(s) = self.session.as_ref()
            && s.is_owner()
        {
            s.send(ClientMsg::SetSpeed(ms));
        }
    }

    // ------------------------------------------------------------------ starting games

    fn table_settings(&self, name: &str) -> TableSettings {
        let st = &self.setup;
        let mut t = self.config.table_settings(name, self.speed().ms, st.host);
        t.seats = st.seats;
        t.fill_with_bots = !st.host || st.fill_with_bots;
        t.rules = st.rules.clone();
        t.fluctuation = st.fluctuation.then(|| self.config.fluctuation());
        t.allow_win_chance = st.allow_win_chance;
        t
    }

    fn seed(&mut self) -> u64 {
        match self.seed {
            Some(s) => s,
            None => self.hint_rng.next_u64(),
        }
    }

    fn start_game(&mut self) {
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

    fn connect_to(&mut self, address: String) {
        if self.connecting.is_some() {
            return;
        }
        let port = self.config.server.port;
        let timeout = Duration::from_secs(self.config.client.join.connect_seconds as u64);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(net::connect(&address, port, timeout));
        });
        self.connecting = Some(rx);
        self.join.status = "Connecting…".to_string();
    }

    /// Leaves the current game and shows the main menu.
    fn return_to_menu(&mut self) {
        self.session = None;
        self.screen = Screen::Menu;
        self.raise = None;
        self.hint = None;
        self.show_log = false;
        self.show_profile = false;
        self.all_in_armed = false;
    }

    // ------------------------------------------------------------------ time passes

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
                .play(c, k as u64 * self.config.client.sound.gap_ms as u64);
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
            Err(e) => self.join.status = format!("Could not connect: {}", e),
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
            t.elapsed() >= Duration::from_secs(self.config.client.join.discovery_seconds as u64)
        });
        if self.discovery.is_none() && due {
            let port = self.config.server.port;
            let wait = Duration::from_millis(self.config.client.join.discovery_wait_ms as u64);
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let _ = tx.send(net::discover(port, wait));
            });
            self.discovery = Some(rx);
            self.last_discovery = Some(Instant::now());
        }
    }

    // ------------------------------------------------------------------ input

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
        } else if self.show_log {
            self.log_key(code);
        } else if self.show_settings {
            self.settings_key(code);
        } else if self.show_profile {
            if keys::command(Place::ProfileWindow, code) == Some(Cmd::Close) {
                self.show_profile = false;
            }
        } else {
            match self.screen {
                Screen::Menu => self.menu_key(code),
                Screen::Setup => self.setup_key(code),
                Screen::Join => self.join_key(code),
                Screen::Lobby => self.lobby_key(code),
                Screen::Table => self.table_key(code),
                Screen::Over => self.over_key(code),
            }
        }
        // Answer at once instead of waiting for the next tick.
        self.tick();
    }

    /// The windows and commands most screens share.
    fn common(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::Help => {
                self.show_help = true;
                self.help_page = 0;
                self.help_scroll = 0;
            }
            Cmd::Log => {
                self.show_log = true;
                self.log_scroll = 0;
            }
            Cmd::Settings => {
                self.show_settings = true;
                self.settings_row = 0;
            }
            Cmd::Profile => self.show_profile = true,
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

    fn log_key(&mut self, code: KeyCode) {
        // The log scrolls up into the past: the offset counts lines above the newest.
        let max = self.log_max.get();
        match keys::command(Place::LogWindow, code) {
            Some(Cmd::Up) => self.log_scroll = (self.log_scroll + 1).min(max),
            Some(Cmd::Down) => self.log_scroll = self.log_scroll.saturating_sub(1),
            Some(Cmd::PageUp) => self.log_scroll = (self.log_scroll + PAGE).min(max),
            Some(Cmd::PageDown) => self.log_scroll = self.log_scroll.saturating_sub(PAGE),
            Some(Cmd::First) => self.log_scroll = max,
            Some(Cmd::Last) => self.log_scroll = 0,
            Some(Cmd::Close) => self.show_log = false,
            _ => {}
        }
    }

    fn settings_key(&mut self, code: KeyCode) {
        let n = SettingsRow::ALL.len();
        match keys::command(Place::SettingsWindow, code) {
            Some(Cmd::Up) => self.settings_row = cycle(self.settings_row, n, -1),
            Some(Cmd::Down) => self.settings_row = cycle(self.settings_row, n, 1),
            Some(Cmd::Left) => self.change_setting(-1),
            Some(Cmd::Right) => self.change_setting(1),
            Some(Cmd::Close) => self.show_settings = false,
            _ => {}
        }
    }

    fn change_setting(&mut self, delta: i32) {
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
                    // Start below the name, which is usually right already, so
                    // that Q still goes straight back.
                    self.setup.row = 1;
                    self.screen = Screen::Setup;
                }
                MenuItem::Join => {
                    self.screen = Screen::Join;
                    self.join.status.clear();
                    self.last_discovery = None;
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

    fn setup_key(&mut self, code: KeyCode) {
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

    fn join_key(&mut self, code: KeyCode) {
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
                    let address = f.address.to_string();
                    self.connect_to(address);
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
            self.connect_to(address);
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

    /// Answers the current decision, once: repeated presses (or a key held
    /// down) are ignored until the table asks again.
    fn act(&mut self, action: Action) {
        let Some(turn) = self.view().filter(|v| v.legal.is_some()).map(|v| v.turn) else {
            return;
        };
        if self.answered == Some(turn) {
            return;
        }
        self.answered = Some(turn);
        self.send(ClientMsg::Act { action, turn });
    }

    fn send(&self, msg: ClientMsg) {
        if let Some(s) = self.session.as_ref() {
            s.send(msg);
        }
    }

    fn table_key(&mut self, code: KeyCode) {
        let armed = std::mem::take(&mut self.all_in_armed);
        if self.raise.is_some() {
            self.raise_key(code);
            return;
        }
        if let Some(cmd) = keys::command(Place::Table, code) {
            return self.common(cmd);
        }
        let Some(v) = self.view() else {
            return;
        };
        if v.state == State::HandOver {
            match keys::command(Place::HandOver, code) {
                Some(Cmd::Ready) if !v.you_ready => self.send(ClientMsg::Ready),
                Some(Cmd::Rebuy) if v.rebuy_offer.is_some() => self.send(ClientMsg::Rebuy(true)),
                _ => {}
            }
            return;
        }
        let Some(legal) = v.legal else {
            return;
        };
        if self.answered == Some(v.turn) {
            // Already answered; waiting for the table.
            return;
        }
        match keys::command(Place::Turn, code) {
            Some(Cmd::Fold) => {
                if legal.can_check {
                    self.notify(format!(
                        "Checking is free, so folding is not offered. Press {} to check.",
                        keys::key(Place::Turn, Cmd::Call)
                    ));
                } else {
                    self.act(Action::Fold);
                }
            }
            Some(Cmd::Call) => self.act(Action::Call),
            Some(Cmd::Raise) => {
                if legal.can_raise {
                    self.raise = Some(RaisePrompt {
                        text: legal.min_to.to_string(),
                        fresh: true,
                        error: None,
                    });
                } else {
                    self.notify(
                        "You cannot raise here: the betting was not reopened, or nobody can respond.",
                    );
                }
            }
            Some(Cmd::AllIn) => {
                if !legal.can_go_all_in() {
                    self.notify("All-in is not possible here: you can only check or call.");
                } else if armed {
                    self.act(Action::AllIn);
                } else {
                    self.all_in_armed = true;
                }
            }
            _ => {}
        }
    }

    fn raise_key(&mut self, code: KeyCode) {
        let outcome = {
            let Some(v) = self.session.as_ref().and_then(|s| s.view.as_ref()) else {
                self.raise = None;
                return;
            };
            let Some(prompt) = self.raise.as_mut() else {
                return;
            };
            let Some(legal) = v.legal else {
                self.raise = None;
                return;
            };
            let bb = v.bb.max(1);
            let current: u32 = prompt.text.parse().unwrap_or(legal.min_to);
            let set = |prompt: &mut RaisePrompt, value: u32| {
                prompt.text = value.to_string();
                prompt.fresh = false;
                prompt.error = None;
                Outcome::Keep
            };
            match keys::command(Place::Raise, code) {
                Some(Cmd::Back) => Outcome::Close,
                Some(Cmd::Select) => match prompt.text.parse::<u32>() {
                    Err(_) => {
                        prompt.error = Some("Type an amount first.".to_string());
                        Outcome::Keep
                    }
                    Ok(x) if x < legal.min_to => {
                        prompt.error = Some(format!(
                            "The smallest raise is to {}.",
                            chips_text(legal.min_to)
                        ));
                        Outcome::Keep
                    }
                    Ok(x) if x > legal.max_to => {
                        prompt.error = Some(format!(
                            "You only have {} to bet.",
                            chips_text(legal.max_to)
                        ));
                        Outcome::Keep
                    }
                    Ok(x) => Outcome::Submit(x),
                },
                Some(Cmd::Erase) => {
                    prompt.text.pop();
                    prompt.fresh = false;
                    prompt.error = None;
                    Outcome::Keep
                }
                Some(Cmd::Up) => set(prompt, (current.saturating_add(bb)).min(legal.max_to)),
                Some(Cmd::Down) => set(prompt, current.saturating_sub(bb).max(legal.min_to)),
                Some(Cmd::MinRaise) => set(prompt, legal.min_to),
                Some(Cmd::HalfPot) => set(prompt, pot_raise_to(v, 0.5)),
                Some(Cmd::Pot) => set(prompt, pot_raise_to(v, 1.0)),
                Some(Cmd::MaxRaise) => set(prompt, legal.max_to),
                _ => {
                    if let KeyCode::Char(c) = code
                        && c.is_ascii_digit()
                    {
                        if prompt.fresh {
                            prompt.text.clear();
                            prompt.fresh = false;
                        }
                        if prompt.text.len() < MAX_AMOUNT_DIGITS {
                            prompt.text.push(c);
                        }
                        prompt.error = None;
                    }
                    Outcome::Keep
                }
            }
        };
        match outcome {
            Outcome::Keep => {}
            Outcome::Close => self.raise = None,
            Outcome::Submit(amount) => {
                self.raise = None;
                self.act(Action::RaiseTo(amount));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_step_and_snap_to_the_list() {
        let c = Config::default().client.setup;
        assert_eq!(step_choice(&c.hand_limits, 0, 1), 10);
        assert_eq!(step_choice(&c.hand_limits, 0, -1), 200);
        assert_eq!(
            step_choice(&c.starting_chips, 1700, 1),
            1500,
            "unknown values snap to the nearest"
        );
    }

    #[test]
    fn solo_setup_hides_the_bot_option() {
        let mut app = App::new(Config::default(), Some(1), Sound::new(true));
        assert!(!app.setup.rows().contains(&SetupRow::FillWithBots));
        app.setup.host = true;
        assert!(app.setup.rows().contains(&SetupRow::FillWithBots));
    }

    #[test]
    fn the_name_is_typed_on_the_setup_screen() {
        let mut config = Config::default();
        config.client.name = "Mo".to_string();
        let mut app = App::new(config, Some(1), Sound::new(true));
        let press = |app: &mut App, code| app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
        press(&mut app, KeyCode::Enter); // Play against bots
        assert_eq!(app.screen, Screen::Setup);
        assert_eq!(
            app.setup.rows()[app.setup.row],
            SetupRow::Seats,
            "starts below the name"
        );
        assert_eq!(app.join.name, "Mo", "the configured name");

        press(&mut app, KeyCode::Up);
        for c in "Qh?".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        assert_eq!(app.join.name, "MoQh?", "letters are typed, not commands");
        assert_eq!(app.screen, Screen::Setup);
        assert!(!app.show_help);

        for _ in 0..5 {
            press(&mut app, KeyCode::Backspace);
        }
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Setup, "no game without a name");
        assert_eq!(app.setup.row, 0);

        for c in "Ann".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.screen, Screen::Menu, "Q goes back on the other rows");
        assert_eq!(app.join.name, "Ann", "the join screen uses the same name");
    }

    #[test]
    fn speeds_come_from_the_config() {
        let mut config = Config::default();
        config.game.speeds.push(holdem::config::SpeedConfig {
            name: "Ludicrous".to_string(),
            ms: 40,
        });
        config.client.speed = "ludicrous".to_string();
        let mut app = App::new(config, Some(1), Sound::new(true));
        assert_eq!(app.speed().name, "Ludicrous");
        assert_eq!(app.speed().ms, 40);
        app.change_speed(1);
        assert_eq!(app.speed().name, "Slow", "the list wraps around");
        app.change_speed(-1);
        assert_eq!(app.speed().name, "Ludicrous");
    }

    #[test]
    fn a_decision_is_answered_only_once() {
        let mut config = Config::default();
        config.game.seats = 2;
        let mut app = App::new(config, Some(5), Sound::new(true));
        app.setup.seats = 2;
        app.start_game();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        // Wait for our turn, then press C twice as fast as possible.
        loop {
            assert!(std::time::Instant::now() < deadline, "never our turn");
            app.tick();
            if app.view().is_some_and(|v| v.legal.is_some()) {
                break;
            }
            if app
                .view()
                .is_some_and(|v| v.state == State::HandOver && !v.you_ready)
            {
                app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let turn = app.view().unwrap().turn;
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        app.on_key(key);
        app.on_key(key);
        assert_eq!(app.answered, Some(turn));
        // Whatever happens next, our second press never acted on a later decision:
        // the next view that asks us again carries a different turn.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            app.tick();
            std::thread::sleep(Duration::from_millis(5));
            if let Some(v) = app.view()
                && v.legal.is_some()
                && v.turn != turn
            {
                let me = v.you.unwrap();
                assert!(
                    v.seats[me].last_action.is_empty() || v.street == holdem::game::Street::PreFlop,
                    "nothing was done for us on the new street: {:?}",
                    v.seats[me].last_action
                );
                break;
            }
        }
    }

    #[test]
    fn the_join_screen_starts_on_the_name() {
        let app = App::new(Config::default(), Some(1), Sound::new(true));
        assert_eq!(app.join.row, 0);
    }

    #[test]
    fn the_hint_cannot_be_turned_on_where_it_is_not_allowed() {
        let mut config = Config::default();
        config.game.allow_win_chance = false;
        let mut app = App::new(config, Some(1), Sound::new(true));
        app.setup.allow_win_chance = false;
        app.start_game();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while app.view().is_none() {
            assert!(std::time::Instant::now() < deadline);
            app.tick();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(app.show_odds, "the wish is kept");
        assert!(!app.hints_on(), "but the table says no");
        app.settings_row = SettingsRow::ALL
            .iter()
            .position(|&r| r == SettingsRow::WinChance)
            .unwrap();
        app.change_setting(1);
        assert!(app.show_odds, "toggling is refused");
        assert!(
            app.toast_text()
                .is_some_and(|t| t.contains("does not allow"))
        );
    }
}
