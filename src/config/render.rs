//! Writing a settings file with every setting explained and each default noted.

use super::Config;
use super::docs::doc;

/// Longest line in the written file (values and comments are wrapped).
pub(super) const WRAP: usize = 88;

/// `first` followed by `parts` joined with ", ", wrapped at [`WRAP`] columns,
/// with continuation lines starting with `indent`; `close` ends the last line.
fn wrap_list(first: &str, parts: &[String], indent: &str, close: &str) -> String {
    let mut out = String::new();
    let mut line = first.to_string();
    for (i, part) in parts.iter().enumerate() {
        let piece = if i + 1 < parts.len() {
            format!("{part},")
        } else {
            format!("{part}{close}")
        };
        let starts_line = line.ends_with('[') || line == indent;
        let candidate = if starts_line {
            format!("{line}{piece}")
        } else {
            format!("{line} {piece}")
        };
        if candidate.chars().count() > WRAP && !starts_line {
            out.push_str(&line);
            out.push('\n');
            line = format!("{indent}{piece}");
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
            let parts: Vec<String> = items.iter().map(std::string::ToString::to_string).collect();
            wrap_list(&format!("{prefix}["), &parts, indent, "]")
        }
        other => format!("{prefix}{other}"),
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
pub(super) fn comment(out: &mut String, text: &str) {
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
                current = format!("# {indent}");
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
    matches!(v, toml::Value::Array(a) if !a.is_empty() && a.iter().all(toml::Value::is_table))
}

/// Lists of `[[tables]]` that may be empty. Empty, they are written as their
/// explanation only: `tables = []` would clash with entries added below it.
const TABLE_LISTS: &[&str] = &["server.tables"];

fn render_table(out: &mut String, path: &str, table: &toml::Table, defaults: &toml::Table) {
    let join = |key: &str| {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
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
        if TABLE_LISTS.contains(&join(key).as_str()) && value.as_array().is_some_and(Vec::is_empty)
        {
            out.push('\n');
            comment(out, doc(&join(key)));
            raw_comment(out, "Default: none");
            continue;
        }
        out.push('\n');
        comment(out, doc(&join(key)));
        let default_line = render_value("Default: ", &default_of(key), "          ");
        raw_comment(out, &default_line);
        out.push_str(&render_value(
            &format!("{key} = "),
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
                out.push_str(&format!("[[{full}]]\n"));
                for (k, v) in t {
                    out.push_str(&format!("{k} = {v}\n"));
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
        out.push_str(&format!("[{full}]\n"));
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
    pub(super) fn walk(
        prefix: &str,
        current: &toml::Table,
        defaults: &toml::Table,
        out: &mut Vec<String>,
    ) {
        for (key, value) in current {
            let path = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
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
