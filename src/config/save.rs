//! Saving the settings changed in the game back to the settings file.
//!
//! Only the settings in [`SAVED_KEYS`] are touched, and in place: the rest of the
//! file, comments and layout included, stays as it was. A setting that is not in
//! the file yet is only added when it differs from its default, so defaults you
//! never changed are not frozen into the file and keep improving with newer
//! versions of holdem.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use super::{Config, Error};

/// The settings the game can change and save: your own ones from the settings
/// window and the setup screen, and the table options of the setup screen.
pub const SAVED_KEYS: &[&str] = &[
    "client.name",
    "client.speed",
    "client.win_chance_hint",
    "client.sound.volume",
    "game.seats",
    "game.fill_with_bots",
    "game.allow_win_chance",
    "game.fluctuation.enabled",
    "game.rules.starting_chips",
    "game.rules.small_blind",
    "game.rules.hands_per_level",
    "game.rules.hand_limit",
    "game.rules.antes",
    "game.rules.straddle",
    "game.rules.rebuy_levels",
    "game.rules.dead_button",
    "game.rules.muck_losers",
];

/// The start of a file that saving creates.
const HEADER: &str = "\
# holdem settings, saved from the game. Every line is optional: missing
# settings keep their defaults. `holdem config write other.toml` writes a file
# with every setting explained.
";

/// What [`save`] did.
#[derive(Debug, PartialEq)]
pub struct SaveReport {
    /// The settings written, as dotted names; empty if the file already had them.
    pub changed: Vec<String>,
    /// The file did not exist before.
    pub created: bool,
}

/// Saves the [`SAVED_KEYS`] of `config` to the file at `path`, creating the file
/// and its folder if needed. Writes nothing when the file already holds these
/// values (or, for a missing file, when they are all defaults).
pub fn save(path: &Path, config: &Config) -> Result<SaveReport, Error> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == ErrorKind::NotFound => None,
        Err(e) => return Err(Error::Io(e).in_file(path)),
    };
    let saved = match &existing {
        Some(text) => Config::parse(text).map_err(|e| e.in_file(path))?,
        None => Config::default(),
    };
    let mut doc: DocumentMut = existing
        .as_deref()
        .unwrap_or("")
        .parse()
        .map_err(|e: toml_edit::TomlError| Error::Parse(e.to_string()).in_file(path))?;

    let wanted = as_table(config);
    let in_file = as_table(&saved);
    let defaults = as_table(&Config::default());
    let mut changed = Vec::new();
    for &key in SAVED_KEYS {
        let new = lookup(&wanted, key);
        let present = item(&doc, key).is_some_and(|i| !i.is_none());
        let differs = if present {
            new != lookup(&in_file, key)
        } else {
            new != lookup(&defaults, key)
        };
        if differs {
            set(&mut doc, key, new);
            changed.push(key.to_string());
        }
    }
    if changed.is_empty() {
        return Ok(SaveReport {
            changed,
            created: false,
        });
    }

    // Never write a file that holdem itself could not read back.
    let text = match existing {
        Some(_) => doc.to_string(),
        None => format!("{HEADER}\n{doc}"),
    };
    Config::parse(&text)
        .map_err(|e| Error::Invalid(format!("the saved file would not load: {e}")).in_file(path))?;
    write_atomically(path, &text).map_err(|e| Error::Io(e).in_file(path))?;
    Ok(SaveReport {
        changed,
        created: existing.is_none(),
    })
}

fn as_table(config: &Config) -> toml::Table {
    toml::Table::try_from(config).expect("settings convert to TOML")
}

/// The value at a dotted `key`.
fn lookup<'a>(table: &'a toml::Table, key: &str) -> &'a toml::Value {
    let mut parts = key.split('.');
    let mut value = table
        .get(parts.next().expect("a key has a name"))
        .expect("saved keys exist in the settings");
    for part in parts {
        value = value.get(part).expect("saved keys exist in the settings");
    }
    value
}

/// The item at a dotted `key` in the file, if its tables exist.
fn item<'a>(doc: &'a DocumentMut, key: &str) -> Option<&'a toml_edit::Item> {
    let mut parts = key.split('.');
    let mut current = doc.get(parts.next()?)?;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}

/// Sets a dotted `key` in the file, creating its tables, and keeps the comment
/// that may follow an existing value on its line.
fn set(doc: &mut DocumentMut, key: &str, value: &toml::Value) {
    let mut parts: Vec<&str> = key.split('.').collect();
    let last = parts.pop().expect("a key has a name");
    let mut table = doc.as_table_mut();
    for part in parts {
        let entry = table.entry(part).or_insert_with(|| {
            // Implicit: `[game]` is not written when only `[game.rules]` is needed.
            let mut new = toml_edit::Table::new();
            new.set_implicit(true);
            toml_edit::Item::Table(new)
        });
        table = entry
            .as_table_mut()
            .expect("the checked settings file has tables here");
    }
    let mut new = edit_value(value);
    if let Some(old) = table.get(last).and_then(toml_edit::Item::as_value) {
        *new.decor_mut() = old.decor().clone();
    }
    table.insert(last, toml_edit::Item::Value(new));
}

/// A plain setting as `toml_edit` writes it.
fn edit_value(value: &toml::Value) -> toml_edit::Value {
    match value {
        toml::Value::String(s) => s.as_str().into(),
        toml::Value::Integer(i) => (*i).into(),
        toml::Value::Float(f) => (*f).into(),
        toml::Value::Boolean(b) => (*b).into(),
        other => unreachable!("saved settings are plain values, not {other}"),
    }
}

/// Writes through a temporary file next to `path`, so an interrupted save
/// cannot leave half a file behind.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir)?;
    }
    let mut temporary = PathBuf::from(path);
    temporary.as_mut_os_string().push(".saving");
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh path in the temporary folder; the file does not exist yet.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("holdem-save-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("holdem.toml")
    }

    #[test]
    fn a_new_file_gets_only_the_changed_settings() {
        let path = scratch("new");
        let mut config = Config::default();
        config.client.name = "Mo".to_string();
        config.game.rules.small_blind = 25;
        let report = save(&path, &config).unwrap();
        assert!(report.created);
        assert_eq!(report.changed, ["client.name", "game.rules.small_blind"]);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# holdem settings, saved from the game"));
        assert!(!text.contains("seats"), "defaults stay out:\n{text}");
        assert_eq!(Config::read(&path).unwrap(), config);

        let again = save(&path, &config).unwrap();
        assert_eq!(
            again,
            SaveReport {
                changed: vec![],
                created: false
            }
        );
    }

    #[test]
    fn nothing_to_save_writes_no_file() {
        let path = scratch("nothing");
        let report = save(&path, &Config::default()).unwrap();
        assert!(report.changed.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn the_rest_of_the_file_stays_as_it_was() {
        let path = scratch("keep");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original = "\
# my own notes
[client]
name = \"Mo\"   # that is me
log_lines = 300

[game]
seats = 6   # the default, but written down
";
        std::fs::write(&path, original).unwrap();
        let mut config = Config::read(&path).unwrap();
        config.client.name = "Ann".to_string();
        config.game.seats = 4;
        config.client.sound.volume = 30;
        let report = save(&path, &config).unwrap();
        assert!(!report.created);
        assert_eq!(
            report.changed,
            ["client.name", "client.sound.volume", "game.seats"]
        );

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my own notes"));
        assert!(text.contains("name = \"Ann\"   # that is me"), "{text}");
        assert!(text.contains("log_lines = 300"));
        assert!(
            text.contains("seats = 4   # the default, but written down"),
            "{text}"
        );
        assert_eq!(Config::read(&path).unwrap(), config);
    }

    #[test]
    fn a_setting_in_the_file_is_updated_even_back_to_its_default() {
        let path = scratch("default");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[game]\nseats = 4\n").unwrap();
        let report = save(&path, &Config::default()).unwrap();
        assert_eq!(report.changed, ["game.seats"]);
        assert_eq!(Config::read(&path).unwrap(), Config::default());
    }

    #[test]
    fn a_save_that_would_not_load_is_refused() {
        let path = scratch("invalid");
        let mut config = Config::default();
        config.game.seats = 1;
        let error = save(&path, &config).unwrap_err().to_string();
        assert!(error.contains("would not load"), "{error}");
        assert!(!path.exists());
    }
}
