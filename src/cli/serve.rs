//! `holdem serve`: tables with no player of their own, all on one port, and
//! what it prints.

use std::path::Path;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use holdem::config::Config;
use holdem::game::chips_text;
use holdem::net::{self, Server};
use holdem::protocol::{APP_VERSION, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};
use holdem::table::TableSettings;

use super::config::source_text;
use super::{BUILD, ServerOptions};

fn on_off(value: bool, on: &str, off: &str) -> String {
    if value { on } else { off }.to_string()
}

/// One table of the server, with the name of its speed.
struct ServedTable {
    settings: TableSettings,
    speed: String,
}

/// Everything a dedicated server runs with, as labelled lines. With several
/// tables, what differs between them is listed per table and the rest once.
fn server_report(
    tables: &[ServedTable],
    port: u16,
    source: Option<&Path>,
    overrides: &[String],
) -> String {
    let several = tables.len() > 1;
    let settings = &tables[0].settings;
    let speed_name = &tables[0].speed;
    let r = &settings.rules;
    let mut out = String::new();
    let mut section = |title: &str, rows: Vec<(&str, String)>| {
        out.push_str(title);
        out.push('\n');
        for (label, value) in rows {
            out.push_str(&format!("  {label:<22}{value}\n"));
        }
    };
    let source = source_text(source);
    let mut origin = vec![("settings from", source)];
    if !overrides.is_empty() {
        origin.push(("command line", overrides.join(", ")));
    }
    section("Configuration", origin);

    let lan = match net::local_ip() {
        Some(ip) => format!("{ip}:{port}"),
        None => format!("this computer's address, port {port}"),
    };
    let fluctuation = match &settings.fluctuation {
        Some(f) => format!(
            "on: a bot leaves with {:.0}%, an empty seat fills with {:.0}% per hand",
            f.leave_chance * 100.0,
            f.join_chance * 100.0
        ),
        None => "off".to_string(),
    };
    let free_seats = |s: &TableSettings| {
        on_off(
            s.fill_with_bots,
            "filled with bots",
            "left empty (people only)",
        )
    };
    if several {
        let rows = tables
            .iter()
            .map(|t| {
                let s = &t.settings;
                (
                    s.name.as_str(),
                    format!(
                        "{} seats, {}, {} chips, blinds {}, free seats {}",
                        s.seats,
                        t.speed,
                        chips_text(s.rules.starting_chips),
                        s.rules.blinds_text(),
                        free_seats(s)
                    ),
                )
            })
            .collect();
        section("Tables", rows);
    }
    // With several tables, the rows that may differ are listed above instead.
    let per_table = ["name", "seats", "free seats", "speed", "starting chips"];
    let mut table_rows = vec![
        ("name", format!("\"{}\"", settings.name)),
        ("port", format!("{port} (on this network: {lan})")),
        ("seats", settings.seats.to_string()),
        ("free seats", free_seats(settings)),
        ("bot names", settings.bot_names.join(", ")),
        (
            "bot thinking",
            format!("{} simulated deals per decision", settings.bot_samples),
        ),
        (
            "speed",
            format!("{} ({} ms per bot move)", speed_name, settings.speed_ms),
        ),
        (
            "all-in run-out pace",
            format!("{}% of the speed", settings.runout_pace_percent),
        ),
        ("players come and go", fluctuation),
        (
            "win-chance hint",
            on_off(settings.allow_win_chance, "allowed", "not allowed"),
        ),
        (
            "style estimate",
            format!(
                "counts hands started with at least {} big blinds",
                settings.stats_min_stack_bb
            ),
        ),
    ];
    if several {
        table_rows.retain(|(label, _)| !per_table.contains(label));
    }
    section(if several { "Every table" } else { "Table" }, table_rows);

    let schedule: Vec<String> = r.blind_schedule.iter().map(|p| format!("{p}%")).collect();
    let blinds = if several {
        format!("as listed for each table, up {}", r.levels_text())
    } else {
        format!("{}, up {}", r.blinds_text(), r.levels_text())
    };
    let mut rules = vec![
        ("starting chips", chips_text(r.starting_chips)),
        ("blinds", blinds),
        (
            "blind schedule",
            format!(
                "{}, then +{}% per level",
                schedule.join(" "),
                r.blind_growth_percent.saturating_sub(100)
            ),
        ),
        ("hand limit", r.hand_limit_text()),
        ("antes", r.antes_text()),
        ("straddle", r.straddle_text()),
        ("rebuys", r.rebuys_text()),
        ("button rule", r.button_text().to_string()),
        ("showdown", r.showdown_text().to_string()),
    ];
    if several {
        rules.retain(|(label, _)| !per_table.contains(label));
    }
    section("Rules", rules);

    let seconds = |s: u32, none: &str| {
        if s == 0 {
            none.to_string()
        } else {
            format!("{s} s")
        }
    };
    section(
        "Network play",
        vec![
            ("time to act", seconds(settings.turn_seconds, "no limit")),
            (
                "next hand after",
                seconds(settings.next_hand_seconds, "everyone is ready"),
            ),
            (
                "dropped players",
                format!("folded after {} s", settings.away_seconds),
            ),
            (
                "protocol",
                format!(
                    "holdem {APP_VERSION} ({BUILD}), protocol versions {MIN_PROTOCOL_VERSION} to {PROTOCOL_VERSION}"
                ),
            ),
        ],
    );
    out
}

/// `holdem serve`: the tables, with no player of their own, until Ctrl-C.
pub(crate) fn run_server(
    config: &Config,
    source: Option<&Path>,
    options: &ServerOptions,
    seed: Option<u64>,
) -> Result<(), String> {
    let mut overrides = Vec::new();
    let mut tables: Vec<ServedTable> = config
        .server_tables()
        .into_iter()
        .zip(&config.tables)
        .map(|(settings, table)| ServedTable {
            settings,
            speed: config.speed(&table.speed).name.clone(),
        })
        .collect();
    if let Some(name) = &options.name {
        if tables.len() > 1 {
            return Err(
                "--name names the one table; with several [[tables]] the names are in the settings file"
                    .to_string(),
            );
        }
        tables[0].settings.name = name.clone();
        overrides.push(format!("--name \"{name}\""));
    }
    if let Some(wanted) = &options.speed {
        let Some(i) = config.speed_index(wanted) else {
            let names: Vec<&str> = config.speeds.iter().map(|s| s.name.as_str()).collect();
            return Err(format!(
                "unknown speed \"{}\"; the speeds are: {}",
                wanted,
                names.join(", ")
            ));
        };
        let speed = &config.speeds[i];
        for t in &mut tables {
            t.settings.speed_ms = speed.ms;
            t.speed = speed.name.clone();
        }
        overrides.push(format!("--speed {}", speed.name));
    }
    let port = match options.port {
        Some(p) => {
            overrides.push(format!("--port {p}"));
            p
        }
        None => config.network.port,
    };
    if let Some(s) = seed {
        overrides.push(format!("--seed {s}"));
    }
    let seed = seed.unwrap_or_else(|| holdem::card::Rng::from_entropy().next_u64());
    let settings = tables.iter().map(|t| t.settings.clone()).collect();
    let (server, events) = Server::start_tables(settings, Some(port), seed)
        .map_err(|e| format!("could not listen on port {port}: {e}"))?;
    let port = server.port.unwrap_or(port);
    println!("holdem {APP_VERSION} ({BUILD}) server is running.\n");
    print!("{}", server_report(&tables, port, source, &overrides));
    println!(
        "\nFor players on the internet, forward TCP port {port} to this computer. Press Ctrl-C to stop.\n"
    );
    // One line per event, with the time (UTC) and, with several tables, the table.
    let several = tables.len() > 1;
    while server.is_running() {
        match events.recv_timeout(Duration::from_secs(1)) {
            Ok((table, event)) => {
                let at = clock(SystemTime::now());
                if several {
                    println!("{at}  {table}: {event}");
                } else {
                    println!("{at}  {event}");
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    Ok(())
}

/// The time of day in UTC as HH:MM:SS, without a time-zone library.
fn clock(at: SystemTime) -> String {
    let seconds = at.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use holdem::table::TableEvent;

    use super::*;

    #[test]
    fn the_log_shows_the_time_and_what_happened() {
        let at = UNIX_EPOCH + Duration::from_secs(19 * 86_400 + 3 * 3600 + 4 * 60 + 5);
        assert_eq!(clock(at), "03:04:05");
        let joined = TableEvent::Joined {
            name: "Ann".to_string(),
            people: 1,
        };
        assert_eq!(joined.to_string(), "Ann joined (1 person connected)");
        let started = TableEvent::GameStarted { people: 2, bots: 4 };
        assert_eq!(started.to_string(), "Game started: 2 people and 4 bots");
        let over = TableEvent::GameOver {
            winner: Some("Ann".to_string()),
        };
        assert_eq!(over.to_string(), "Game over: Ann wins");
    }

    #[test]
    fn the_server_report_shows_every_setting_and_its_source() {
        let mut c = Config::default();
        c.tables[0].rules.antes = true;
        c.tables[0].rules.rebuy_levels = 2;
        c.tables[0].fluctuation.enabled = true;
        let s = [ServedTable {
            settings: c.server_tables().remove(0),
            speed: "Normal".to_string(),
        }];
        let text = server_report(
            &s,
            7777,
            Some(Path::new("/srv/holdem.toml")),
            &["--port 7777".to_string()],
        );
        for expected in [
            "settings from         /srv/holdem.toml",
            "command line          --port 7777",
            "\"Hold'em table\"",
            "Ava, Ben, Chloe",
            "Normal (1200 ms per bot move)",
            "1,500",
            "10/20, up every 8 hands",
            "on: 1/8 of the big blind",
            "100% 150% 250%",
            "then +50% per level",
            "during the first 2 levels, for 1,500 chips",
            "a bot leaves with 4%, an empty seat fills with 25% per hand",
            "moving button",
            "everyone shows",
            "30 s",
            "folded after 1 s",
            "protocol versions",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
        let defaults = server_report(&s, 7777, None, &[]);
        assert!(defaults.contains("built-in defaults"));
        assert!(!defaults.contains("command line"));
    }

    #[test]
    fn with_several_tables_the_report_lists_each_and_the_rest_once() {
        let c = Config::default();
        let table = |name: &str, blind: u32, speed: &str| {
            let mut settings = c.server_tables().remove(0);
            settings.name = name.to_string();
            settings.rules.small_blind = blind;
            ServedTable {
                settings,
                speed: speed.to_string(),
            }
        };
        let tables = [table("Low", 5, "Normal"), table("Turbo", 10, "Fast")];
        let text = server_report(&tables, 7777, None, &[]);
        for expected in [
            "Tables",
            "Low                   6 seats, Normal, 1,500 chips, blinds 5/10",
            "Turbo                 6 seats, Fast, 1,500 chips, blinds 10/20",
            "Every table",
            "as listed for each table",
        ] {
            assert!(text.contains(expected), "missing {expected:?} in:\n{text}");
        }
        assert!(
            !text.contains("  name "),
            "the names are in the list:\n{text}"
        );
    }
}
