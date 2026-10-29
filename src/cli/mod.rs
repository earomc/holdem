//! The command line: its options and commands, and what they ask for. The
//! commands other than playing live in their own files.

pub(crate) mod config;
pub(crate) mod serve;
pub(crate) mod uninstall;

use std::path::PathBuf;

use clap::builder::{PathBufValueParser, RangedU64ValueParser};
use clap::{Arg, ArgAction, ArgMatches, Command};

// Which build this is. Both releases install a program called `holdem`; the one
// from the `holdem-server` package is built without the `sound` feature.
#[cfg(feature = "sound")]
pub(crate) const BUILD: &str = "with sound";

#[cfg(feature = "sound")]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (with sound)");

#[cfg(feature = "sound")]
const AFTER_HELP: &str = "Without a command, holdem starts the game. Press ? in the game for the \
                          rules, the keys and a glossary of poker terms.\n\n\
                          This build plays sound. The holdem-server release is the same \
                          program without sound, for computers without audio libraries.";

#[cfg(not(feature = "sound"))]
pub(crate) const BUILD: &str = "without sound";

#[cfg(not(feature = "sound"))]
const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (without sound)");

#[cfg(not(feature = "sound"))]
const AFTER_HELP: &str = "Without a command, holdem starts the game. Press ? in the game for the \
                          rules, the keys and a glossary of poker terms.\n\n\
                          This build has no sound (the holdem-server release). The holdem \
                          release is the same program with sound.";

/// The command line, built with clap's builder API.
pub(crate) fn cli() -> Command {
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
                .help("Settings file to use [default: holdem.toml in the current folder, next to the program, or in your settings folder]"),
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
                .about("Run tables that anyone can join, without playing yourself")
                .after_help(
                    "Runs every [[tables]] entry of the settings file on one port; the options \
                     above override single values.",
                )
                .arg(
                    Arg::new("port")
                        .long("port")
                        .short('p')
                        .value_name("NUMBER")
                        .value_parser(RangedU64ValueParser::<u16>::new())
                        .help("Port to listen on, instead of network.port"),
                )
                .arg(
                    Arg::new("name")
                        .long("name")
                        .short('n')
                        .value_name("TEXT")
                        .help("Table name, instead of the name in [[tables]] (with one table)"),
                )
                .arg(
                    Arg::new("speed")
                        .long("speed")
                        .value_name("NAME")
                        .help("Game speed (one of [[speeds]]) for every table, instead of their own"),
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
                                .help("Replace the file if it exists, keeping its values"),
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
        .subcommand(
            Command::new("uninstall")
                .about("Remove holdem, as installed by the release installer")
                .after_help(
                    "Removes the program and the installer's receipt, and takes the program's \
                     folder off the PATH if nothing else is in it. Your settings and statistics \
                     stay unless you choose otherwise.",
                )
                .arg(
                    Arg::new("yes")
                        .long("yes")
                        .short('y')
                        .action(ArgAction::SetTrue)
                        .help("Do not ask: remove the program, keep your settings and statistics"),
                )
                .arg(
                    Arg::new("purge")
                        .long("purge")
                        .action(ArgAction::SetTrue)
                        .help("Also delete your settings and statistics"),
                ),
        )
}

/// What the command line asked for.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Options {
    pub(crate) config: Option<PathBuf>,
    pub(crate) seed: Option<u64>,
    pub(crate) mute: bool,
    pub(crate) action: Action,
}

#[derive(Debug, Default, PartialEq)]
pub(crate) enum Action {
    /// No command: start the game.
    #[default]
    Play,
    Server(ServerOptions),
    Config(ConfigAction),
    Uninstall {
        yes: bool,
        purge: bool,
    },
}

#[derive(Debug, Default, PartialEq)]
pub(crate) struct ServerOptions {
    pub(crate) port: Option<u16>,
    pub(crate) name: Option<String>,
    pub(crate) speed: Option<String>,
}

#[derive(Debug, PartialEq)]
pub(crate) enum ConfigAction {
    Write { file: Option<PathBuf>, force: bool },
    Show,
    Check { file: Option<PathBuf> },
}

impl Options {
    pub(crate) fn from_matches(m: &ArgMatches) -> Options {
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
            Some(("uninstall", u)) => Action::Uninstall {
                yes: u.get_flag("yes"),
                purge: u.get_flag("purge"),
            },
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
            other => panic!("not a config command: {other:?}"),
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
        assert_eq!(
            parse(&["uninstall", "--yes", "--purge"]).unwrap().action,
            Action::Uninstall {
                yes: true,
                purge: true
            }
        );
        assert_eq!(
            parse(&["uninstall"]).unwrap().action,
            Action::Uninstall {
                yes: false,
                purge: false
            }
        );
        assert!(parse(&["serve", "--port", "70000"]).is_err());
        assert!(parse(&["--bogus"]).is_err());
        assert!(parse(&["--config"]).is_err());
        assert!(
            parse(&["--port", "1"]).is_err(),
            "--port belongs to the server"
        );
    }
}
