//! The optional configuration file, `holdem.toml`.
//!
//! It is looked for in the current folder, then next to the program; a path
//! can also be given with `--config`. Every setting has a default, so the file
//! may contain only what you want to change. `holdem config write` writes a
//! commented file with all the defaults ([`template`]).
//!
//! The file has three parts, by who reads them:
//!
//! * `[client]`: only your own screen and keys (name, sound, the values
//!   offered on the setup screen, joining other tables).
//! * `[game]`: how a table run by this program plays: seats, bots, speeds,
//!   rules. Used for solo games, hosted games and `holdem serve` alike.
//! * `[server]`: network play only: table name, port, time limits.
//!
//! What is configurable: everything about the game's design. What is not: the
//! internals of algorithms (how bots weigh their decisions, the hand evaluator,
//! the measured style reference values, the sound synthesis), where a number
//! only makes sense together with the code around it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bot;
use crate::game::{DEFAULT_BOT_SAMPLES, MAX_PLAYERS, MIN_PLAYERS, Rules};
use crate::protocol::DEFAULT_PORT;
use crate::stats::{MIN_HANDS, MIN_STACK_BB};
use crate::table::{Fluctuation, TableSettings};

pub const FILE_NAME: &str = "holdem.toml";

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub client: ClientConfig,
    pub game: GameConfig,
    pub server: ServerConfig,
}

// ------------------------------------------------------------------ [client]

/// Settings that only affect your own screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientConfig {
    /// Your name at the table; empty means your computer's login name.
    pub name: String,
    /// Name of the speed to start with (one of `[[game.speeds]]`).
    pub speed: String,
    /// Show the win-chance hint where the table allows it.
    pub win_chance_hint: bool,
    /// Simulated deals for the win-chance hint.
    pub hint_samples: u32,
    /// How long status messages stay on screen.
    pub message_ms: u32,
    /// How long the pot flashes when it grows.
    pub pot_flash_ms: u32,
    /// Hand log lines kept for the log window.
    pub log_lines: usize,
    /// Counted hands before your playing style is estimated.
    pub style_min_hands: u32,
    pub sound: SoundConfig,
    pub setup: SetupChoices,
    pub join: JoinConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SoundConfig {
    /// 0 (off) to 100.
    pub volume: u8,
    /// How much one press changes the volume.
    pub volume_step: u8,
    /// Pause between sounds triggered together.
    pub gap_ms: u32,
}

/// The values offered on the setup screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SetupChoices {
    pub starting_chips: Vec<u32>,
    pub small_blinds: Vec<u32>,
    /// 0 = the blinds never go up.
    pub hands_per_level: Vec<u32>,
    /// 0 = no limit.
    pub hand_limits: Vec<u32>,
    /// 0 = no rebuys.
    pub rebuy_levels: Vec<u32>,
}

/// Joining other people's tables.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct JoinConfig {
    /// Give up connecting to a table after this long.
    pub connect_seconds: u32,
    /// How often the join screen looks for tables on the local network.
    pub discovery_seconds: u32,
    /// How long it waits for tables to answer.
    pub discovery_wait_ms: u32,
}

// ------------------------------------------------------------------ [game]

/// How a table run by this program plays (solo, hosted or `holdem serve`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GameConfig {
    /// Seats at the table, 2 to 9.
    pub seats: usize,
    /// Seats that people do not take are given to bots.
    pub fill_with_bots: bool,
    /// Players may turn on the win-chance hint.
    pub allow_win_chance: bool,
    /// Pause between the streets of an all-in run-out, in percent of the game speed.
    pub runout_pace_percent: u32,
    /// Hands started with fewer big blinds do not count for the style estimate.
    pub style_min_stack_bb: u32,
    /// Game speeds, slowest first. Any number, with any names.
    pub speeds: Vec<SpeedConfig>,
    pub bots: BotConfig,
    /// Not `deny_unknown_fields` (rules also travel over the network); typos are
    /// caught by [`Config::parse`] instead.
    pub rules: Rules,
    pub fluctuation: FluctuationConfig,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeedConfig {
    pub name: String,
    /// Milliseconds between bot moves.
    pub ms: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BotConfig {
    /// Names the bots are given (numbers are added if there are too few).
    pub names: Vec<String>,
    /// Simulated deals per bot decision: more plays slightly better and slower.
    pub thinking_samples: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct FluctuationConfig {
    /// Bots come and go between hands.
    pub enabled: bool,
    /// Chance per hand that a given bot leaves (0 to 1).
    pub leave_chance: f64,
    /// Chance per hand that an empty seat gets a new bot (0 to 1).
    pub join_chance: f64,
}

// ------------------------------------------------------------------ [server]

/// Network play: hosting from the menu and `holdem serve`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Name shown to people looking for a game.
    pub name: String,
    /// Port to listen on; also the port the join screen assumes and searches.
    pub port: u16,
    /// Speed of `holdem serve` (one of `[[game.speeds]]`).
    pub speed: String,
    /// Seconds a person has to act when several people play (0 = no limit).
    pub turn_seconds: u32,
    /// Seconds after a hand before the next one starts by itself (0 = wait for everyone).
    pub next_hand_seconds: u32,
    /// Seconds before a disconnected player's turn is folded for them.
    pub away_seconds: u32,
}

// ------------------------------------------------------------------ defaults

/// The speed both the client and the server start with.
const DEFAULT_SPEED: &str = "Normal";

fn speed(name: &str, ms: u32) -> SpeedConfig {
    SpeedConfig {
        name: name.to_string(),
        ms,
    }
}

impl Default for ClientConfig {
    fn default() -> ClientConfig {
        ClientConfig {
            name: String::new(),
            speed: DEFAULT_SPEED.to_string(),
            win_chance_hint: true,
            hint_samples: 1500,
            message_ms: 3000,
            pot_flash_ms: 450,
            log_lines: 1000,
            style_min_hands: MIN_HANDS,
            sound: SoundConfig::default(),
            setup: SetupChoices::default(),
            join: JoinConfig::default(),
        }
    }
}

impl Default for SoundConfig {
    fn default() -> SoundConfig {
        SoundConfig {
            volume: 60,
            volume_step: 10,
            gap_ms: 110,
        }
    }
}

impl Default for SetupChoices {
    fn default() -> SetupChoices {
        SetupChoices {
            starting_chips: vec![1000, 1500, 2500, 5000, 10000],
            small_blinds: vec![5, 10, 25, 50, 100],
            hands_per_level: vec![0, 5, 8, 12, 20],
            hand_limits: vec![0, 10, 25, 50, 100, 200],
            rebuy_levels: vec![0, 1, 2, 3, 5],
        }
    }
}

impl Default for JoinConfig {
    fn default() -> JoinConfig {
        JoinConfig {
            connect_seconds: 5,
            discovery_seconds: 3,
            discovery_wait_ms: 700,
        }
    }
}

impl Default for GameConfig {
    fn default() -> GameConfig {
        GameConfig {
            seats: 6,
            fill_with_bots: true,
            allow_win_chance: true,
            runout_pace_percent: 150,
            style_min_stack_bb: MIN_STACK_BB,
            speeds: vec![
                speed("Slow", 1100),
                speed(DEFAULT_SPEED, 650),
                speed("Fast", 220),
            ],
            bots: BotConfig::default(),
            rules: Rules::default(),
            fluctuation: FluctuationConfig::default(),
        }
    }
}

impl Default for BotConfig {
    fn default() -> BotConfig {
        BotConfig {
            names: bot::DEFAULT_NAMES.iter().map(|s| s.to_string()).collect(),
            thinking_samples: DEFAULT_BOT_SAMPLES,
        }
    }
}

impl Default for FluctuationConfig {
    fn default() -> FluctuationConfig {
        FluctuationConfig {
            enabled: false,
            leave_chance: 0.04,
            join_chance: 0.25,
        }
    }
}

impl Default for ServerConfig {
    fn default() -> ServerConfig {
        ServerConfig {
            name: "Hold'em table".to_string(),
            port: DEFAULT_PORT,
            speed: DEFAULT_SPEED.to_string(),
            turn_seconds: 30,
            next_hand_seconds: 8,
            away_seconds: 1,
        }
    }
}

impl ClientConfig {
    /// The name to play under: the configured one, or else the computer's login
    /// name, or else "Player".
    pub fn resolved_name(&self) -> String {
        let configured = self.name.trim();
        let name = if configured.is_empty() {
            std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .unwrap_or_default()
        } else {
            configured.to_string()
        };
        crate::protocol::clean_name(&name)
    }
}

/// What every setting means, by its dotted path: the single source of the
/// comments in the settings file. Boolean settings say what `true` and `false`
/// do, each on its own line. (Tests check that every setting is described and
/// that nothing described is missing from the settings.)
const DOCS: &[(&str, &str)] = &[
    ("client", "Only your own screen and keys."),
    (
        "client.name",
        "Your name at the table. Empty means your computer's login name.",
    ),
    (
        "client.speed",
        "The game speed to start with: one of the names in [[game.speeds]].",
    ),
    (
        "client.win_chance_hint",
        "The win-chance hint, where the table allows it.\n\
         true: shown while you are in a hand\n\
         false: hidden (switch it on in the settings, key S)",
    ),
    (
        "client.hint_samples",
        "Simulated deals behind the win-chance hint.",
    ),
    (
        "client.message_ms",
        "How long status messages stay on screen, in milliseconds.",
    ),
    (
        "client.pot_flash_ms",
        "How long the pot flashes when it grows, in milliseconds.",
    ),
    (
        "client.log_lines",
        "Hand log lines kept for the log window.",
    ),
    (
        "client.style_min_hands",
        "Counted hands before your playing style is estimated.",
    ),
    ("client.sound", "Sound effects."),
    ("client.sound.volume", "0 (off) to 100."),
    (
        "client.sound.volume_step",
        "How much one press changes the volume in the settings.",
    ),
    (
        "client.sound.gap_ms",
        "Pause between sounds that happen together, in milliseconds.",
    ),
    (
        "client.setup",
        "The values offered on the setup screen. 0 means \"never\", \"no limit\" or \"off\".",
    ),
    ("client.setup.starting_chips", "Starting stacks."),
    (
        "client.setup.small_blinds",
        "Starting small blinds (the big blind is twice as much).",
    ),
    ("client.setup.hands_per_level", "Hands per blind level."),
    ("client.setup.hand_limits", "Hand limits."),
    (
        "client.setup.rebuy_levels",
        "How many blind levels rebuys stay open.",
    ),
    ("client.join", "Joining other people's tables."),
    (
        "client.join.connect_seconds",
        "Give up connecting to a table after this many seconds.",
    ),
    (
        "client.join.discovery_seconds",
        "How often the join screen looks for tables on your network, in seconds.",
    ),
    (
        "client.join.discovery_wait_ms",
        "How long it waits for answers, in milliseconds.",
    ),
    (
        "game",
        "How tables run by this program play: solo games, games you host, and\n\
         `holdem serve`. At someone else's table, that table's settings apply.",
    ),
    ("game.seats", "Seats at the table, 2 to 9."),
    (
        "game.fill_with_bots",
        "Seats that no person takes.\n\
         true: given to bots\n\
         false: left empty (in a network game only people play)",
    ),
    (
        "game.allow_win_chance",
        "Whether players may use the win-chance hint.\n\
         true: allowed\n\
         false: not allowed; it cannot be switched on",
    ),
    (
        "game.runout_pace_percent",
        "Pause between the streets of an all-in run-out, in percent of the game speed.",
    ),
    (
        "game.style_min_stack_bb",
        "Hands started with fewer big blinds than this do not count for the style\n\
         estimate (short stacks must play push-or-fold whatever their style).",
    ),
    (
        "game.speeds",
        "Game speeds, slowest first: as many as you like, each with a name and the\n\
         milliseconds between bot moves (20 to 10000).",
    ),
    ("game.bots", "The computer players."),
    (
        "game.bots.names",
        "Names given to the bots; numbers are added if there are too few.",
    ),
    (
        "game.bots.thinking_samples",
        "Simulated deals per bot decision: more plays slightly better but slower.",
    ),
    ("game.rules", "The rules."),
    (
        "game.rules.starting_chips",
        "Every player starts with this many chips.",
    ),
    (
        "game.rules.small_blind",
        "The small blind at the start; the big blind is twice as much.",
    ),
    (
        "game.rules.hands_per_level",
        "Hands per blind level; 0 means the blinds never go up.",
    ),
    (
        "game.rules.blind_schedule",
        "The small blind at each level, in percent of the starting small blind.",
    ),
    (
        "game.rules.blind_growth_percent",
        "After the schedule's last level, each level is this many percent of the one before.",
    ),
    (
        "game.rules.hand_limit",
        "The game ends after this many hands; 0 means no limit.",
    ),
    (
        "game.rules.antes",
        "Antes.\n\
         true: every player pays an ante each hand\n\
         false: no antes",
    ),
    (
        "game.rules.ante_divisor",
        "The ante is the big blind divided by this (at least 1 chip).",
    ),
    (
        "game.rules.straddle",
        "Straddle.\n\
         true: the player left of the big blind posts a straddle and acts last before the flop\n\
         false: no straddle",
    ),
    (
        "game.rules.straddle_multiplier",
        "The straddle, in big blinds (2 or more).",
    ),
    (
        "game.rules.straddle_min_players",
        "Fewest players at the table for a straddle (3 to 9).",
    ),
    (
        "game.rules.rebuy_levels",
        "Busted players may buy back in while the blind level is at most this; 0 means no rebuys.",
    ),
    (
        "game.rules.rebuy_chips",
        "Chips a rebuy gives; 0 means the starting stack.",
    ),
    (
        "game.rules.dead_button",
        "Button rule.\n\
         true: dead button: the big blind moves exactly one player each hand, and the \
         small blind and button follow it, even onto empty seats\n\
         false: moving button: the button simply moves to the next player",
    ),
    (
        "game.rules.muck_losers",
        "Showdown.\n\
         true: players who cannot win may throw their cards away unseen\n\
         false: everyone still in shows their cards",
    ),
    ("game.fluctuation", "Bots coming and going between hands."),
    (
        "game.fluctuation.enabled",
        "Players come and go.\n\
         true: bots sometimes leave and new bots sit down in empty seats\n\
         false: the bots stay until they are knocked out",
    ),
    (
        "game.fluctuation.leave_chance",
        "Chance per hand that a given bot leaves (0 to 1).",
    ),
    (
        "game.fluctuation.join_chance",
        "Chance per hand that an empty seat gets a new bot (0 to 1).",
    ),
    (
        "server",
        "Network play: hosting from the menu, and `holdem serve`.",
    ),
    (
        "server.name",
        "The table's name, shown to people looking for a game.",
    ),
    (
        "server.port",
        "Port to listen on. The join screen also assumes and searches this port.",
    ),
    (
        "server.speed",
        "The game speed of `holdem serve`: one of the names in [[game.speeds]].\n\
         (When you host from the menu, your own speed setting is used.)",
    ),
    (
        "server.turn_seconds",
        "Seconds a person has to act when several people play; 0 means no limit.",
    ),
    (
        "server.next_hand_seconds",
        "Seconds after a hand before the next one starts by itself; 0 means wait for everyone.",
    ),
    (
        "server.away_seconds",
        "Seconds before a disconnected player's turn is folded for them.",
    ),
];

fn doc(path: &str) -> &'static str {
    DOCS.iter()
        .find(|(p, _)| *p == path)
        .map(|(_, d)| *d)
        .unwrap_or("")
}

/// Longest line in the written file (values and comments are wrapped).
const WRAP: usize = 88;

/// `first` followed by `parts` joined with ", ", wrapped at [`WRAP`] columns,
/// with continuation lines starting with `indent`; `close` ends the last line.
fn wrap_list(first: &str, parts: &[String], indent: &str, close: &str) -> String {
    let mut out = String::new();
    let mut line = first.to_string();
    for (i, part) in parts.iter().enumerate() {
        let piece = if i + 1 < parts.len() {
            format!("{},", part)
        } else {
            format!("{}{}", part, close)
        };
        let starts_line = line.ends_with('[') || line == indent;
        let candidate = if starts_line {
            format!("{}{}", line, piece)
        } else {
            format!("{} {}", line, piece)
        };
        if candidate.chars().count() > WRAP && !starts_line {
            out.push_str(&line);
            out.push('\n');
            line = format!("{}{}", indent, piece);
        } else {
            line = candidate;
        }
    }
    if parts.is_empty() {
        line.push_str(close);
    }
    out.push_str(&line);
    out
}

/// `prefix` + the value in TOML syntax, wrapping long arrays.
fn render_value(prefix: &str, value: &toml::Value, indent: &str) -> String {
    match value {
        toml::Value::Array(items) => {
            let parts: Vec<String> = items.iter().map(|v| v.to_string()).collect();
            wrap_list(&format!("{}[", prefix), &parts, indent, "]")
        }
        other => format!("{}{}", prefix, other),
    }
}

/// Writes already formatted lines as comments.
fn raw_comment(out: &mut String, text: &str) {
    for line in text.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
}

/// Writes `text` as comment lines, word-wrapped to [`WRAP`] columns.
/// Lines that start with `true:` or `false:` continue indented under their text.
fn comment(out: &mut String, text: &str) {
    for line in text.lines() {
        let indent = if line.starts_with("true: ") {
            "      "
        } else if line.starts_with("false: ") {
            "       "
        } else {
            ""
        };
        let mut current = String::from("#");
        for word in line.split(' ') {
            let first = current == "#";
            if !first && current.chars().count() + 1 + word.chars().count() > WRAP {
                out.push_str(&current);
                out.push('\n');
                current = format!("# {}", indent);
                current.push_str(word);
            } else {
                current.push(' ');
                current.push_str(word);
            }
        }
        out.push_str(current.trim_end());
        out.push('\n');
    }
}

fn is_table_array(v: &toml::Value) -> bool {
    matches!(v, toml::Value::Array(a) if !a.is_empty() && a.iter().all(|x| x.is_table()))
}

fn render_table(out: &mut String, path: &str, table: &toml::Table, defaults: &toml::Table) {
    let join = |key: &str| {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{}.{}", path, key)
        }
    };
    let default_of = |key: &str| {
        defaults
            .get(key)
            .cloned()
            .unwrap_or(toml::Value::Boolean(false))
    };
    // Plain settings first, then arrays of tables, then sub-tables (TOML needs that order).
    for (key, value) in table {
        if value.is_table() || is_table_array(value) {
            continue;
        }
        out.push('\n');
        comment(out, doc(&join(key)));
        let default_line = render_value("Default: ", &default_of(key), "          ");
        raw_comment(out, &default_line);
        out.push_str(&render_value(
            &format!("{} = ", key),
            value,
            &" ".repeat(key.len() + 4),
        ));
        out.push('\n');
    }
    for (key, value) in table {
        let toml::Value::Array(items) = value else {
            continue;
        };
        if !is_table_array(value) {
            continue;
        }
        let full = join(key);
        out.push('\n');
        comment(out, doc(&full));
        raw_comment(
            out,
            &render_value("Default: ", &default_of(key), "          "),
        );
        for item in items {
            if let toml::Value::Table(t) = item {
                out.push_str(&format!("[[{}]]\n", full));
                for (k, v) in t {
                    out.push_str(&format!("{} = {}\n", k, v));
                }
                out.push('\n');
            }
        }
    }
    for (key, value) in table {
        let toml::Value::Table(sub) = value else {
            continue;
        };
        let full = join(key);
        let empty = toml::Table::new();
        let sub_defaults = match defaults.get(key) {
            Some(toml::Value::Table(t)) => t,
            _ => &empty,
        };
        out.push('\n');
        if !full.contains('.') {
            out.push_str(&"#".repeat(WRAP));
            out.push('\n');
        }
        out.push_str(&format!("[{}]\n", full));
        comment(out, doc(&full));
        render_table(out, &full, sub, sub_defaults);
    }
}

/// A complete settings file with `config`'s values, every setting explained,
/// and each one's default noted (as a comment, rendered from
/// [`Config::default`], so it can never be out of date).
pub fn render(config: &Config) -> String {
    let current = toml::Table::try_from(config).expect("settings convert to TOML");
    let defaults = toml::Table::try_from(Config::default()).expect("settings convert to TOML");
    let mut out = String::new();
    comment(
        &mut out,
        "holdem settings. Every line is optional: delete anything you do not change,\n\
         and missing settings keep their defaults (which can improve in newer versions).\n\
         \n\
         [client]  your own screen and keys only\n\
         [game]    how tables run by this program play: solo games, games you host,\n\
         \x20         and `holdem serve`\n\
         [server]  network play: hosting and `holdem serve`",
    );
    render_table(&mut out, "", &current, &defaults);
    out
}

/// The settings in `config` that differ from the defaults, as dotted names
/// (a list or `[[table array]]` counts as one setting).
pub fn differences(config: &Config) -> Vec<String> {
    fn walk(prefix: &str, current: &toml::Table, defaults: &toml::Table, out: &mut Vec<String>) {
        for (key, value) in current {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };
            match (value, defaults.get(key)) {
                (toml::Value::Table(sub), Some(toml::Value::Table(d))) => walk(&path, sub, d, out),
                (v, d) if Some(v) != d => out.push(path),
                _ => {}
            }
        }
    }
    let current = toml::Table::try_from(config).expect("settings convert to TOML");
    let defaults = toml::Table::try_from(Config::default()).expect("settings convert to TOML");
    let mut out = Vec::new();
    walk("", &current, &defaults, &mut out);
    out
}

/// The file `holdem config write` writes: every setting at its default.
pub fn template() -> String {
    render(&Config::default())
}

/// Settings in `[game.rules]` that the file may contain (Rules itself ignores
/// unknown fields, because it also travels over the network).
fn check_rule_keys(text: &str) -> Result<(), String> {
    let value: toml::Table = toml::from_str(text).map_err(|e| e.to_string())?;
    let Some(rules) = value
        .get("game")
        .and_then(|g| g.get("rules"))
        .and_then(|r| r.as_table())
    else {
        return Ok(());
    };
    let known = toml::Table::try_from(Rules::default()).map_err(|e| e.to_string())?;
    for key in rules.keys() {
        if !known.contains_key(key) {
            let mut names: Vec<&str> = known.keys().map(|s| s.as_str()).collect();
            names.sort();
            return Err(format!(
                "unknown setting `{}` in [game.rules]; the settings there are: {}",
                key,
                names.join(", ")
            ));
        }
    }
    Ok(())
}

fn check_list(name: &str, values: &[u32], allow_zero: bool) -> Result<(), String> {
    if values.is_empty() {
        return Err(format!(
            "client.setup.{} must list at least one value",
            name
        ));
    }
    if !allow_zero && values.contains(&0) {
        return Err(format!("client.setup.{} must not contain 0", name));
    }
    Ok(())
}

impl Config {
    /// Parses a configuration file's text.
    pub fn parse(text: &str) -> Result<Config, String> {
        let config: Config = toml::from_str(text).map_err(|e| {
            format!(
                "{}\n(A file written by an older holdem may use old setting names: \
                 `holdem config write new.toml` writes the current format.)",
                e
            )
        })?;
        check_rule_keys(text)?;
        config.check()?;
        Ok(config)
    }

    fn check(&self) -> Result<(), String> {
        let g = &self.game;
        if g.speeds.is_empty() {
            return Err("at least one [[game.speeds]] entry is needed".to_string());
        }
        for s in &g.speeds {
            if s.name.trim().is_empty() {
                return Err("every speed needs a name".to_string());
            }
            if !(20..=10_000).contains(&s.ms) {
                return Err(format!(
                    "speed \"{}\": ms must be between 20 and 10000",
                    s.name
                ));
            }
        }
        for (i, a) in g.speeds.iter().enumerate() {
            if g.speeds[..i]
                .iter()
                .any(|b| b.name.eq_ignore_ascii_case(&a.name))
            {
                return Err(format!("the speed name \"{}\" is used twice", a.name));
            }
        }
        let names: Vec<&str> = g.speeds.iter().map(|s| s.name.as_str()).collect();
        for (setting, value) in [
            ("client.speed", &self.client.speed),
            ("server.speed", &self.server.speed),
        ] {
            if self.speed_index(value).is_none() {
                return Err(format!(
                    "{} \"{}\" is not one of the speeds: {}",
                    setting,
                    value,
                    names.join(", ")
                ));
            }
        }
        let c = &self.client;
        if c.hint_samples == 0 || c.hint_samples > 100_000 {
            return Err("client.hint_samples must be between 1 and 100000".to_string());
        }
        if c.log_lines == 0 {
            return Err("client.log_lines must be at least 1".to_string());
        }
        if c.style_min_hands == 0 {
            return Err("client.style_min_hands must be at least 1".to_string());
        }
        if c.sound.volume > 100 || c.sound.volume_step == 0 || c.sound.volume_step > 100 {
            return Err(
                "client.sound.volume must be 0 to 100 and volume_step 1 to 100".to_string(),
            );
        }
        let s = &c.setup;
        check_list("starting_chips", &s.starting_chips, false)?;
        check_list("small_blinds", &s.small_blinds, false)?;
        check_list("hands_per_level", &s.hands_per_level, true)?;
        check_list("hand_limits", &s.hand_limits, true)?;
        check_list("rebuy_levels", &s.rebuy_levels, true)?;
        if c.join.connect_seconds == 0 || c.join.discovery_seconds == 0 {
            return Err("client.join times must be at least 1 second".to_string());
        }
        if !(MIN_PLAYERS..=MAX_PLAYERS).contains(&g.seats) {
            return Err(format!(
                "game.seats must be between {} and {}",
                MIN_PLAYERS, MAX_PLAYERS
            ));
        }
        if g.bots.names.iter().any(|n| n.trim().is_empty()) {
            return Err("game.bots.names must not contain empty names".to_string());
        }
        if g.bots.thinking_samples == 0 || g.bots.thinking_samples > 100_000 {
            return Err("game.bots.thinking_samples must be between 1 and 100000".to_string());
        }
        let r = &g.rules;
        if r.starting_chips == 0 || r.small_blind == 0 {
            return Err("game.rules.starting_chips and small_blind must be above 0".to_string());
        }
        if r.blind_schedule.is_empty() || r.blind_schedule.contains(&0) {
            return Err(
                "game.rules.blind_schedule needs at least one level, none of them 0".to_string(),
            );
        }
        if r.blind_growth_percent < 100 {
            return Err("game.rules.blind_growth_percent must be at least 100".to_string());
        }
        if r.ante_divisor == 0 {
            return Err("game.rules.ante_divisor must be at least 1".to_string());
        }
        if r.straddle_multiplier < 2 {
            return Err("game.rules.straddle_multiplier must be at least 2".to_string());
        }
        if !(3..=MAX_PLAYERS).contains(&r.straddle_min_players) {
            return Err(format!(
                "game.rules.straddle_min_players must be between 3 and {}",
                MAX_PLAYERS
            ));
        }
        let f = &g.fluctuation;
        if !(0.0..=1.0).contains(&f.leave_chance) || !(0.0..=1.0).contains(&f.join_chance) {
            return Err("game.fluctuation chances must be between 0 and 1".to_string());
        }
        Ok(())
    }

    /// Position of a speed in `game.speeds`, by name (case does not matter).
    pub fn speed_index(&self, name: &str) -> Option<usize> {
        self.game
            .speeds
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The settings file holdem uses: `path` if given (it must then exist),
    /// otherwise the first of `holdem.toml` in the current folder and
    /// `holdem.toml` next to the program that exists. `None` means the defaults.
    pub fn locate(path: Option<&Path>) -> Option<PathBuf> {
        if let Some(p) = path {
            return Some(p.to_path_buf());
        }
        let mut candidates = vec![PathBuf::from(FILE_NAME)];
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            candidates.push(dir.join(FILE_NAME));
        }
        candidates.into_iter().find(|c| c.is_file())
    }

    /// Reads and checks one settings file.
    pub fn read(path: &Path) -> Result<Config, String> {
        let text =
            std::fs::read_to_string(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        Config::parse(&text).map_err(|e| format!("{}: {}", path.display(), e))
    }

    /// Loads the settings from the file [`Config::locate`] picks, or the
    /// defaults if there is none. Returns the file that was used, if any.
    pub fn load(path: Option<&Path>) -> Result<(Config, Option<PathBuf>), String> {
        match Config::locate(path) {
            Some(file) => Ok((Config::read(&file)?, Some(file))),
            None => Ok((Config::default(), None)),
        }
    }

    pub fn fluctuation(&self) -> Fluctuation {
        Fluctuation {
            leave_chance: self.game.fluctuation.leave_chance as f32,
            join_chance: self.game.fluctuation.join_chance as f32,
        }
    }

    /// Table settings from this file, for a table running at `speed_ms`.
    pub fn table_settings(&self, name: &str, speed_ms: u32, network: bool) -> TableSettings {
        let g = &self.game;
        TableSettings {
            name: name.to_string(),
            seats: g.seats,
            fill_with_bots: g.fill_with_bots,
            rules: g.rules.clone(),
            fluctuation: g.fluctuation.enabled.then(|| self.fluctuation()),
            bot_names: g.bots.names.clone(),
            speed_ms,
            turn_seconds: if network { self.server.turn_seconds } else { 0 },
            next_hand_seconds: if network {
                self.server.next_hand_seconds
            } else {
                0
            },
            away_seconds: self.server.away_seconds,
            runout_pace_percent: g.runout_pace_percent,
            allow_win_chance: g.allow_win_chance,
            bot_samples: g.bots.thinking_samples,
            stats_min_stack_bb: g.style_min_stack_bb,
        }
    }

    /// The speed called `name`, or the first speed if there is none by that
    /// name. (A checked configuration has at least one speed, and its
    /// `client.speed` and `server.speed` always name one.)
    pub fn speed(&self, name: &str) -> &SpeedConfig {
        &self.game.speeds[self.speed_index(name).unwrap_or(0)]
    }

    /// Table settings for `holdem serve`.
    pub fn server_table(&self) -> TableSettings {
        self.table_settings(&self.server.name, self.speed(&self.server.speed).ms, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn differences_name_the_changed_settings() {
        assert!(differences(&Config::default()).is_empty());
        let c = Config::parse(
            "[game.rules]\nantes = true\n[client.sound]\nvolume = 5\n\
             [[game.speeds]]\nname = \"Only\"\nms = 300\n[client]\nspeed = \"Only\"\n\
             [server]\nspeed = \"only\"",
        )
        .unwrap();
        assert_eq!(
            differences(&c),
            [
                "client.speed",
                "client.sound.volume",
                "game.speeds",
                "game.rules.antes",
                "server.speed"
            ]
        );
    }

    /// Every dotted path of the default settings: sections and settings.
    fn paths() -> Vec<(String, toml::Value)> {
        fn walk(prefix: &str, t: &toml::Table, out: &mut Vec<(String, toml::Value)>) {
            for (k, v) in t {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                out.push((p.clone(), v.clone()));
                if let toml::Value::Table(sub) = v {
                    walk(&p, sub, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(
            "",
            &toml::Table::try_from(Config::default()).unwrap(),
            &mut out,
        );
        out
    }

    #[test]
    fn the_written_file_is_exactly_the_defaults() {
        assert_eq!(Config::parse(&template()).unwrap(), Config::default());
    }

    #[test]
    fn every_setting_is_described_and_booleans_say_what_each_value_does() {
        let paths = paths();
        for (path, value) in &paths {
            let d = doc(path);
            assert!(!d.is_empty(), "{} has no description", path);
            if value.is_bool() {
                assert!(
                    d.lines().any(|l| l.starts_with("true:"))
                        && d.lines().any(|l| l.starts_with("false:")),
                    "{} does not say what true and false do",
                    path
                );
            }
        }
        for (path, _) in DOCS {
            assert!(
                paths.iter().any(|(p, _)| p == path),
                "{} is described but is not a setting",
                path
            );
        }
    }

    #[test]
    fn the_file_shows_current_values_and_notes_the_defaults() {
        let mut c = Config::default();
        c.game.rules.antes = true;
        c.client.sound.volume = 35;
        let text = render(&c);
        assert!(
            text.contains("# Default: false\nantes = true\n"),
            "{}",
            text
        );
        assert!(text.contains("# Default: 60\nvolume = 35\n"));
        assert_eq!(Config::parse(&text).unwrap(), c);
        // Long lists are wrapped and every bot name is spelled out in the comment.
        assert!(text.contains("# Default: [\"Ava\", \"Ben\""));
        assert!(text.contains("\"Priya\"]"));
        for l in text.lines() {
            assert!(l.chars().count() <= WRAP, "too long: {}", l);
        }
    }

    #[test]
    fn partial_files_keep_the_other_defaults() {
        let c = Config::parse(
            "[game.bots]\nnames = [\"Rex\", \"Ola\"]\n\n[[game.speeds]]\nname = \"Turbo\"\nms = 50\n\n[client]\nspeed = \"turbo\"\n[server]\nspeed = \"Turbo\"\n",
        )
        .unwrap();
        assert_eq!(c.game.bots.names, vec!["Rex", "Ola"]);
        assert_eq!(c.game.speeds.len(), 1);
        assert_eq!(c.speed_index("TURBO"), Some(0));
        assert_eq!(c.game.rules, Rules::default());
        assert_eq!(c.server_table().speed_ms, 50);
    }

    #[test]
    fn mistakes_are_reported() {
        let bad = [
            "[[game.speeds]]\nname = \"x\"\nms = 1\n[client]\nspeed = \"x\"\n[server]\nspeed = \"x\"\n",
            "[client]\nspeed = \"Warp\"\n",
            "[server]\nspeed = \"Warp\"\n",
            "[game]\nseats = 12\n",
            "[client.sound]\nvolume = 300\n",
            "[client.setup]\nstarting_chips = []\n",
            "[client.setup]\nsmall_blinds = [0, 5]\n",
            "[game.rules]\nblind_schedule = []\n",
            "[game.rules]\nblind_growth_percent = 50\n",
            "[game.rules]\nstraddle_multiplier = 1\n",
            // Typos are errors instead of being silently ignored.
            "[client.sound]\nvolum = 100\n",
            "[game.rules]\nantez = true\n",
            "not toml at all [",
        ];
        for text in bad {
            assert!(Config::parse(text).is_err(), "accepted: {}", text);
        }
        let e = Config::parse("[game.rules]\nantez = true\n").unwrap_err();
        assert!(e.contains("antez") && e.contains("ante_divisor"), "{}", e);
        // A file from an older holdem gets a hint about the new format.
        let e = Config::parse("[speed]\nfast_ms = 100\n").unwrap_err();
        assert!(e.contains("config write"), "{}", e);
    }

    #[test]
    fn the_player_name_has_a_sensible_fallback() {
        let named = ClientConfig {
            name: "  Mo  ".to_string(),
            ..ClientConfig::default()
        };
        assert_eq!(named.resolved_name(), "Mo");
        let unnamed = ClientConfig::default().resolved_name();
        assert!(!unnamed.is_empty() && unnamed.chars().count() <= crate::protocol::MAX_NAME);
    }

    #[test]
    fn a_missing_explicit_file_is_an_error() {
        assert!(Config::load(Some(Path::new("/no/such/holdem.toml"))).is_err());
    }

    #[test]
    fn table_settings_follow_the_file() {
        let mut c = Config::default();
        c.game.allow_win_chance = false;
        c.game.bots.thinking_samples = 99;
        c.game.style_min_stack_bb = 20;
        c.server.speed = "Fast".to_string();
        let s = c.server_table();
        assert!(!s.allow_win_chance);
        assert_eq!(s.bot_samples, 99);
        assert_eq!(s.stats_min_stack_bb, 20);
        assert_eq!(s.speed_ms, 220);
        assert_eq!(s.turn_seconds, 30);
        let solo = c.table_settings("Solo", 220, false);
        assert_eq!((solo.turn_seconds, solo.next_hand_seconds), (0, 0));
    }
}
