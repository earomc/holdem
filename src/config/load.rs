//! Finding, reading and checking the settings file.

use std::path::{Path, PathBuf};

use super::{Config, Error, FILE_NAME};
use crate::game::{MAX_PLAYERS, MIN_PLAYERS, Rules};

/// `holdem.toml` in your own settings folder: `%APPDATA%\holdem` on Windows,
/// `~/Library/Application Support/holdem` on macOS, and
/// `$XDG_CONFIG_HOME/holdem` or `~/.config/holdem` elsewhere. `None` if the
/// environment names no such folder.
pub fn user_file() -> Option<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let dir = if cfg!(windows) {
        var("APPDATA")?
    } else if cfg!(target_os = "macos") {
        var("HOME")?.join("Library").join("Application Support")
    } else {
        var("XDG_CONFIG_HOME").or_else(|| var("HOME").map(|h| h.join(".config")))?
    };
    Some(dir.join("holdem").join(FILE_NAME))
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
            let mut names: Vec<&str> = known.keys().map(std::string::String::as_str).collect();
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
        return Err(format!("client.setup.{name} must list at least one value"));
    }
    if !allow_zero && values.contains(&0) {
        return Err(format!("client.setup.{name} must not contain 0"));
    }
    Ok(())
}

impl Config {
    /// Parses a configuration file's text.
    pub fn parse(text: &str) -> Result<Config, Error> {
        let config: Config = toml::from_str(text).map_err(|e| {
            Error::Parse(format!(
                "{e}\n(A file written by an older holdem may use old setting names: \
                 `holdem config write new.toml` writes the current format.)"
            ))
        })?;
        check_rule_keys(text).map_err(Error::Parse)?;
        config.check().map_err(Error::Invalid)?;
        Ok(config)
    }

    pub(super) fn check(&self) -> Result<(), String> {
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
                "game.seats must be between {MIN_PLAYERS} and {MAX_PLAYERS}"
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
                "game.rules.straddle_min_players must be between 3 and {MAX_PLAYERS}"
            ));
        }
        let f = &g.fluctuation;
        let mut tables: Vec<&str> = Vec::new();
        for t in &self.server.tables {
            let name = t.name.trim();
            if name.is_empty() {
                return Err("every [[server.tables]] entry needs a name".to_string());
            }
            if tables.contains(&name) {
                return Err(format!(
                    "the table name \"{name}\" is used twice in [[server.tables]]"
                ));
            }
            tables.push(name);
            if let Some(speed) = &t.speed
                && self.speed_index(speed).is_none()
            {
                return Err(format!(
                    "the table \"{name}\" has the speed \"{speed}\", which is not in [[game.speeds]]"
                ));
            }
            if t.seats
                .is_some_and(|s| !(MIN_PLAYERS..=MAX_PLAYERS).contains(&s))
            {
                return Err(format!(
                    "the table \"{name}\" must have {MIN_PLAYERS} to {MAX_PLAYERS} seats"
                ));
            }
            if t.starting_chips == Some(0) || t.small_blind == Some(0) {
                return Err(format!(
                    "the table \"{name}\": starting_chips and small_blind must be above 0"
                ));
            }
        }
        if !(0.0..=1.0).contains(&f.leave_chance) || !(0.0..=1.0).contains(&f.join_chance) {
            return Err("game.fluctuation chances must be between 0 and 1".to_string());
        }
        Ok(())
    }

    /// The settings file holdem uses: `path` if given (it must then exist),
    /// otherwise the first that exists of `holdem.toml` in the current folder,
    /// next to the program, and in your settings folder ([`user_file`]).
    /// `None` means the defaults.
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
        candidates.extend(user_file());
        candidates.into_iter().find(|c| c.is_file())
    }

    /// Reads and checks one settings file.
    pub fn read(path: &Path) -> Result<Config, Error> {
        let text = std::fs::read_to_string(path).map_err(|e| Error::Io(e).in_file(path))?;
        Config::parse(&text).map_err(|e| e.in_file(path))
    }

    /// Loads the settings from the file [`Config::locate`] picks, or the
    /// defaults if there is none. Returns the file that was used, if any.
    pub fn load(path: Option<&Path>) -> Result<(Config, Option<PathBuf>), Error> {
        match Config::locate(path) {
            Some(file) => Ok((Config::read(&file)?, Some(file))),
            None => Ok((Config::default(), None)),
        }
    }
}
