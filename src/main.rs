//! holdem: no-limit Texas Hold'em in the terminal, against bots or other people.

mod app;
mod help;
mod keys;
mod popups;
mod session;
mod sound;
mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::builder::{PathBufValueParser, RangedU64ValueParser};
use clap::{Arg, ArgAction, ArgMatches, Command};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use holdem::config::{self, Config};
use holdem::game::chips_text;
use holdem::net::{self, Server};
use holdem::protocol::{APP_VERSION, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION};
use holdem::table::TableSettings;

use app::App;
use sound::Sound;

// Which build this is. Both releases install a program called `holdem`; the one
// from the `holdem-server` package is built without the `sound` feature.
#[cfg(feature = "sound")]
const BUILD: &str = "with sound";
#[cfg(feature = "sound")]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (with sound)");
#[cfg(feature = "sound")]
const AFTER_HELP: &str = "Without a command, holdem starts the game. Press ? in the game for the \
                          rules, the keys and a glossary of poker terms.\n\n\
                          This build plays sound. The holdem-server release is the same \
                          program without sound, for computers without audio libraries.";

#[cfg(not(feature = "sound"))]
const BUILD: &str = "without sound";
#[cfg(not(feature = "sound"))]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (without sound)");
#[cfg(not(feature = "sound"))]
const AFTER_HELP: &str = "Without a command, holdem starts the game. Press ? in the game for the \
                          rules, the keys and a glossary of poker terms.\n\n\
                          This build has no sound (the holdem-server release). The holdem \
                          release is the same program with sound.";

/// The command line, built with clap's builder API.
fn cli() -> Command {
    Command::new("holdem")
        .version(VERSION)
        .about("No-limit Texas Hold'em in the terminal: against bots, or with others over the network")
        .after_help(AFTER_HELP)
        .arg(
            Arg::new("config")
                .long("config")
                .short('c')
                .value_name("FILE")
                .value_parser(PathBufValueParser::new())
                .global(true)
                .help("Settings file to use [default: holdem.toml in the current folder or next to the program]"),
        )
        .arg(
            Arg::new("seed")
                .long("seed")
                .value_name("NUMBER")
                .value_parser(RangedU64ValueParser::<u64>::new())
                .global(true)
                .help("Fixed random seed: the same deals every game"),
        )
        // Only the build with sound has --mute.
        .args(cfg!(feature = "sound").then(|| {
            Arg::new("mute")
                .long("mute")
                .action(ArgAction::SetTrue)
                .help("Do not open the audio device at all")
        }))
        .subcommand(
            Command::new("serve")
                .about("Run a table that anyone can join, without playing yourself")
                .after_help(
                    "The table uses the [game] and [server] sections of the settings file; \
                     the options above override single values.",
                )
                .arg(
                    Arg::new("port")
                        .long("port")
                        .short('p')
                        .value_name("NUMBER")
                        .value_parser(RangedU64ValueParser::<u16>::new())
                        .help("Port to listen on, instead of server.port"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .value_name("TEXT")
                        .help("Table name, instead of server.name"),
                )
                .arg(
                    Arg::new("speed")
                        .long("speed")
                        .value_name("NAME")
                        .help("Game speed (one of [[game.speeds]]), instead of server.speed"),
                ),
        )
        .subcommand(
            Command::new("config")
                .about("Write, show or check the settings file")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(
                    Command::new("write")
                        .about("Write a settings file with every setting explained and at its default")
                        .arg(
                            Arg::new("file")
                                .value_name("FILE")
                                .value_parser(PathBufValueParser::new())
                                .help("Where to write it [default: the --config file, or holdem.toml]"),
                        )
                        .arg(
                            Arg::new("force")
                                .long("force")
                                .short('f')
                                .action(ArgAction::SetTrue)
                                .help("Replace the file if it exists"),
                        ),
                )
                .subcommand(
                    Command::new("show")
                        .about("Print the settings holdem would use, and the file they come from"),
                )
                .subcommand(
                    Command::new("check")
                        .about("Check a settings file for mistakes without starting anything")
                        .arg(
                            Arg::new("file")
                                .value_name("FILE")
                                .value_parser(PathBufValueParser::new())
                                .help("The file to check [default: the one holdem would use]"),
                        ),
                ),
        )
}

/// What the command line asked for.
#[derive(Debug, Default, PartialEq)]
struct Options {
    config: Option<PathBuf>,
    seed: Option<u64>,
    mute: bool,
    action: Action,
}

#[derive(Debug, Default, PartialEq)]
enum Action {
    /// No command: start the game.
    #[default]
    Play,
    Server(ServerOptions),
    Config(ConfigAction),
}

#[derive(Debug, Default, PartialEq)]
struct ServerOptions {
    port: Option<u16>,
    name: Option<String>,
    speed: Option<String>,
}

#[derive(Debug, PartialEq)]
enum ConfigAction {
    Write { file: Option<PathBuf>, force: bool },
    Show,
    Check { file: Option<PathBuf> },
}

impl Options {
    fn from_matches(m: &ArgMatches) -> Options {
        let file = |m: &ArgMatches| m.get_one::<PathBuf>("file").cloned();
        let action = match m.subcommand() {
            Some(("serve", s)) => Action::Server(ServerOptions {
                port: s.get_one::<u16>("port").copied(),
                name: s.get_one::<String>("name").cloned(),
                speed: s.get_one::<String>("speed").cloned(),
            }),
            Some(("config", c)) => Action::Config(match c.subcommand() {
                Some(("write", w)) => ConfigAction::Write {
                    file: file(w),
                    force: w.get_flag("force"),
                },
                Some(("check", k)) => ConfigAction::Check { file: file(k) },
                Some(("show", _)) => ConfigAction::Show,
                other => unreachable!("clap requires a config command, got {:?}", other),
            }),
            None => Action::Play,
            Some((other, _)) => unreachable!("unknown command {}", other),
        };
        Options {
            config: m.get_one::<PathBuf>("config").cloned(),
            seed: m.get_one::<u64>("seed").copied(),
            // clap panics on reading an argument it does not know.
            mute: cfg!(feature = "sound") && m.get_flag("mute"),
            action,
        }
    }
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(40))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
        app.tick();
    }
}

fn on_off(value: bool, on: &str, off: &str) -> String {
    if value { on } else { off }.to_string()
}

/// Everything a dedicated server runs with, as labelled lines.
fn server_report(
    settings: &TableSettings,
    speed_name: &str,
    port: u16,
    source: Option<&Path>,
    overrides: &[String],
) -> String {
    let r = &settings.rules;
    let mut out = String::new();
    let mut section = |title: &str, rows: Vec<(&str, String)>| {
        out.push_str(title);
        out.push('\n');
        for (label, value) in rows {
            out.push_str(&format!("  {:<22}{}\n", label, value));
        }
    };
    let source = source_text(source);
    let mut origin = vec![("settings from", source)];
    if !overrides.is_empty() {
        origin.push(("command line", overrides.join(", ")));
    }
    section("Configuration", origin);

    let lan = match net::local_ip() {
        Some(ip) => format!("{}:{}", ip, port),
        None => format!("this computer's address, port {}", port),
    };
    let fluctuation = match &settings.fluctuation {
        Some(f) => format!(
            "on: a bot leaves with {:.0}%, an empty seat fills with {:.0}% per hand",
            f.leave_chance * 100.0,
            f.join_chance * 100.0
        ),
        None => "off".to_string(),
    };
    section(
        "Table",
        vec![
            ("name", format!("\"{}\"", settings.name)),
            ("port", format!("{} (on this network: {})", port, lan)),
            ("seats", settings.seats.to_string()),
            (
                "free seats",
                on_off(
                    settings.fill_with_bots,
                    "filled with bots",
                    "left empty (people only)",
                ),
            ),
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
        ],
    );

    let schedule: Vec<String> = r.blind_schedule.iter().map(|p| format!("{}%", p)).collect();
    section(
        "Rules",
        vec![
            ("starting chips", chips_text(r.starting_chips)),
            (
                "blinds",
                format!("{}, up {}", r.blinds_text(), r.levels_text()),
            ),
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
        ],
    );

    let seconds = |s: u32, none: &str| {
        if s == 0 {
            none.to_string()
        } else {
            format!("{} s", s)
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
                    "holdem {} ({}), protocol versions {} to {}",
                    APP_VERSION, BUILD, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION
                ),
            ),
        ],
    );
    out
}

/// `holdem serve`: a table with no player of its own, until Ctrl-C.
fn run_server(
    config: &Config,
    source: Option<&Path>,
    options: &ServerOptions,
    seed: Option<u64>,
) -> Result<(), String> {
    let mut overrides = Vec::new();
    let mut settings = config.server_table();
    let mut speed = config.speed(&config.server.speed);
    if let Some(name) = &options.name {
        settings.name = name.clone();
        overrides.push(format!("--name \"{}\"", name));
    }
    if let Some(wanted) = &options.speed {
        let Some(i) = config.speed_index(wanted) else {
            let names: Vec<&str> = config.game.speeds.iter().map(|s| s.name.as_str()).collect();
            return Err(format!(
                "unknown speed \"{}\"; the speeds are: {}",
                wanted,
                names.join(", ")
            ));
        };
        speed = &config.game.speeds[i];
        settings.speed_ms = speed.ms;
        overrides.push(format!("--speed {}", speed.name));
    }
    let port = match options.port {
        Some(p) => {
            overrides.push(format!("--port {}", p));
            p
        }
        None => config.server.port,
    };
    if let Some(s) = seed {
        overrides.push(format!("--seed {}", s));
    }
    let seed = seed.unwrap_or_else(|| holdem::card::Rng::from_entropy().next_u64());
    let server = Server::start(settings.clone(), Some(port), seed)
        .map_err(|e| format!("could not listen on port {}: {}", port, e))?;
    let port = server.port.unwrap_or(port);
    println!("holdem {} ({}) server is running.\n", APP_VERSION, BUILD);
    print!(
        "{}",
        server_report(&settings, &speed.name, port, source, &overrides)
    );
    println!(
        "\nFor players on the internet, forward TCP port {} to this computer. Press Ctrl-C to stop.\n",
        port
    );
    let mut last = None;
    while server.is_running() {
        std::thread::sleep(Duration::from_secs(1));
        let status = server.status();
        let summary = status.as_ref().map(|a| (a.players, a.playing));
        if summary != last {
            if let Some(a) = status {
                println!(
                    "{} {} connected",
                    if a.playing { "Game running:" } else { "Lobby:" },
                    a.players
                );
            }
            last = summary;
        }
    }
    Ok(())
}

/// Where the settings came from, for people to read.
fn source_text(source: Option<&Path>) -> String {
    match source {
        Some(p) => p.display().to_string(),
        None => format!(
            "built-in defaults (no {} found; `holdem config write` writes one)",
            config::FILE_NAME
        ),
    }
}

/// The full path, so it is clear which file is meant.
fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

/// `holdem config ...`
fn run_config(action: &ConfigAction, chosen: Option<&Path>) -> Result<(), String> {
    match action {
        ConfigAction::Write { file, force } => {
            let path = file
                .as_deref()
                .or(chosen)
                .unwrap_or(Path::new(config::FILE_NAME));
            if path.exists() && !force {
                return Err(format!(
                    "{} already exists; add --force to replace it",
                    path.display()
                ));
            }
            std::fs::write(path, config::template())
                .map_err(|e| format!("could not write {}: {}", path.display(), e))?;
            println!("Wrote {}", absolute(path.to_path_buf()).display());
        }
        ConfigAction::Show => {
            let (config, source) = Config::load(chosen)?;
            println!(
                "# Settings from {}\n",
                source_text(source.map(absolute).as_deref())
            );
            print!("{}", config::render(&config));
        }
        ConfigAction::Check { file } => {
            let Some(path) = Config::locate(file.as_deref().or(chosen)) else {
                println!("{}", source_text(None));
                return Ok(());
            };
            let config = Config::read(&path)?;
            let changed = config::differences(&config);
            println!(
                "{} is fine: {}",
                absolute(path).display(),
                match changed.len() {
                    0 => "every setting is at its default".to_string(),
                    1 => format!("1 setting differs from the default: {}", changed[0]),
                    n => format!(
                        "{} settings differ from the defaults: {}",
                        n,
                        changed.join(", ")
                    ),
                }
            );
        }
    }
    Ok(())
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {}", message);
    std::process::exit(1);
}

fn main() {
    // clap prints help, the version and usage errors itself, and exits.
    let options = Options::from_matches(&cli().get_matches());
    let chosen = options.config.as_deref();
    let (config, source) = match &options.action {
        Action::Config(action) => {
            if let Err(e) = run_config(action, chosen) {
                fail(e);
            }
            return;
        }
        _ => match Config::load(chosen) {
            Ok((c, source)) => (c, source.map(absolute)),
            Err(e) => fail(format!("in the settings file: {}", e)),
        },
    };
    if let Action::Server(server) = &options.action {
        if let Err(e) = run_server(&config, source.as_deref(), server, options.seed) {
            fail(e);
        }
        return;
    }
    // Open the audio device before taking over the terminal: audio back ends may
    // print warnings, which would garble the screen later on.
    let sound = Sound::new(options.mute);
    let mut app = App::new(config, options.seed, sound);
    app.config_note = format!("Settings: {}", source_text(source.as_deref()));
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    if let Err(e) = result {
        fail(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, clap::Error> {
        let mut all = vec!["holdem"];
        all.extend_from_slice(args);
        cli()
            .try_get_matches_from(all)
            .map(|m| Options::from_matches(&m))
    }

    #[test]
    fn the_command_line_is_well_formed() {
        cli().debug_assert();
    }

    #[test]
    fn command_lines() {
        let o = parse(&["serve", "--port", "9000", "-n", "Den", "--speed", "fast"]).unwrap();
        let Action::Server(s) = o.action else {
            panic!("not the server")
        };
        assert_eq!(s.port, Some(9000));
        assert_eq!(s.name.as_deref(), Some("Den"));
        assert_eq!(s.speed.as_deref(), Some("fast"));
        // Global options work before and after the command.
        let o = parse(&["--config", "a.toml", "serve", "--seed", "4"]).unwrap();
        assert_eq!(o.config, Some(PathBuf::from("a.toml")));
        assert_eq!(o.seed, Some(4));
        let o = parse(&["serve", "-c", "b.toml"]).unwrap();
        assert_eq!(o.config, Some(PathBuf::from("b.toml")));
        let config = |o: Options| match o.action {
            Action::Config(c) => c,
            other => panic!("not a config command: {:?}", other),
        };
        assert_eq!(
            config(parse(&["config", "write"]).unwrap()),
            ConfigAction::Write {
                file: None,
                force: false
            }
        );
        assert_eq!(
            config(parse(&["config", "write", "my.toml", "--force"]).unwrap()),
            ConfigAction::Write {
                file: Some(PathBuf::from("my.toml")),
                force: true
            }
        );
        assert_eq!(
            config(parse(&["config", "show"]).unwrap()),
            ConfigAction::Show
        );
        let o = parse(&["-c", "x.toml", "config", "check"]).unwrap();
        assert_eq!(o.config, Some(PathBuf::from("x.toml")));
        assert_eq!(config(o), ConfigAction::Check { file: None });
        assert!(parse(&["config"]).is_err(), "config needs a command");
        assert!(
            parse(&["--write-config"]).is_err(),
            "replaced by `config write`"
        );
        if cfg!(feature = "sound") {
            assert!(parse(&["--mute"]).unwrap().mute);
        } else {
            assert!(parse(&["--mute"]).is_err(), "no --mute without sound");
        }
        assert_eq!(parse(&[]).unwrap(), Options::default());
        assert!(parse(&["--seed", "x"]).is_err());
        assert!(parse(&["serve", "--port", "70000"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--config"]).is_err());
        assert!(
            parse(&["--port", "1"]).is_err(),
            "--port belongs to the server"
        );
    }

    #[test]
    fn the_server_report_shows_every_setting_and_its_source() {
        let mut c = Config::default();
        c.game.rules.antes = true;
        c.game.rules.rebuy_levels = 2;
        c.game.fluctuation.enabled = true;
        let s = c.server_table();
        let text = server_report(
            &s,
            "Normal",
            7777,
            Some(Path::new("/srv/holdem.toml")),
            &["--port 7777".to_string()],
        );
        for expected in [
            "settings from         /srv/holdem.toml",
            "command line          --port 7777",
            "\"Hold'em table\"",
            "Ava, Ben, Chloe",
            "Normal (650 ms per bot move)",
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
            assert!(
                text.contains(expected),
                "missing {:?} in:\n{}",
                expected,
                text
            );
        }
        let defaults = server_report(&s, "Normal", 7777, None, &[]);
        assert!(defaults.contains("built-in defaults"));
        assert!(!defaults.contains("command line"));
    }
}
