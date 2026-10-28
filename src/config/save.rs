//! Saving the settings changed in the game back to the settings file.
//!
//! Only your own settings ([`CLIENT_KEYS`]) and the setup screen's options of
//! one table ([`TABLE_KEYS`], in its `[[tables]]` entry) are touched, and in
//! place: the rest of the file, comments and layout included, stays as it was.
//! A setting that is not in the file yet is only added when it differs from
//! its default, so defaults you never changed are not frozen into the file and
//! keep improving with newer versions of holdem.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use toml_edit::DocumentMut;

use super::{Config, Error, TableConfig};

/// Your own settings that the game can change: from the settings window and
/// the setup screen's name.
const CLIENT_KEYS: &[&str] = &[
    "client.name",
    "client.speed",
    "client.win_chance_hint",
    "client.sound.volume",
];

/// A table's settings that the setup screen can change, within its
/// `[[tables]]` entry.
const TABLE_KEYS: &[&str] = &[
    "seats",
    "fill_with_bots",
    "allow_win_chance",
    "fluctuation.enabled",
    "rules.starting_chips",
    "rules.small_blind",
    "rules.hands_per_level",
    "rules.hand_limit",
    "rules.antes",
    "rules.straddle",
    "rules.rebuy_levels",
    "rules.dead_button",
    "rules.muck_losers",
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
    /// The settings written, as dotted names (a table's as `tables.<key>`);
    /// empty if the file already had them.
    pub changed: Vec<String>,
    /// The file did not exist before.
    pub created: bool,
}

/// Saves your own settings and the options of `config.tables[table]` to the
/// file at `path`, creating the file and its folder if needed. The table is
/// found in the file by its name; a file without it gets a new `[[tables]]`
/// entry. Writes nothing when the file already holds these values (or, for a
/// missing file, when they are all defaults).
pub fn save(path: &Path, config: &Config, table: usize) -> Result<SaveReport, Error> {
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

    let mut changed = Vec::new();
    let (wanted, in_file) = (as_table(config), as_table(&saved));
    let defaults = as_table(&Config::default());
    save_keys(
        doc.as_table_mut(),
        CLIENT_KEYS,
        &wanted,
        &in_file,
        &defaults,
        "",
        &mut changed,
    );

    let chosen = &config.tables[table];
    let in_file_table = saved
        .tables
        .iter()
        .find(|t| t.name == chosen.name)
        .filter(|_| has_tables(&doc));
    let wanted = as_table(chosen);
    let in_file = as_table(in_file_table.unwrap_or(&TableConfig::default()));
    let defaults = as_table(&TableConfig::default());
    let entry = table_entry(&mut doc, &chosen.name);
    let before = changed.len();
    save_keys(
        entry,
        TABLE_KEYS,
        &wanted,
        &in_file,
        &defaults,
        "tables.",
        &mut changed,
    );
    if changed.len() == before {
        // Nothing to save for the table: no new entry for it either.
        drop_empty_entry(&mut doc, &chosen.name);
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

/// Writes each of `keys` into `table` where it differs from what the file had
/// (or, if the file did not have it, from its default).
fn save_keys(
    table: &mut toml_edit::Table,
    keys: &[&str],
    wanted: &toml::Table,
    in_file: &toml::Table,
    defaults: &toml::Table,
    prefix: &str,
    changed: &mut Vec<String>,
) {
    for &key in keys {
        let new = lookup(wanted, key);
        let present = item(table, key).is_some_and(|i| !i.is_none());
        let old = if present {
            lookup(in_file, key)
        } else {
            lookup(defaults, key)
        };
        if new != old {
            set(table, key, new);
            changed.push(format!("{prefix}{key}"));
        }
    }
}

fn has_tables(doc: &DocumentMut) -> bool {
    doc.get("tables")
        .and_then(toml_edit::Item::as_array_of_tables)
        .is_some_and(|a| !a.is_empty())
}

/// The name of a `[[tables]]` entry in the file (an entry without one has the
/// default name).
fn entry_name(entry: &toml_edit::Table) -> String {
    entry
        .get("name")
        .and_then(toml_edit::Item::as_str)
        .map_or_else(|| TableConfig::default().name, str::to_string)
}

/// The file's `[[tables]]` entry called `name`, added if there is none.
fn table_entry<'a>(doc: &'a mut DocumentMut, name: &str) -> &'a mut toml_edit::Table {
    let tables = doc
        .entry("tables")
        .or_insert_with(|| toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .expect("the checked settings file has a list of tables here");
    let found = tables.iter().position(|t| entry_name(t) == name);
    let index = match found {
        Some(i) => i,
        None => {
            let mut entry = toml_edit::Table::new();
            entry.insert("name", toml_edit::value(name));
            tables.push(entry);
            tables.len() - 1
        }
    };
    tables.get_mut(index).expect("the entry exists")
}

/// Removes the entry called `name` again if it holds nothing but its name.
fn drop_empty_entry(doc: &mut DocumentMut, name: &str) {
    let Some(tables) = doc
        .get_mut("tables")
        .and_then(toml_edit::Item::as_array_of_tables_mut)
    else {
        return;
    };
    let empty = tables
        .iter()
        .position(|t| entry_name(t) == name && t.len() == 1 && t.contains_key("name"));
    if let Some(i) = empty {
        tables.remove(i);
    }
    if tables.is_empty() {
        doc.remove("tables");
    }
}

fn as_table<T: serde::Serialize>(value: &T) -> toml::Table {
    toml::Table::try_from(value).expect("settings convert to TOML")
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

/// The item at a dotted `key` in `table`, if its sub-tables exist.
fn item<'a>(table: &'a toml_edit::Table, key: &str) -> Option<&'a toml_edit::Item> {
    let mut parts = key.split('.');
    let mut current = table.get(parts.next()?)?;
    for part in parts {
        current = current.get(part)?;
    }
    Some(current)
}

/// Sets a dotted `key` in `table`, creating its sub-tables, and keeps the
/// comment that may follow an existing value on its line.
fn set(table: &mut toml_edit::Table, key: &str, value: &toml::Value) {
    let mut parts: Vec<&str> = key.split('.').collect();
    let last = parts.pop().expect("a key has a name");
    let mut table = table;
    for part in parts {
        let entry = table.entry(part).or_insert_with(|| {
            // Implicit: `[client]` is not written when only `[client.sound]` is needed.
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

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_new_file_gets_only_the_changed_settings() {
        let path = scratch("new");
        let mut config = Config::default();
        config.client.name = "Mo".to_string();
        config.tables[0].rules.small_blind = 25;
        let report = save(&path, &config, 0).unwrap();
        assert!(report.created);
        assert_eq!(report.changed, ["client.name", "tables.rules.small_blind"]);

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# holdem settings, saved from the game"));
        assert!(
            text.contains("[[tables]]\nname = \"Hold'em table\""),
            "{text}"
        );
        assert!(!text.contains("seats"), "defaults stay out:\n{text}");
        assert_eq!(Config::read(&path).unwrap(), config);

        let again = save(&path, &config, 0).unwrap();
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
        let report = save(&path, &Config::default(), 0).unwrap();
        assert!(report.changed.is_empty());
        assert!(!path.exists());
    }

    #[test]
    fn your_own_settings_alone_add_no_table() {
        let path = scratch("client");
        let mut config = Config::default();
        config.client.sound.volume = 30;
        save(&path, &config, 0).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.contains("tables"), "{text}");
    }

    #[test]
    fn the_rest_of_the_file_stays_as_it_was() {
        let path = scratch("keep");
        write(
            &path,
            r#"# my own notes
[client]
name = "Mo"   # that is me
log_lines = 300

[[tables]]
name = "Low stakes"
seats = 6   # the default, but written down

[[tables]]
name = "Turbo"
seats = 4
"#,
        );
        let mut config = Config::read(&path).unwrap();
        config.client.name = "Ann".to_string();
        config.client.sound.volume = 30;
        config.tables[1].seats = 5;
        config.tables[1].rules.antes = true;
        let report = save(&path, &config, 1).unwrap();
        assert!(!report.created);
        assert_eq!(
            report.changed,
            [
                "client.name",
                "client.sound.volume",
                "tables.seats",
                "tables.rules.antes"
            ]
        );

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("# my own notes"));
        assert!(text.contains("name = \"Ann\"   # that is me"), "{text}");
        assert!(text.contains("log_lines = 300"));
        assert!(
            text.contains("seats = 6   # the default, but written down"),
            "{text}"
        );
        assert_eq!(Config::read(&path).unwrap(), config, "only Turbo changed");
    }

    #[test]
    fn a_setting_in_the_file_is_updated_even_back_to_its_default() {
        let path = scratch("default");
        write(&path, "[[tables]]\nseats = 4\n");
        let report = save(&path, &Config::default(), 0).unwrap();
        assert_eq!(report.changed, ["tables.seats"]);
        assert_eq!(Config::read(&path).unwrap(), Config::default());
    }

    #[test]
    fn a_save_that_would_not_load_is_refused() {
        let path = scratch("invalid");
        let mut config = Config::default();
        config.tables[0].seats = 1;
        let error = save(&path, &config, 0).unwrap_err().to_string();
        assert!(error.contains("would not load"), "{error}");
        assert!(!path.exists());
    }
}
