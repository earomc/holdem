//! Help page: hosting, joining and the server.

use holdem::config::Config;

use super::{H, Page, T};
use crate::keys::{self, Cmd, Place};

pub(super) fn page(d: &Config) -> Page {
    let ready = keys::key(Place::HandOver, Cmd::Ready);
    Page {
        title: "Playing with others",
        items: vec![
            H("HOSTING"),
            T(format!(
                "Choose \"Host a network game\", pick the rules and press Enter. Your table \
                 is open and the lobby shows the address others type to join, such as \
                 192.168.1.20:{}. On the same network (home Wi-Fi, office LAN) others also \
           find it automatically on their join screen. The game starts when everyone in \
           the lobby is ready; bots take the free seats unless you switched that off.",
                d.network.port
            )),
            T(format!(
                "Over the internet, others need your public address, and port {} (or the \
                 port set in holdem.toml) must be forwarded to your computer in your router. \
           Your firewall may ask whether holdem may accept connections.",
                d.network.port
            )),
            H("A SERVER ANYONE CAN JOIN"),
            T(
                "Run \"holdem serve\" on a computer that stays on: a table with no player of \
           its own, running every [[tables]] entry of holdem.toml on one port \
           (\"holdem serve --port 9000\" picks another port; it prints every setting it \
           uses when it starts). After each game it goes \
           back to the lobby for the next one.",
            ),
            H("JOINING"),
            T(
                "Choose \"Join a network game\", type your name, then pick a table found on \
           your network or type an address and press Enter. You can join a game that is \
           already running: you get the next free seat with the starting stack (a bot \
           makes room if the table is full).",
            ),
            H("WHILE PLAYING"),
            T(format!(
                "Each person has a time limit to act ({} seconds by default); when it runs \
                 out they check or fold automatically. After a hand the next one starts \
                 when everyone pressed {}, or after {} seconds. If your connection drops, \
           your hand is folded and you leave the table at the end of the hand; join \
           again with the same name before that to keep your seat.",
                d.tables[0].turn_seconds, ready, d.tables[0].next_hand_seconds
            )),
            T(
                "Nobody can see another player's cards before they are turned up: the table \
           only ever sends you what you could see at a real table.",
            ),
            H("VERSIONS"),
            T(
                "Everyone at a table needs a version of holdem that speaks the same network \
           protocol. Versions are compared as soon as you connect: if yours cannot \
           play with a table, you are told whether you or the host needs to update. \
           Tables found on your network that need a different version are marked on \
           the join screen.",
            ),
        ],
    }
}
