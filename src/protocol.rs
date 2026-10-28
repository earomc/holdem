//! Messages between a table server and its clients, sent as one JSON object
//! per line. The server is authoritative: a client only ever learns what its
//! player could see at a real table (its own hole cards, and other players'
//! cards once they are turned up).
//!
//! # Versions
//!
//! Every program speaks a range of protocol versions, from
//! [`MIN_PROTOCOL_VERSION`] to [`PROTOCOL_VERSION`]. The first messages
//! (`Hello`, `Welcome`, `Goodbye`) keep the same shape forever, so two programs
//! of any age can always tell each other whether they can play together:
//!
//! * the client says which versions it speaks in `Hello`;
//! * the server picks the newest version both speak ([`negotiate`]), or says
//!   in plain words in `Goodbye` who needs to update;
//! * LAN discovery announces the versions too, so the join screen can mark
//!   tables that need a newer or older program before connecting.
//!
//! Within a version, new fields always have defaults and unknown fields are
//! ignored, and a message of an unknown kind is skipped rather than treated as
//! an error. Bump [`PROTOCOL_VERSION`] when a change cannot be understood that
//! way, and raise [`MIN_PROTOCOL_VERSION`] when old versions are no longer
//! supported.

use serde::{Deserialize, Serialize};

use crate::card::Card;
use crate::game::{Action, GameEvent, HandResult, Legal, Rules, Standing, State, Street};
use crate::stats::Stats;

/// The newest protocol version this program speaks. Version 4 added several
/// tables per server: `Hello` may name a table, and a server with several
/// answers a `Hello` that names none with [`ServerMsg::Tables`].
pub const PROTOCOL_VERSION: u32 = 4;
/// The oldest protocol version this program still speaks.
pub const MIN_PROTOCOL_VERSION: u32 = 3;
/// This program's version, shown when versions do not match.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_PORT: u16 = 7777;
/// Longest player name, in characters.
pub const MAX_NAME: usize = 16;

/// From a client to the server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMsg {
    /// First message on every connection. Its shape never changes.
    Hello {
        name: String,
        /// The newest protocol version the client speaks.
        version: u32,
        /// The oldest one (missing from programs before protocol 3: same as `version`).
        #[serde(default)]
        min_version: Option<u32>,
        /// The client program's version, for messages.
        #[serde(default)]
        app: String,
        /// The table to join, by name, at a server with several (protocol 4).
        #[serde(default)]
        table: Option<String>,
    },
    /// The table to join, chosen from [`ServerMsg::Tables`] (protocol 4).
    ChooseTable(String),
    /// A decision for the turn identified by `turn` (from [`TableView::turn`]).
    /// Answers to an earlier turn are ignored.
    Act { action: Action, turn: u64 },
    /// In the lobby: ready to start. After a hand: deal the next one. After the
    /// game: back to the lobby.
    Ready,
    /// Answer to a rebuy offer: true buys back in, false gives up.
    Rebuy(bool),
    /// Pause the game (only honoured for the table owner while no other people are playing).
    Pause(bool),
    /// Milliseconds between bot moves (table owner only).
    SetSpeed(u32),
}

impl ClientMsg {
    /// The `Hello` this program sends, for the table called `table` if the
    /// server has several.
    pub fn hello(name: &str, table: Option<&str>) -> ClientMsg {
        ClientMsg::Hello {
            name: name.to_string(),
            version: PROTOCOL_VERSION,
            min_version: Some(MIN_PROTOCOL_VERSION),
            app: APP_VERSION.to_string(),
            table: table.map(str::to_string),
        }
    }
}

/// From the server to a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMsg {
    /// Answer to `Hello`: you are in, with this (possibly changed) name.
    Welcome {
        name: String,
        table: String,
        /// The protocol version the server chose for this connection.
        #[serde(default)]
        version: u32,
        /// The server program's version.
        #[serde(default)]
        app: String,
    },
    /// The server's tables, to choose one with [`ClientMsg::ChooseTable`]. Sent
    /// instead of `Welcome` when the server has several and `Hello` named none
    /// of them (protocol 4).
    Tables(Vec<Announcement>),
    Lobby(LobbyView),
    Table(Box<TableView>),
    /// A short message to show (an illegal action, a refused request...).
    Notice(String),
    /// The server is closing this connection, and why. Its shape never changes.
    Goodbye(String),
}

/// Why two programs cannot play together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mismatch {
    /// The other side only speaks protocol versions older than ours.
    TheyAreOlder { theirs: u32, ours_min: u32 },
    /// The other side only speaks protocol versions newer than ours.
    TheyAreNewer { their_min: u32, ours: u32 },
}

impl Mismatch {
    /// Said by a table to a client that cannot join.
    pub fn to_client(&self) -> String {
        match *self {
            Mismatch::TheyAreOlder { theirs, ours_min } => format!(
                "Your holdem is too old for this table (it speaks protocol {theirs}, the table needs {ours_min} or newer). Please update holdem."
            ),
            Mismatch::TheyAreNewer { their_min, ours } => format!(
                "This table runs an older holdem (holdem {APP_VERSION}, protocol {ours}) than yours (protocol {their_min} or newer). Ask the host to update holdem."
            ),
        }
    }

    /// Explains to this program's user why a table found on the network cannot be joined.
    pub fn to_client_about_table(&self, table: &str) -> String {
        match *self {
            Mismatch::TheyAreOlder { .. } => format!(
                "\"{table}\" runs an older version of holdem that this version cannot play with. Ask its host to update."
            ),
            Mismatch::TheyAreNewer { .. } => format!(
                "\"{table}\" runs a newer version of holdem. Please update yours (you have holdem {APP_VERSION})."
            ),
        }
    }

    /// Said by a client about a table it cannot join.
    pub fn about_table(&self) -> String {
        match *self {
            Mismatch::TheyAreOlder { .. } => "runs an older holdem".to_string(),
            Mismatch::TheyAreNewer { .. } => "needs a newer holdem".to_string(),
        }
    }
}

/// Picks the newest protocol version both sides speak, given that the other
/// side speaks `peer_min..=peer_version`.
pub fn negotiate(peer_version: u32, peer_min: Option<u32>) -> Result<u32, Mismatch> {
    let peer_min = peer_min.unwrap_or(peer_version).min(peer_version);
    let agreed = peer_version.min(PROTOCOL_VERSION);
    if agreed < MIN_PROTOCOL_VERSION {
        return Err(Mismatch::TheyAreOlder {
            theirs: peer_version,
            ours_min: MIN_PROTOCOL_VERSION,
        });
    }
    if agreed < peer_min {
        return Err(Mismatch::TheyAreNewer {
            their_min: peer_min,
            ours: PROTOCOL_VERSION,
        });
    }
    Ok(agreed)
}

fn yes() -> bool {
    true
}

fn default_min_stack() -> u32 {
    crate::stats::MIN_STACK_BB
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LobbyPlayer {
    pub name: String,
    pub ready: bool,
    pub you: bool,
}

/// The waiting room before a game.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LobbyView {
    pub table: String,
    pub players: Vec<LobbyPlayer>,
    pub seats: usize,
    pub fill_with_bots: bool,
    pub fluctuation: bool,
    pub rules: Rules,
    /// Players may turn on the win-chance hint at this table.
    #[serde(default = "yes")]
    pub win_chance_allowed: bool,
}

/// One seat as a particular viewer sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeatView {
    /// Empty for a vacant seat.
    pub name: String,
    pub is_human: bool,
    /// False for a human whose connection dropped. (Missing means connected,
    /// so a peer that does not send it shows nobody as away.)
    #[serde(default = "yes")]
    pub connected: bool,
    pub chips: u32,
    pub bet: u32,
    pub in_hand: bool,
    pub folded: bool,
    pub all_in: bool,
    pub out: bool,
    pub vacant: bool,
    /// Place of an eliminated player.
    pub place: Option<usize>,
    pub last_action: String,
    /// Face-up cards: your own, or anyone's that were shown.
    pub cards: Vec<Card>,
    /// Holds cards the viewer cannot see.
    pub hidden_cards: bool,
    pub payout: u32,
    pub rebuy_pending: bool,
    pub rebuys: u32,
}

/// The whole table as one client sees it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TableView {
    /// Your seat, if you have one.
    pub you: Option<usize>,
    pub seats: Vec<SeatView>,
    pub board: Vec<Card>,
    pub state: State,
    pub street: Street,
    /// Chips in the middle now, and how they split into main and side pots.
    pub pot: u32,
    pub pots: Vec<u32>,
    /// Size of the pot that was just won (after a hand).
    pub last_pot: u32,
    pub hand_no: u32,
    pub level: u32,
    pub sb: u32,
    pub bb: u32,
    pub ante: u32,
    pub rules: Rules,
    pub button: usize,
    pub sb_seat: usize,
    pub sb_dead: bool,
    pub bb_seat: usize,
    pub straddle_seat: Option<usize>,
    pub to_act: Option<usize>,
    pub current_bet: u32,
    /// What you may do; only present on your turn.
    pub legal: Option<Legal>,
    /// Identifies the decision being awaited; send it back with your action.
    pub turn: u64,
    /// "Pocket Kings", "Two Pair, Aces and Sevens"...
    pub your_hand: Option<String>,
    pub result_lines: Vec<String>,
    /// Log lines that are new since the previous view.
    pub new_log: Vec<String>,
    /// What happened since the previous view (for sounds).
    pub events: Vec<GameEvent>,
    /// You busted and may buy back in for this many chips.
    pub rebuy_offer: Option<u32>,
    /// The next hand would end the game.
    pub last_hand: bool,
    pub game_over: bool,
    pub standings: Vec<Standing>,
    /// Your statistics (for the style estimate).
    pub your_stats: Stats,
    /// How your last settled hand went (it stays until the next one is settled).
    pub your_result: Option<HandResult>,
    /// Seconds left for the player to act, when there is a time limit.
    pub turn_seconds_left: Option<u32>,
    /// Seconds until the next hand starts by itself.
    pub next_hand_seconds_left: Option<u32>,
    /// Players who said they are ready for the next hand, and how many are needed.
    pub ready: (usize, usize),
    /// You already said you are ready.
    pub you_ready: bool,
    /// People waiting for a free seat.
    pub waiting: Vec<String>,
    pub paused: bool,
    pub speed_ms: u32,
    /// You may pause and change the speed.
    pub owner: bool,
    /// More than one person is connected.
    pub multiplayer: bool,
    /// Players may turn on the win-chance hint at this table.
    #[serde(default = "yes")]
    pub win_chance_allowed: bool,
    /// Hands started with fewer big blinds do not count for the style estimate.
    #[serde(default = "default_min_stack")]
    pub stats_min_stack_bb: u32,
}

/// Cleans up a player name: printable characters only, at most [`MAX_NAME`].
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "Player".to_string()
    } else {
        cleaned
    }
}

/// Answer to a LAN discovery request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Announcement {
    pub table: String,
    pub port: u16,
    pub players: usize,
    pub seats: usize,
    pub playing: bool,
    /// Newest protocol version the table speaks.
    pub version: u32,
    /// Oldest one (missing before protocol 3: same as `version`).
    pub min_version: Option<u32>,
}

impl Announcement {
    /// Whether this program can join the table, and if not, why.
    pub fn compatibility(&self) -> Result<u32, Mismatch> {
        negotiate(self.version, self.min_version)
    }
}

/// What a client broadcasts to find tables on the local network.
pub const DISCOVERY_REQUEST: &[u8] = b"HOLDEM-DISCOVER";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_round_trip_through_json() {
        let msgs = vec![
            ClientMsg::Hello {
                name: "Ann".into(),
                version: PROTOCOL_VERSION,
                min_version: Some(MIN_PROTOCOL_VERSION),
                app: APP_VERSION.into(),
                table: None,
            },
            ClientMsg::Act {
                action: Action::RaiseTo(120),
                turn: 7,
            },
            ClientMsg::Ready,
            ClientMsg::Rebuy(true),
            ClientMsg::ChooseTable("Turbo".into()),
        ];
        for m in msgs {
            let line = serde_json::to_string(&m).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(serde_json::from_str::<ClientMsg>(&line).unwrap(), m);
        }
    }

    #[test]
    fn the_list_of_tables_round_trips() {
        let list = ServerMsg::Tables(vec![Announcement {
            table: "Turbo".into(),
            port: 7777,
            players: 2,
            seats: 6,
            playing: true,
            version: PROTOCOL_VERSION,
            min_version: Some(MIN_PROTOCOL_VERSION),
        }]);
        let line = serde_json::to_string(&list).unwrap();
        assert_eq!(serde_json::from_str::<ServerMsg>(&line).unwrap(), list);
    }

    #[test]
    fn versions_are_negotiated() {
        assert_eq!(negotiate(PROTOCOL_VERSION, None), Ok(PROTOCOL_VERSION));
        // A newer peer that still speaks our version: use ours.
        assert_eq!(
            negotiate(PROTOCOL_VERSION + 3, Some(MIN_PROTOCOL_VERSION)),
            Ok(PROTOCOL_VERSION)
        );
        // A peer that only speaks older versions.
        let old = negotiate(MIN_PROTOCOL_VERSION - 1, None).unwrap_err();
        assert!(matches!(old, Mismatch::TheyAreOlder { .. }));
        assert!(old.to_client().contains("Please update holdem"));
        assert_eq!(old.about_table(), "runs an older holdem");
        // A peer that only speaks newer versions.
        let new = negotiate(PROTOCOL_VERSION + 5, Some(PROTOCOL_VERSION + 1)).unwrap_err();
        assert!(matches!(new, Mismatch::TheyAreNewer { .. }));
        assert!(new.to_client().contains("Ask the host to update"));
        assert_eq!(new.about_table(), "needs a newer holdem");
    }

    #[test]
    fn the_first_messages_of_protocol_2_still_parse() {
        // What holdem 0.3 sends and expects: the handshake must stay readable.
        let hello: ClientMsg =
            serde_json::from_str(r#"{"Hello":{"name":"Old","version":2}}"#).unwrap();
        assert!(matches!(
            hello,
            ClientMsg::Hello {
                version: 2,
                min_version: None,
                ..
            }
        ));
        let bye = serde_json::to_string(&ServerMsg::Goodbye("x".into())).unwrap();
        assert_eq!(bye, r#"{"Goodbye":"x"}"#);
        let welcome = serde_json::to_string(&ServerMsg::Welcome {
            name: "a".into(),
            table: "t".into(),
            version: 3,
            app: "0.4.0".into(),
        })
        .unwrap();
        assert!(welcome.starts_with(r#"{"Welcome":{"name":"a","table":"t""#));
    }

    #[test]
    fn views_tolerate_missing_and_unknown_fields() {
        let v: TableView =
            serde_json::from_str(r#"{"hand_no": 4, "some_future_field": [1, 2]}"#).unwrap();
        assert_eq!(v.hand_no, 4);
        assert!(
            v.win_chance_allowed,
            "missing permission defaults to allowed"
        );
        assert_eq!(v.stats_min_stack_bb, crate::stats::MIN_STACK_BB);
        let seat: SeatView = serde_json::from_str(r#"{"name": "Ann"}"#).unwrap();
        assert!(seat.connected, "missing means connected");
        let a: Announcement = serde_json::from_str(
            r#"{"table":"T","port":7777,"players":1,"seats":6,"playing":false,"version":2}"#,
        )
        .unwrap();
        assert!(a.compatibility().is_err());
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Ann  "), "Ann");
        assert_eq!(clean_name("\u{1b}[31mEvil"), "[31mEvil");
        assert_eq!(clean_name(""), "Player");
        assert_eq!(
            clean_name("abcdefghijklmnopqrstuvwxyz").chars().count(),
            MAX_NAME
        );
    }
}
