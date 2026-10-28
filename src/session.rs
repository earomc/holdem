//! One game as the client sees it: the connection to a table server (which we
//! may be running ourselves), and everything the server has told us.

use holdem::game::HandResult;
use holdem::net::{Connection, Server};
use holdem::protocol::{Announcement, ClientMsg, LobbyView, ServerMsg, TableView};
use holdem::stats::Stats;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// Just you and bots, on a table running inside this program.
    Solo,
    /// A table running inside this program that others can join over the network.
    Host,
    /// Someone else's table.
    Guest,
}

pub struct Session {
    pub kind: Kind,
    conn: Connection,
    /// Our own table server (solo and host games).
    server: Option<Server>,
    /// Your name as the table knows it.
    pub name: String,
    pub table: String,
    pub lobby: Option<LobbyView>,
    pub view: Option<TableView>,
    /// The hand log, accumulated from the views.
    pub log: Vec<String>,
    /// How many log lines to keep.
    log_keep: usize,
    /// Your hands in this game, oldest first.
    pub my_hands: Vec<HandResult>,
    /// Your statistics this game, as last seen while you had a seat (the
    /// views stop carrying them once you are knocked out).
    pub game_stats: Stats,
    /// This game's statistics have been added to your saved ones.
    pub stats_saved: bool,
    /// Where others can reach a hosted table, e.g. "192.168.1.20:7777".
    pub share: Option<String>,
    /// Set when the table hung up on us.
    pub closed: Option<String>,
    /// The table's program version (empty for tables older than holdem 0.4).
    pub server_app: String,
    /// The server's tables, while it waits for us to choose one.
    pub choices: Option<Vec<Announcement>>,
    paused: bool,
}

/// What the app should react to after reading the connection.
#[derive(Default)]
pub struct Update {
    pub events: Vec<holdem::game::GameEvent>,
    pub notices: Vec<String>,
    pub changed: bool,
}

impl Session {
    pub fn new(
        kind: Kind,
        conn: Connection,
        server: Option<Server>,
        name: &str,
        table: Option<&str>,
        log_keep: usize,
    ) -> Session {
        conn.send(ClientMsg::hello(name, table));
        let share =
            server
                .as_ref()
                .and_then(|s| s.port)
                .map(|port| match holdem::net::local_ip() {
                    Some(ip) => format!("{ip}:{port}"),
                    None => format!("this computer's address, port {port}"),
                });
        Session {
            kind,
            conn,
            server,
            name: name.to_string(),
            table: String::new(),
            lobby: None,
            view: None,
            log: Vec::new(),
            log_keep: log_keep.max(1),
            my_hands: Vec::new(),
            game_stats: Stats::default(),
            stats_saved: false,
            share,
            closed: None,
            server_app: String::new(),
            choices: None,
            paused: false,
        }
    }

    pub fn send(&self, msg: ClientMsg) {
        self.conn.send(msg);
    }

    /// Reads everything the table sent since the last call.
    pub fn poll(&mut self) -> Update {
        let mut update = Update::default();
        while let Ok(msg) = self.conn.incoming.try_recv() {
            update.changed = true;
            match msg {
                ServerMsg::Welcome {
                    name, table, app, ..
                } => {
                    self.name = name;
                    self.table = table;
                    self.server_app = app;
                    self.choices = None;
                }
                ServerMsg::Tables(tables) => self.choices = Some(tables),
                ServerMsg::Lobby(lobby) => {
                    self.lobby = Some(lobby);
                    self.view = None;
                    self.log.clear();
                    self.my_hands.clear();
                    self.game_stats = Stats::default();
                    self.stats_saved = false;
                }
                ServerMsg::Table(view) => {
                    let mut view = *view;
                    self.log.append(&mut view.new_log);
                    if self.log.len() > self.log_keep {
                        let extra = self.log.len() - self.log_keep;
                        self.log.drain(0..extra);
                    }
                    update.events.append(&mut view.events);
                    if let Some(r) = &view.your_result
                        && self.my_hands.last().is_none_or(|h| h.hand != r.hand)
                    {
                        self.my_hands.push(r.clone());
                    }
                    if view.you.is_some() {
                        self.game_stats = view.your_stats;
                    }
                    self.view = Some(view);
                }
                ServerMsg::Notice(text) => update.notices.push(text),
                ServerMsg::Goodbye(reason) => {
                    self.closed = Some(reason);
                    break;
                }
            }
        }
        update
    }

    /// Joins the table called `name`, from the server's list.
    pub fn choose(&mut self, name: &str) {
        self.send(ClientMsg::ChooseTable(name.to_string()));
    }

    /// Pauses a solo game while a window is open.
    pub fn set_paused(&mut self, paused: bool) {
        let allowed = self
            .view
            .as_ref()
            .is_some_and(|v| v.owner && !v.multiplayer);
        if paused != self.paused && (allowed || !paused) {
            self.paused = paused;
            self.send(ClientMsg::Pause(paused));
        }
    }

    pub fn is_owner(&self) -> bool {
        self.kind != Kind::Guest
    }

    /// Stops our own server, if we run one. (The connection closes on drop.)
    pub fn shutdown(&mut self) {
        if let Some(mut s) = self.server.take() {
            s.stop();
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shutdown();
    }
}
