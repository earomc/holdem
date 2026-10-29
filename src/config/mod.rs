//! The optional configuration file, `holdem.toml`.
//!
//! It is looked for in the current folder, next to the program, then in your
//! own settings folder ([`user_file`]); a path can also be given with
//! `--config`. Every setting has a default, so the file may contain only what
//! you want to change. `holdem config write` writes a commented file with all
//! the defaults ([`template`]), and the game saves the settings you change in it
//! when you ask it to ([`save()`]).
//!
//! The file has these parts:
//!
//! * `[client]`: only your own screen and keys (name, sound, the values
//!   offered on the setup screen, joining other tables).
//! * `[network]`: the port for hosting, `holdem serve` and joining.
//! * `[bots]` and `[[speeds]]`: shared by every table.
//! * `[[tables]]`: one or more tables, each with its seats, rules, time limits.
//!   At home they are presets on the setup screen; `holdem serve` runs them all.
//!
//! What is configurable: everything about the game's design. What is not: the
//! internals of algorithms (how bots weigh their decisions, the hand evaluator,
//! the measured style reference values, the sound synthesis), where a number
//! only makes sense together with the code around it.
//!
//! This module has the settings and their defaults; `load` finds, reads and
//! checks a file, `render` writes one with every setting explained (`docs`
//! says what each one means), and `save` updates one with what the game changed.

use serde::{Deserialize, Serialize};

use crate::bot;
use crate::game::{DEFAULT_BOT_SAMPLES, Rules};
use crate::protocol::DEFAULT_PORT;
use crate::stats::{MIN_HANDS, MIN_STACK_BB};
use crate::table::{Fluctuation, TableSettings};

mod docs;
mod error;
mod load;
mod render;
mod save;

pub use error::Error;
pub use load::user_file;
pub use render::{differences, render, template};
pub use save::{SaveReport, save};

pub const FILE_NAME: &str = "holdem.toml";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub client: ClientConfig,
    pub network: NetworkConfig,
    pub bots: BotConfig,
    /// Game speeds, slowest first. Any number, with any names.
    pub speeds: Vec<SpeedConfig>,
    /// One or more tables: presets at home, all of them under `holdem serve`.
    pub tables: Vec<TableConfig>,
}

// ------------------------------------------------------------------ [client]

/// Settings that only affect your own screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClientConfig {
    /// Your name at the table; empty means your computer's login name.
    pub name: String,
    /// Your symbol, "★ cyan"; empty means the one your name gives.
    pub symbol: String,
    /// Name of the speed to start with (one of `[[speeds]]`).
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

// ------------------------------------------------------------------ shared

/// Network play: hosting from the menu, `holdem serve` and joining.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    /// Port to listen on; also the port the join screen assumes and searches.
    pub port: u16,
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

// ------------------------------------------------------------------ [[tables]]

/// One table (`[[tables]]`): a preset on the setup screen, and one of the
/// tables of `holdem serve`. Everything left out keeps its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TableConfig {
    /// The table's name, shown to players; it also tells the tables apart.
    pub name: String,
    /// Seats at the table, 2 to 9.
    pub seats: usize,
    /// Seats that people do not take are given to bots.
    pub fill_with_bots: bool,
    /// The speed under `holdem serve` (one of `[[speeds]]`); at home your own
    /// `client.speed` is used.
    pub speed: String,
    /// Players may turn on the win-chance hint.
    pub allow_win_chance: bool,
    /// Pause between the streets of an all-in run-out, in percent of the game speed.
    pub runout_pace_percent: u32,
    /// Hands started with fewer big blinds do not count for the style estimate.
    pub style_min_stack_bb: u32,
    /// Seconds a person has to act when several people play (0 = no limit).
    pub turn_seconds: u32,
    /// Seconds after a hand before the next one starts by itself (0 = wait for everyone).
    pub next_hand_seconds: u32,
    /// Seconds before a disconnected player's turn is folded for them.
    pub away_seconds: u32,
    /// Not `deny_unknown_fields` (rules also travel over the network); typos are
    /// caught by [`Config::parse`] instead.
    pub rules: Rules,
    pub fluctuation: FluctuationConfig,
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

// ------------------------------------------------------------------ defaults

/// The speed both the client and the tables start with.
const DEFAULT_SPEED: &str = "Normal";

fn speed(name: &str, ms: u32) -> SpeedConfig {
    SpeedConfig {
        name: name.to_string(),
        ms,
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            client: ClientConfig::default(),
            network: NetworkConfig::default(),
            bots: BotConfig::default(),
            speeds: vec![
                speed("Slow", 2000),
                speed(DEFAULT_SPEED, 1200),
                speed("Fast", 600),
                speed("Turbo", 200),
            ],
            tables: vec![TableConfig::default()],
        }
    }
}

impl Default for ClientConfig {
    fn default() -> ClientConfig {
        ClientConfig {
            name: String::new(),
            symbol: String::new(),
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

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig { port: DEFAULT_PORT }
    }
}

impl Default for BotConfig {
    fn default() -> BotConfig {
        BotConfig {
            names: bot::DEFAULT_NAMES
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            thinking_samples: DEFAULT_BOT_SAMPLES,
        }
    }
}

impl Default for TableConfig {
    fn default() -> TableConfig {
        TableConfig {
            name: "Hold'em table".to_string(),
            seats: 6,
            fill_with_bots: true,
            speed: DEFAULT_SPEED.to_string(),
            allow_win_chance: true,
            runout_pace_percent: 150,
            style_min_stack_bb: MIN_STACK_BB,
            turn_seconds: 30,
            next_hand_seconds: 8,
            away_seconds: 1,
            rules: Rules::default(),
            fluctuation: FluctuationConfig::default(),
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

impl TableConfig {
    /// How bots come and go at this table, when they do.
    pub fn fluctuation(&self) -> Fluctuation {
        Fluctuation {
            leave_chance: self.fluctuation.leave_chance as f32,
            join_chance: self.fluctuation.join_chance as f32,
        }
    }
}

impl Config {
    /// Position of a speed in `speeds`, by name (case does not matter).
    pub fn speed_index(&self, name: &str) -> Option<usize> {
        self.speeds
            .iter()
            .position(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The speed called `name`, or the first speed if there is none by that
    /// name. (A checked configuration has at least one speed, and every speed
    /// it names is one of them.)
    pub fn speed(&self, name: &str) -> &SpeedConfig {
        &self.speeds[self.speed_index(name).unwrap_or(0)]
    }

    /// The settings of `table`, running at `speed_ms`; `network` adds the time
    /// limits that only matter when several people play.
    pub fn table_settings(
        &self,
        table: &TableConfig,
        speed_ms: u32,
        network: bool,
    ) -> TableSettings {
        TableSettings {
            name: table.name.clone(),
            seats: table.seats,
            fill_with_bots: table.fill_with_bots,
            rules: table.rules.clone(),
            fluctuation: table.fluctuation.enabled.then(|| table.fluctuation()),
            bot_names: self.bots.names.clone(),
            speed_ms,
            turn_seconds: if network { table.turn_seconds } else { 0 },
            next_hand_seconds: if network { table.next_hand_seconds } else { 0 },
            away_seconds: table.away_seconds,
            runout_pace_percent: table.runout_pace_percent,
            allow_win_chance: table.allow_win_chance,
            bot_samples: self.bots.thinking_samples,
            stats_min_stack_bb: table.style_min_stack_bb,
        }
    }

    /// The tables of `holdem serve`: every `[[tables]]` entry, each at its speed.
    pub fn server_tables(&self) -> Vec<TableSettings> {
        self.tables
            .iter()
            .map(|t| self.table_settings(t, self.speed(&t.speed).ms, true))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::docs::{DOCS, doc};
    use super::render::WRAP;
    use super::*;

    #[test]
    fn differences_name_the_changed_settings() {
        assert!(differences(&Config::default()).is_empty());
        let c = Config::parse(
            r#"
[client]
speed = "Only"
[client.sound]
volume = 5

[[speeds]]
name = "Only"
ms = 300

[[tables]]
speed = "only"
[tables.rules]
antes = true
"#,
        )
        .unwrap();
        assert_eq!(
            differences(&c),
            ["client.speed", "client.sound.volume", "speeds", "tables"]
        );
    }

    /// Every dotted path of the default settings: sections and settings.
    fn paths() -> Vec<(String, toml::Value)> {
        fn walk(prefix: &str, t: &toml::Table, out: &mut Vec<(String, toml::Value)>) {
            for (k, v) in t {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.push((p.clone(), v.clone()));
                match v {
                    toml::Value::Table(sub) => walk(&p, sub, out),
                    // A list whose entries are explained setting by setting
                    // ([[tables]], not [[speeds]]): the settings of an entry.
                    toml::Value::Array(items) if !doc(&format!("{p}.name")).is_empty() => {
                        if let Some(toml::Value::Table(first)) = items.first() {
                            walk(&p, first, out);
                        }
                    }
                    _ => {}
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
            assert!(!d.is_empty(), "{path} has no description");
            if value.is_bool() {
                assert!(
                    d.lines().any(|l| l.starts_with("true:"))
                        && d.lines().any(|l| l.starts_with("false:")),
                    "{path} does not say what true and false do"
                );
            }
        }
        for (path, _) in DOCS {
            assert!(
                paths.iter().any(|(p, _)| p == path),
                "{path} is described but is not a setting"
            );
        }
    }

    #[test]
    fn the_file_shows_current_values_and_notes_the_defaults() {
        let mut c = Config::default();
        c.tables[0].rules.antes = true;
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
            assert!(l.chars().count() <= WRAP, "too long: {l}");
        }
    }

    #[test]
    fn partial_files_keep_the_other_defaults() {
        let c = Config::parse(
            r#"
[client]
speed = "turbo"

[bots]
names = ["Rex", "Ola"]

[[speeds]]
name = "Turbo"
ms = 50

[[tables]]
speed = "Turbo"
"#,
        )
        .unwrap();
        assert_eq!(c.bots.names, vec!["Rex", "Ola"]);
        assert_eq!(c.speeds.len(), 1);
        assert_eq!(c.speed_index("TURBO"), Some(0));
        assert_eq!(c.tables[0].rules, Rules::default());
        assert_eq!(c.tables[0].name, "Hold'em table");
        assert_eq!(c.server_tables()[0].speed_ms, 50);
    }

    #[test]
    fn mistakes_are_reported() {
        let bad = [
            "[[speeds]]\nname = \"x\"\nms = 1\n[client]\nspeed = \"x\"\n[[tables]]\nspeed = \"x\"\n",
            "[client]\nspeed = \"Warp\"\n",
            "[[tables]]\nspeed = \"Warp\"\n",
            "[[tables]]\nseats = 12\n",
            "[client.sound]\nvolume = 300\n",
            "[client.setup]\nstarting_chips = []\n",
            "[client.setup]\nsmall_blinds = [0, 5]\n",
            "[[tables]]\n[tables.rules]\nblind_schedule = []\n",
            "[[tables]]\n[tables.rules]\nblind_growth_percent = 50\n",
            "[[tables]]\n[tables.rules]\nstraddle_multiplier = 1\n",
            "tables = []\n",
            // Typos are errors instead of being silently ignored.
            "[client.sound]\nvolum = 100\n",
            "[[tables]]\n[tables.rules]\nantez = true\n",
            "not toml at all [",
        ];
        for text in bad {
            assert!(Config::parse(text).is_err(), "accepted: {text}");
        }
        let e = Config::parse("[[tables]]\n[tables.rules]\nantez = true\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("antez") && e.contains("ante_divisor"), "{}", e);
        // A file from an older holdem gets a hint about the new format.
        let e = Config::parse("[game]\nseats = 4\n")
            .unwrap_err()
            .to_string();
        assert!(e.contains("config write"), "{}", e);
    }

    #[test]
    fn several_tables() {
        let c = Config::default();
        let one = c.server_tables();
        assert_eq!(one.len(), 1, "by default, one table");
        assert_eq!(one[0].name, "Hold'em table");

        let c = Config::parse(
            r#"
[[tables]]
name = "Low"
[tables.rules]
small_blind = 5

[[tables]]
name = "Turbo"
speed = "Fast"
seats = 4
"#,
        )
        .unwrap();
        let tables = c.server_tables();
        assert_eq!(tables.len(), 2);
        assert_eq!(tables[0].name, "Low");
        assert_eq!(tables[0].rules.small_blind, 5);
        assert_eq!(tables[0].seats, 6, "what is left out keeps its default");
        assert_eq!(tables[1].seats, 4);
        assert_eq!(tables[1].speed_ms, c.speed("Fast").ms);

        let table = |extra: &str| format!("[[tables]]\nname = \"A\"\n{extra}");
        for bad in [
            format!("{}\n{}", table(""), table("")),
            "[[tables]]\nname = \" \"\n".to_string(),
            table("speed = \"Warp\"\n"),
            table("seats = 12\n"),
            table("[tables.rules]\nsmall_blind = 0\n"),
            table("antes = true\n"),
        ] {
            assert!(Config::parse(&bad).is_err(), "accepted: {bad}");
        }
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
        c.tables[0].allow_win_chance = false;
        c.bots.thinking_samples = 99;
        c.tables[0].style_min_stack_bb = 20;
        c.tables[0].speed = "Fast".to_string();
        let s = c.server_tables().remove(0);
        assert!(!s.allow_win_chance);
        assert_eq!(s.bot_samples, 99);
        assert_eq!(s.stats_min_stack_bb, 20);
        assert_eq!(s.speed_ms, 600);
        assert_eq!(s.turn_seconds, 30);
        let solo = c.table_settings(&c.tables[0], 600, false);
        assert_eq!((solo.turn_seconds, solo.next_hand_seconds), (0, 0));
    }
}
