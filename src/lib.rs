//! Texas Hold'em engine and table server.
//!
//! * `card`, `eval`, `game`: cards, hand evaluation and the rules (no I/O).
//! * `bot`, `stats`: computer players and the playing-style estimate.
//! * `table`, `protocol`, `net`: the table server every game runs on, and how
//!   clients talk to it (in memory for your own seat, TCP for everyone else).
//! * `config`: the optional `holdem.toml` file; `history`: your statistics over
//!   all games, in `stats.toml` next to it; `uninstall`: what `holdem uninstall`
//!   removes.
//!
//! The terminal front end lives in the `holdem` binary.

pub mod bot;
pub mod card;
pub mod config;
pub mod eval;
pub mod game;
pub mod history;
pub mod net;
pub mod protocol;
pub mod stats;
pub mod table;
pub mod uninstall;
