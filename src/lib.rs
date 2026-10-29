//! Texas Hold'em engine and table server.
//!
//! * `card`, `eval`, `game`: cards, hand evaluation and the rules (no I/O).
//! * `bot`, `stats`: computer players and the playing-style estimate.
//! * `table`, `protocol`, `net`: the table server every game runs on, and how
//!   clients talk to it (in memory for your own seat, TCP for everyone else).
//! * `config`: the optional `holdem.toml` file.
//! * `player`: you beyond one game, kept next to `holdem.toml`: your
//!   statistics over all games.
//! * `install`: how the release installer put holdem in place, and what
//!   `holdem uninstall` removes.
//!
//! The terminal front end lives in the `holdem` binary.

pub mod bot;
pub mod card;
pub mod config;
pub mod eval;
pub mod game;
pub mod install;
pub mod net;
pub mod player;
pub mod protocol;
pub mod stats;
pub mod table;
