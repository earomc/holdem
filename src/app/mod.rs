//! Front-end state and keyboard handling. Nothing here draws (that is `ui`),
//! and what each key means comes from `keys.rs`.
//!
//! This module holds the state and its types; the methods are split by
//! concern: `input` routes key presses (and handles the menu, lobby, final
//! standings and windows), `setup` the setup and join screens, `table` your
//! actions at the table, `settings` the settings window and saving, and
//! `lifecycle` starting and leaving games and what happens as time passes.

use std::cell::{Cell, RefCell};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use holdem::card::Rng;
use holdem::config::{Config, SpeedConfig, TableConfig};
use holdem::game::{Rules, State};
use holdem::net::{Connection, Found};
use holdem::player::history::{self, Record};
use holdem::player::symbol::{self, Symbol};
use holdem::protocol::TableView;

use crate::help;
use crate::session::Session;
use crate::sound::Sound;

mod input;
mod lifecycle;
mod settings;
mod setup;
mod table;
#[cfg(test)]
mod tests;

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
    /// Choosing one of the tables at a server with several.
    Choose,
    Lobby,
    Table,
    Over,
}

/// The tabs of the game info window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum InfoTab {
    TableLog,
    YourHands,
    Style,
}

impl InfoTab {
    pub const ALL: [InfoTab; 3] = [InfoTab::TableLog, InfoTab::YourHands, InfoTab::Style];

    pub fn title(self) -> &'static str {
        match self {
            InfoTab::TableLog => "Table log",
            InfoTab::YourHands => "Your hands",
            InfoTab::Style => "Your style",
        }
    }
}

/// The rows of the game setup screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SetupRow {
    /// Your name at the table, typed in place (the same one the join screen uses).
    Name,
    /// Your symbol: ←→ its shape, C its colour.
    Symbol,
    /// Which of the `[[tables]]` to start from (shown when there are several).
    Table,
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

pub const SETUP_ROWS: [SetupRow; 16] = [
    SetupRow::Name,
    SetupRow::Symbol,
    SetupRow::Table,
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
    /// The chosen one of the `[[tables]]`, and how many there are.
    pub table: usize,
    pub tables: usize,
    pub seats: usize,
    pub fill_with_bots: bool,
    pub rules: Rules,
    pub fluctuation: bool,
    /// Players may turn on the win-chance hint.
    pub allow_win_chance: bool,
}

impl Setup {
    /// Rows shown for this kind of game (solo games always fill seats with
    /// bots, and with one table there is nothing to choose).
    pub fn rows(&self) -> Vec<SetupRow> {
        SETUP_ROWS
            .iter()
            .copied()
            .filter(|r| self.host || *r != SetupRow::FillWithBots)
            .filter(|r| self.tables > 1 || *r != SetupRow::Table)
            .collect()
    }

    /// Starts over from `table`'s settings.
    pub fn load(&mut self, index: usize, table: &TableConfig) {
        self.table = index;
        self.seats = table.seats;
        self.fill_with_bots = table.fill_with_bots;
        self.rules = table.rules.clone();
        self.fluctuation = table.fluctuation.enabled;
        self.allow_win_chance = table.allow_win_chance;
    }
}

/// The rows of the join screen: these, then the tables found.
pub const JOIN_NAME: usize = 0;
pub const JOIN_SYMBOL: usize = 1;
pub const JOIN_ADDRESS: usize = 2;
pub const JOIN_FOUND: usize = 3;

/// The join screen: your name, an address, and tables found on the network.
#[derive(Clone, Debug, Default)]
pub struct Join {
    pub name: String,
    pub address: String,
    /// One of `JOIN_NAME` etc., or `JOIN_FOUND` + the index of a found table.
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
    /// The settings file that was loaded, which saving updates. `None` means
    /// the defaults: saving then creates the file in your settings folder.
    pub config_path: Option<PathBuf>,
    /// Index into `MenuItem::ALL`.
    pub menu_row: usize,
    pub setup: Setup,
    pub join: Join,
    pub session: Option<Session>,
    /// Index into `config.speeds`.
    pub speed: usize,
    /// Your wish to see the win-chance hint (shown only where the table allows it).
    pub show_odds: bool,
    /// The symbol you chose (`None`: the one your name gives).
    pub symbol: Option<Symbol>,
    pub sound: Sound,
    pub raise: Option<RaisePrompt>,

    pub show_help: bool,
    pub help_page: usize,
    pub help_scroll: usize,
    /// Largest useful help scroll offset, written by the renderer.
    pub help_max: Cell<usize>,
    /// The game info window, open on this tab.
    pub info: Option<InfoTab>,
    /// The tab the game info window opens on: the one looked at last.
    pub info_tab: InfoTab,
    /// What the panel beside the table shows: the log tab looked at last.
    pub side_tab: InfoTab,
    /// Lines scrolled up from the newest entry, in the log tabs.
    pub log_scroll: usize,
    pub log_max: Cell<usize>,
    /// Where your statistics over all games are kept (`None`: nowhere).
    pub stats_file: Option<PathBuf>,
    /// Your saved statistics, read once per name: (name, record).
    saved_stats: RefCell<Option<(String, Record)>>,
    pub show_settings: bool,
    /// Index into `SettingsRow::ALL`.
    pub settings_row: usize,
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
    /// The table asked for when joining a server's table found on the network.
    join_table: Option<String>,
    /// The highlighted table on the choose screen.
    pub choose_row: usize,
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
            table: 0,
            tables: config.tables.len(),
            seats: config.tables[0].seats,
            fill_with_bots: config.tables[0].fill_with_bots,
            rules: config.tables[0].rules.clone(),
            fluctuation: config.tables[0].fluctuation.enabled,
            allow_win_chance: config.tables[0].allow_win_chance,
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
            symbol: Symbol::parse(&config.client.symbol),
            config,
            config_note: String::new(),
            config_path: None,
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
            info: None,
            info_tab: InfoTab::TableLog,
            side_tab: InfoTab::TableLog,
            log_scroll: 0,
            log_max: Cell::new(0),
            // Tests must never write to the real settings folder.
            stats_file: if cfg!(test) { None } else { history::file() },
            saved_stats: RefCell::new(None),
            show_settings: false,
            settings_row: 0,
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
            join_table: None,
            choose_row: 0,
        }
    }

    /// Shows a short message that disappears by itself after a few seconds.
    pub fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    pub fn toast_text(&self) -> Option<&str> {
        match &self.toast {
            Some((text, at))
                if at.elapsed()
                    < Duration::from_millis(u64::from(self.config.client.message_ms)) =>
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
        &self.config.speeds[self.speed]
    }

    /// The `[[tables]]` entry chosen on the setup screen.
    pub fn table(&self) -> &TableConfig {
        &self.config.tables[self.setup.table]
    }

    /// Values the help and the style window quote: the style estimate's
    /// thresholds, from the table if there is one.
    pub fn help_facts(&self) -> help::Facts {
        help::Facts {
            style_min_hands: self.config.client.style_min_hands,
            style_min_stack_bb: self
                .view()
                .map_or(self.table().style_min_stack_bb, |v| v.stats_min_stack_bb),
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
        self.show_help || self.info.is_some() || self.show_settings || self.leave.is_some()
    }

    /// Your symbol as others will see it: the chosen one, or your name's.
    pub fn shown_symbol(&self) -> Symbol {
        self.symbol
            .unwrap_or_else(|| Symbol::for_name(&self.join.name))
    }

    /// The shapes you may choose from (indices into [`symbol::GLYPHS`]).
    pub fn available_glyphs(&self) -> Vec<u8> {
        (0..symbol::BASE_GLYPHS as u8).collect()
    }

    /// Steps your symbol's shape through the ones you may use.
    pub(super) fn step_symbol_shape(&mut self, delta: i32) {
        let mut s = self.shown_symbol();
        let shapes = self.available_glyphs();
        let i = shapes.iter().position(|g| *g == s.glyph).unwrap_or(0);
        s.glyph = shapes[cycle(i, shapes.len(), delta)];
        self.symbol = Some(s);
    }

    /// Your symbol's next colour.
    pub(super) fn next_symbol_color(&mut self) {
        let mut s = self.shown_symbol();
        s.color = ((usize::from(s.color) + 1) % symbol::COLORS.len()) as u8;
        self.symbol = Some(s);
    }

    /// The name your statistics are kept under: the one you chose, not the
    /// "Mo 2" a table may have made of it.
    pub fn stats_name(&self) -> String {
        self.join.name.trim().to_string()
    }

    /// Your statistics from earlier games (read from the file once per name).
    pub fn saved_stats(&self) -> Record {
        let name = self.stats_name();
        let mut cache = self.saved_stats.borrow_mut();
        if let Some((n, r)) = cache.as_ref()
            && *n == name
        {
            return *r;
        }
        let record = self
            .stats_file
            .as_deref()
            .map(|path| history::load(path, &name))
            .unwrap_or_default();
        *cache = Some((name, record));
        record
    }

    /// Adds this game's statistics to your saved ones, once per game.
    pub(super) fn save_game_stats(&mut self) {
        let Some(session) = self.session.as_mut() else {
            return;
        };
        if session.stats_saved {
            return;
        }
        if session.view.is_none() {
            return;
        }
        session.stats_saved = true;
        let stats = session.game_stats;
        if stats.hands == 0 {
            return;
        }
        let Some(path) = self.stats_file.clone() else {
            return;
        };
        if let Err(e) = history::add_game(&path, &self.stats_name(), &stats) {
            self.notify(format!("Your statistics could not be saved: {e}"));
        }
        // Read again next time, with this game in.
        *self.saved_stats.borrow_mut() = None;
    }

    pub fn connecting(&self) -> bool {
        self.connecting.is_some()
    }
}
