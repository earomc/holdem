use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

use super::server::MAX_CLIENT_LINE;
use super::*;
use crate::game::{Action, State};
use crate::protocol::TableView;
use crate::protocol::{ClientMsg, ServerMsg};
use crate::table::TableSettings;

fn hello(c: &Connection, name: &str) {
    c.send(ClientMsg::hello(name, None));
}

/// Waits for a table view that satisfies `pred`.
fn wait_for(c: &Connection, pred: impl Fn(&TableView) -> bool) -> TableView {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        assert!(!left.is_zero(), "timed out waiting for the table");
        if let Ok(ServerMsg::Table(v)) = c.incoming.recv_timeout(left)
            && pred(&v)
        {
            return *v;
        }
    }
}

#[test]
fn a_host_and_a_network_player_play_together() {
    let settings = TableSettings {
        seats: 3,
        speed_ms: 20,
        ..TableSettings::default()
    };
    let server = Server::start(settings, Some(0), 42).expect("the server starts");
    let port = server.port.unwrap();
    let host = server.connect_local(true);
    hello(&host, "Host");
    let guest = connect("127.0.0.1", port, Duration::from_secs(5)).expect("connects");
    hello(&guest, "Guest");
    // Both say they are ready; the game starts with one bot.
    thread::sleep(Duration::from_millis(100));
    host.send(ClientMsg::Ready);
    guest.send(ClientMsg::Ready);
    let v = wait_for(&guest, |v| v.hand_no >= 1);
    assert_eq!(v.seats.len(), 3);
    let me = v.you.expect("the guest has a seat");
    assert_eq!(v.seats[me].name, "Guest");
    // Play a few hands: both people fold or check whenever it is their turn.
    let mut hands = 0;
    let deadline = Instant::now() + Duration::from_secs(30);
    while hands < 3 && Instant::now() < deadline {
        for c in [&host, &guest] {
            while let Ok(m) = c.incoming.try_recv() {
                if let ServerMsg::Table(v) = m {
                    if let Some(l) = v.legal {
                        c.send(ClientMsg::Act {
                            action: if l.can_check {
                                Action::Check
                            } else {
                                Action::Fold
                            },
                            turn: v.turn,
                        });
                    }
                    if v.state == State::HandOver && !v.you_ready {
                        c.send(ClientMsg::Ready);
                    }
                    hands = hands.max(v.hand_no);
                    for (i, s) in v.seats.iter().enumerate() {
                        if Some(i) != v.you && !s.cards.is_empty() {
                            assert!(
                                v.state == State::HandOver || v.state == State::Runout,
                                "cards are only ever revealed at the end"
                            );
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(hands >= 3, "only {hands} hands were played");
}

#[test]
fn discovery_finds_a_table_on_this_computer() {
    let settings = TableSettings {
        name: "Kitchen table".to_string(),
        ..TableSettings::default()
    };
    let server = Server::start(settings, Some(0), 1).expect("the server starts");
    let port = server.port.unwrap();
    thread::sleep(Duration::from_millis(100));
    let found = discover(port, Duration::from_millis(800));
    assert!(
        found
            .iter()
            .any(|f| f.info.table == "Kitchen table" && f.info.port == port),
        "found {found:?}"
    );
}

#[test]
fn garbage_and_huge_lines_are_rejected() {
    let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
    let port = server.port.unwrap();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.write_all(b"this is not json\n").unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut buf = [0u8; 16];
    // The server hangs up.
    let n = s.read(&mut buf).unwrap_or(0);
    assert_eq!(n, 0);
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let big = vec![b'x'; (MAX_CLIENT_LINE + 10) as usize];
    let _ = s.write_all(&big);
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let n = s.read(&mut buf).unwrap_or(0);
    assert_eq!(n, 0);
}

#[test]
fn unknown_messages_are_answered_not_fatal() {
    let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
    let port = server.port.unwrap();
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut w = s.try_clone().unwrap();
    let mut r = BufReader::new(s);
    let mut line = String::new();
    let mut send = |text: &str| {
        w.write_all(text.as_bytes()).unwrap();
        w.write_all(b"\n").unwrap();
    };
    send(&serde_json::to_string(&ClientMsg::hello("Future", None)).unwrap());
    r.read_line(&mut line).unwrap();
    assert!(line.contains("Welcome"), "{}", line);
    // A request from some future version of holdem.
    send(r#"{"Teleport":{"to":"Mars"}}"#);
    let mut got_notice = false;
    for _ in 0..5 {
        line.clear();
        if r.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if line.contains("Notice") {
            got_notice = true;
            break;
        }
    }
    assert!(got_notice, "the table explains instead of hanging up");
    // The connection still works.
    send(r#""Ready""#);
    line.clear();
    assert!(r.read_line(&mut line).unwrap() > 0);
}

#[test]
fn a_greeting_the_table_cannot_read_gets_an_explanation() {
    let server = Server::start(TableSettings::default(), Some(0), 1).expect("starts");
    let port = server.port.unwrap();
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.write_all(b"{\"Greetings\":{\"protocol\":99}}\n").unwrap();
    let mut r = BufReader::new(s);
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    assert!(
        line.contains("Goodbye") && line.contains("version"),
        "{}",
        line
    );
}

#[test]
fn local_connections_leave_cleanly() {
    let mut server = Server::start(TableSettings::default(), None, 1).expect("starts");
    assert_eq!(server.port, None);
    let c = server.connect_local(true);
    hello(&c, "Solo");
    c.send(ClientMsg::Ready);
    let v = wait_for(&c, |v| v.hand_no == 1);
    assert_eq!(v.you, Some(0));
    drop(c);
    server.stop();
    assert!(!server.is_running());
}

#[test]
fn addresses_get_the_default_port() {
    let server = Server::start(TableSettings::default(), Some(0), 1).unwrap();
    let port = server.port.unwrap();
    assert!(connect(&format!("127.0.0.1:{port}"), 1, Duration::from_secs(2)).is_ok());
    assert!(connect("127.0.0.1", port, Duration::from_secs(2)).is_ok());
    assert!(connect("localhost", port, Duration::from_secs(2)).is_ok());
}

/// Two tables behind one port.
fn two_tables() -> Server {
    let table = |name: &str| TableSettings {
        name: name.to_string(),
        speed_ms: 20,
        ..TableSettings::default()
    };
    let (server, _log) =
        Server::start_tables(vec![table("Low"), table("High")], Some(0), 1).expect("starts");
    server
}

/// The next message that is not a table or lobby update.
fn next_reply(c: &Connection) -> ServerMsg {
    loop {
        match c.incoming.recv_timeout(Duration::from_secs(5)) {
            Ok(ServerMsg::Lobby(_) | ServerMsg::Table(_)) => {}
            Ok(m) => return m,
            Err(e) => panic!("no answer from the server: {e}"),
        }
    }
}

#[test]
fn a_greeting_naming_a_table_goes_to_that_table() {
    let server = two_tables();
    let c = connect("127.0.0.1", server.port.unwrap(), Duration::from_secs(5)).unwrap();
    c.send(ClientMsg::hello("Ann", Some("High")));
    assert!(matches!(next_reply(&c), ServerMsg::Welcome { table, .. } if table == "High"));
}

#[test]
fn without_a_table_the_client_chooses_from_the_list() {
    let server = two_tables();
    let c = connect("127.0.0.1", server.port.unwrap(), Duration::from_secs(5)).unwrap();
    c.send(ClientMsg::hello("Ann", None));
    let ServerMsg::Tables(tables) = next_reply(&c) else {
        panic!("expected the list of tables");
    };
    let names: Vec<&str> = tables.iter().map(|t| t.table.as_str()).collect();
    assert_eq!(names, ["Low", "High"]);
    // A name that is not there: the list again.
    c.send(ClientMsg::ChooseTable("Middle".to_string()));
    assert!(matches!(next_reply(&c), ServerMsg::Notice(_)));
    assert!(matches!(next_reply(&c), ServerMsg::Tables(_)));
    c.send(ClientMsg::ChooseTable("Low".to_string()));
    assert!(matches!(next_reply(&c), ServerMsg::Welcome { table, .. } if table == "Low"));
}

#[test]
fn older_programs_land_at_the_first_table() {
    let server = two_tables();
    let c = connect("127.0.0.1", server.port.unwrap(), Duration::from_secs(5)).unwrap();
    // What holdem 0.4 to 0.7 send: protocol 3, no table.
    c.send(ClientMsg::Hello {
        name: "Old".to_string(),
        version: 3,
        min_version: Some(3),
        app: "0.7.0".to_string(),
        table: None,
    });
    assert!(
        matches!(next_reply(&c), ServerMsg::Welcome { table, version: 3, .. } if table == "Low")
    );
}

#[test]
fn names_are_unique_per_table_not_per_server() {
    let server = two_tables();
    let port = server.port.unwrap();
    let a = connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
    let b = connect("127.0.0.1", port, Duration::from_secs(5)).unwrap();
    a.send(ClientMsg::hello("Ann", Some("Low")));
    b.send(ClientMsg::hello("Ann", Some("High")));
    for c in [&a, &b] {
        assert!(matches!(next_reply(c), ServerMsg::Welcome { name, .. } if name == "Ann"));
    }
}

#[test]
fn discovery_finds_every_table_of_a_server() {
    let server = two_tables();
    let port = server.port.unwrap();
    let found = discover(port, Duration::from_millis(500));
    let mut names: Vec<&str> = found
        .iter()
        .filter(|f| f.info.port == port)
        .map(|f| f.info.table.as_str())
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(names, ["High", "Low"]);
}
