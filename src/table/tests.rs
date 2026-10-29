use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use super::*;
use crate::card::Rng;
use crate::game::{Action, State};
use crate::player::symbol::Symbol;
use crate::protocol::{ClientMsg, MIN_PROTOCOL_VERSION, PROTOCOL_VERSION, ServerMsg, TableView};

struct Peer {
    id: ClientId,
    rx: Receiver<ServerMsg>,
    view: Option<TableView>,
}

impl Peer {
    /// Reads everything sent so far and keeps the newest table view.
    fn read(&mut self) -> Vec<ServerMsg> {
        let msgs: Vec<ServerMsg> = self.rx.try_iter().collect();
        for m in &msgs {
            if let ServerMsg::Table(v) = m {
                self.view = Some((**v).clone());
            }
        }
        msgs
    }
}

fn join(t: &mut Table, id: ClientId, name: &str, owner: bool, now: Instant) -> Peer {
    let (tx, rx) = channel();
    t.connect(id, tx, owner);
    t.handle(id, ClientMsg::hello(name, None, None), now);
    Peer { id, rx, view: None }
}

/// Runs the clock forward in small steps, letting `play` answer for people.
fn run(t: &mut Table, now: &mut Instant, peers: &mut [Peer], steps: usize, play: bool) {
    for _ in 0..steps {
        *now += Duration::from_millis(100);
        t.tick(*now);
        t.flush(*now);
        for p in peers.iter_mut() {
            p.read();
            let Some(v) = p.view.clone() else { continue };
            if play && let Some(l) = v.legal {
                // Cautious: check when free, call only small bets.
                let a = if l.can_check {
                    Action::Check
                } else if l.to_call <= l.stack / 20 {
                    Action::Call
                } else {
                    Action::Fold
                };
                t.handle(
                    p.id,
                    ClientMsg::Act {
                        action: a,
                        turn: v.turn,
                    },
                    *now,
                );
            }
            if play && v.state == State::HandOver && !v.you_ready && v.you.is_some() {
                t.handle(p.id, ClientMsg::Ready, *now);
            }
        }
    }
}

fn settings() -> TableSettings {
    TableSettings {
        speed_ms: 100,
        seats: 4,
        ..TableSettings::default()
    }
}

#[test]
fn single_player_game_starts_and_plays_hands() {
    let mut now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(1));
    let mut me = join(&mut t, 1, "Me", true, now);
    let msgs = me.read();
    assert!(matches!(msgs[0], ServerMsg::Welcome { .. }));
    t.handle(1, ClientMsg::Ready, now);
    t.flush(now);
    me.read();
    let v = me.view.clone().expect("the game started");
    assert_eq!(v.you, Some(0));
    assert_eq!(v.seats.len(), 4);
    assert_eq!(v.seats[0].cards.len(), 2, "you see your own cards");
    for s in &v.seats[1..] {
        assert!(
            s.cards.is_empty() && s.hidden_cards,
            "bots' cards stay hidden"
        );
    }
    let mut peers = [me];
    run(&mut t, &mut now, &mut peers, 600, true);
    let g = t.game().unwrap();
    assert!(g.hand_no >= 3, "only {} hands in a minute", g.hand_no);
    assert_eq!(g.chips_in_play(), g.total_chips());
    assert!(!peers[0].view.as_ref().unwrap().multiplayer);
}

#[test]
fn single_player_waits_for_enter_between_hands() {
    let mut now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(2));
    let me = join(&mut t, 1, "Me", true, now);
    t.handle(1, ClientMsg::Ready, now);
    let mut peers = [me];
    // Play the hand but never say "next hand".
    for _ in 0..300 {
        now += Duration::from_millis(100);
        t.tick(now);
        t.flush(now);
        peers[0].read();
        if let Some(l) = peers[0].view.as_ref().and_then(|v| v.legal) {
            let a = if l.can_check {
                Action::Check
            } else {
                Action::Fold
            };
            t.handle(
                1,
                ClientMsg::Act {
                    action: a,
                    turn: peers[0].view.as_ref().unwrap().turn,
                },
                now,
            );
        }
    }
    let g = t.game().unwrap();
    assert_eq!(g.hand_no, 1);
    assert_eq!(g.state, State::HandOver);
    t.handle(1, ClientMsg::Ready, now);
    t.tick(now);
    assert_eq!(t.game().unwrap().hand_no, 2);
}

#[test]
fn two_people_never_see_each_others_cards() {
    let mut now = Instant::now();
    let s = TableSettings {
        fill_with_bots: false,
        ..settings()
    };
    let mut t = Table::new(s, Rng::seeded(3));
    let a = join(&mut t, 1, "Ann", true, now);
    let b = join(&mut t, 2, "Bob", false, now);
    t.handle(1, ClientMsg::Ready, now);
    assert!(t.game().is_none(), "waits for everyone to be ready");
    t.handle(2, ClientMsg::Ready, now);
    assert_eq!(
        t.game().unwrap().players.iter().filter(|p| !p.out).count(),
        2
    );
    let mut peers = [a, b];
    for _ in 0..200 {
        run(&mut t, &mut now, &mut peers, 1, true);
        for p in &peers {
            let Some(v) = &p.view else { continue };
            for (i, seat) in v.seats.iter().enumerate() {
                if Some(i) != v.you && !seat.cards.is_empty() {
                    // Only cards turned up at a showdown or an all-in may be seen.
                    let g = t.game().unwrap();
                    assert!(g.players[i].shown, "{} saw hidden cards", p.id);
                }
            }
            // The hand result is always your own (it holds your cards).
            if let (Some(r), Some(me)) = (&v.your_result, v.you)
                && v.state == State::HandOver
                && r.hand == v.hand_no
            {
                assert_eq!(
                    r.hole, v.seats[me].cards,
                    "{} got someone else's result",
                    p.id
                );
            }
        }
    }
    assert!(peers[0].view.as_ref().unwrap().multiplayer);
}

#[test]
fn a_dropped_player_folds_and_leaves_and_can_come_back() {
    let mut now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(4));
    let a = join(&mut t, 1, "Ann", true, now);
    let _b = join(&mut t, 2, "Bob", false, now);
    t.handle(1, ClientMsg::Ready, now);
    t.handle(2, ClientMsg::Ready, now);
    let bob_seat = t.clients[&2].seat.unwrap();
    t.disconnect(2);
    let mut peers = [a];
    // Bob is folded for when it is his turn, and leaves before hand 2.
    while t.game().unwrap().hand_no < 2 {
        run(&mut t, &mut now, &mut peers, 1, true);
    }
    let g = t.game().unwrap();
    assert!(g.players[bob_seat].vacant, "Bob left the table");
    assert!(g.history.iter().any(|f| f.name == "Bob" && f.left));
    assert_eq!(g.chips_in_play(), g.total_chips());
    // Bob comes back and is seated again at the next hand.
    let _b2 = join(&mut t, 3, "Bob", false, now);
    while t.game().unwrap().hand_no < 3 {
        run(&mut t, &mut now, &mut peers, 1, true);
    }
    let g = t.game().unwrap();
    assert!(
        g.players
            .iter()
            .any(|p| p.name == "Bob" && p.is_human && !p.out)
    );
    assert_eq!(t.clients[&3].seat, Some(bob_seat));
}

#[test]
fn a_player_who_reconnects_during_the_hand_keeps_the_seat() {
    let now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(10));
    let _a = join(&mut t, 1, "Ann", true, now);
    let _b = join(&mut t, 2, "Bob", false, now);
    t.handle(1, ClientMsg::Ready, now);
    t.handle(2, ClientMsg::Ready, now);
    let seat = t.clients[&2].seat;
    t.disconnect(2);
    let _b2 = join(&mut t, 7, "Bob", false, now);
    assert_eq!(t.clients[&7].seat, seat);
    assert!(!t.clients.contains_key(&2));
}

#[test]
fn people_joining_a_running_game_get_a_seat_next_hand() {
    let mut now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(5));
    let a = join(&mut t, 1, "Ann", true, now);
    t.handle(1, ClientMsg::Ready, now);
    // Table is full of bots (4 seats): one bot makes room.
    let mut c = join(&mut t, 9, "Cy", false, now);
    assert!(c.read().iter().any(|m| matches!(m, ServerMsg::Notice(_))));
    let mut peers = [a, c];
    run(&mut t, &mut now, &mut peers, 400, true);
    let g = t.game().unwrap();
    assert!(g.players.iter().any(|p| p.name == "Cy" && p.is_human));
    assert_eq!(g.players.len(), 4);
}

#[test]
fn an_answer_to_an_earlier_turn_is_ignored() {
    let mut now = Instant::now();
    let s = TableSettings {
        fill_with_bots: false,
        ..settings()
    };
    let mut t = Table::new(s, Rng::seeded(13));
    let mut a = join(&mut t, 1, "Ann", true, now);
    let mut b = join(&mut t, 2, "Bob", false, now);
    t.handle(1, ClientMsg::Ready, now);
    t.handle(2, ClientMsg::Ready, now);
    t.flush(now);
    a.read();
    b.read();
    // Heads-up: whoever acts first calls, and then sends the same call again.
    let (first, first_peer) = if a.view.as_ref().unwrap().legal.is_some() {
        (1, &a)
    } else {
        (2, &b)
    };
    let old_turn = first_peer.view.as_ref().unwrap().turn;
    let call = ClientMsg::Act {
        action: Action::Call,
        turn: old_turn,
    };
    t.handle(first, call.clone(), now);
    let after_one = t.game().unwrap().action_count;
    t.handle(first, call, now);
    assert_eq!(
        t.game().unwrap().action_count,
        after_one,
        "the repeat changes nothing"
    );
    assert_ne!(t.game().unwrap().turn_id(), old_turn);
    now += Duration::from_millis(10);
    t.tick(now);
}

#[test]
fn after_a_person_acts_the_next_bot_waits_a_full_step() {
    let mut now = Instant::now();
    let s = TableSettings {
        speed_ms: 1000,
        ..settings()
    };
    let mut t = Table::new(s, Rng::seeded(21));
    let mut me = join(&mut t, 1, "Me", true, now);
    t.handle(1, ClientMsg::Ready, now);
    let mut checked = 0;
    for _ in 0..2000 {
        now += Duration::from_millis(50);
        t.tick(now);
        t.flush(now);
        me.read();
        let Some(v) = me.view.clone() else { continue };
        if v.state == State::HandOver && !v.you_ready {
            t.handle(1, ClientMsg::Ready, now);
            continue;
        }
        let Some(l) = v.legal else { continue };
        // Think for a while, longer than a step, then act.
        now += Duration::from_secs(5);
        let action = if l.can_check {
            Action::Check
        } else {
            Action::Call
        };
        t.handle(
            1,
            ClientMsg::Act {
                action,
                turn: v.turn,
            },
            now,
        );
        let g = t.game().unwrap();
        if g.state != State::Betting || g.players[g.to_act].is_human {
            continue;
        }
        let before = g.action_count;
        t.tick(now + Duration::from_millis(100));
        assert_eq!(
            t.game().unwrap().action_count,
            before,
            "the bot moved at once"
        );
        t.tick(now + Duration::from_millis(1000));
        assert_ne!(
            t.game().unwrap().action_count,
            before,
            "the bot never moved"
        );
        now += Duration::from_millis(1000);
        checked += 1;
        if checked == 3 {
            return;
        }
    }
    panic!("only {checked} turns were followed by a bot");
}

#[test]
fn duplicate_names_get_a_number() {
    let now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(6));
    let _a = join(&mut t, 1, "Ann", true, now);
    let mut b = join(&mut t, 2, "Ann", false, now);
    let msgs = b.read();
    assert!(
        msgs.iter()
            .any(|m| matches!(m, ServerMsg::Welcome { name, .. } if name == "Ann 2"))
    );
    let mut c = join(&mut t, 3, "ANN", false, now);
    assert!(
        c.read()
            .iter()
            .any(|m| matches!(m, ServerMsg::Welcome { name, .. } if name == "ANN 3"))
    );
}

#[test]
fn wrong_versions_are_turned_away_with_a_reason() {
    let now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(7));
    let hello = |version, min_version| ClientMsg::Hello {
        name: "X".into(),
        version,
        min_version,
        app: "test".into(),
        table: None,
        symbol: None,
    };
    // A client from before protocol 3 (holdem 0.3).
    let (tx, rx) = channel();
    t.connect(1, tx, false);
    t.handle(1, hello(2, None), now);
    match rx.try_recv() {
        Ok(ServerMsg::Goodbye(why)) => assert!(why.contains("too old"), "{}", why),
        other => panic!("expected a goodbye, got {other:?}"),
    }
    // A client that only speaks newer versions.
    let (tx, rx) = channel();
    t.connect(2, tx, false);
    t.handle(
        2,
        hello(PROTOCOL_VERSION + 2, Some(PROTOCOL_VERSION + 1)),
        now,
    );
    match rx.try_recv() {
        Ok(ServerMsg::Goodbye(why)) => assert!(why.contains("older holdem"), "{}", why),
        other => panic!("expected a goodbye, got {other:?}"),
    }
    t.flush(now);
    assert!(t.clients.is_empty());
    // A newer client that still speaks our version is welcome, at our version.
    let (tx, rx) = channel();
    t.connect(3, tx, false);
    t.handle(
        3,
        hello(PROTOCOL_VERSION + 2, Some(MIN_PROTOCOL_VERSION)),
        now,
    );
    match rx.try_recv() {
        Ok(ServerMsg::Welcome { version, .. }) => assert_eq!(version, PROTOCOL_VERSION),
        other => panic!("expected a welcome, got {other:?}"),
    }
}

#[test]
fn the_win_chance_permission_reaches_every_view() {
    let now = Instant::now();
    let s = TableSettings {
        allow_win_chance: false,
        ..settings()
    };
    let mut t = Table::new(s, Rng::seeded(12));
    let mut me = join(&mut t, 1, "Ann", true, now);
    t.flush(now);
    let lobby = me
        .read()
        .into_iter()
        .find_map(|m| match m {
            ServerMsg::Lobby(l) => Some(l),
            _ => None,
        })
        .unwrap();
    assert!(!lobby.win_chance_allowed);
    t.handle(1, ClientMsg::Ready, now);
    t.flush(now);
    me.read();
    assert!(!me.view.unwrap().win_chance_allowed);
}

#[test]
fn fluctuation_changes_the_table_and_keeps_the_chips_right() {
    let mut now = Instant::now();
    let s = TableSettings {
        seats: 6,
        fluctuation: Some(Fluctuation {
            leave_chance: 0.2,
            join_chance: 0.5,
        }),
        ..settings()
    };
    let mut t = Table::new(s, Rng::seeded(8));
    let a = join(&mut t, 1, "Ann", true, now);
    t.handle(1, ClientMsg::Ready, now);
    let mut peers = [a];
    let first: Vec<String> = t
        .game()
        .unwrap()
        .players
        .iter()
        .map(|p| p.name.clone())
        .collect();
    run(&mut t, &mut now, &mut peers, 1500, true);
    let g = t.game().unwrap();
    assert!(g.hand_no >= 8);
    let names: Vec<String> = g.players.iter().map(|p| p.name.clone()).collect();
    assert_ne!(first, names, "somebody left or joined");
    assert!(g.history.iter().any(|f| f.left));
    assert_eq!(g.chips_in_play(), g.total_chips());
}

#[test]
fn the_owner_can_pause_a_single_player_game() {
    let mut now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(9));
    let _a = join(&mut t, 1, "Ann", true, now);
    t.handle(1, ClientMsg::Ready, now);
    t.handle(1, ClientMsg::Pause(true), now);
    let before = t.game().unwrap().action_count;
    for _ in 0..50 {
        now += Duration::from_millis(200);
        t.tick(now);
    }
    assert_eq!(t.game().unwrap().action_count, before);
    t.handle(1, ClientMsg::Pause(false), now);
    t.handle(1, ClientMsg::SetSpeed(20), now);
    assert_eq!(t.settings.speed_ms, 20);
}

#[test]
fn a_pause_does_not_eat_into_the_time_to_act() {
    let mut now = Instant::now();
    let mut s = settings();
    s.turn_seconds = 30;
    let mut t = Table::new(s, Rng::seeded(2));
    let mut host = join(&mut t, 1, "Host", true, now);
    t.handle(1, ClientMsg::Ready, now);
    // Let the bots play until it is the host's turn.
    let deadline = now + Duration::from_secs(600);
    while t
        .game()
        .is_none_or(|g| g.state != State::Betting || g.to_act != 0)
    {
        assert!(now < deadline, "never the host's turn");
        now += Duration::from_millis(100);
        t.tick(now);
        if t.game().is_some_and(|g| g.state == State::HandOver) {
            t.handle(1, ClientMsg::Ready, now);
        }
    }
    let decision = t.game().unwrap().turn_id();
    t.handle(1, ClientMsg::Pause(true), now);
    // A long pause; then a guest arrives, which resumes the game.
    now += Duration::from_secs(120);
    let _guest = join(&mut t, 2, "Guest", false, now);
    t.tick(now);
    assert!(t.paused.is_none(), "a second person ends the pause");
    now += Duration::from_secs(5);
    t.tick(now);
    assert_eq!(
        t.game().unwrap().turn_id(),
        decision,
        "the host still has time to act"
    );
    t.flush(now);
    host.read();
    let left = host.view.unwrap().turn_seconds_left.unwrap();
    assert!((24..=25).contains(&left), "{left} seconds left");
}

#[test]
fn everyone_sees_the_symbol_a_person_chose() {
    let now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(17));
    let star = Symbol { glyph: 4, color: 5 };
    let (tx, rx) = channel();
    t.connect(1, tx, true);
    t.handle(1, ClientMsg::hello("Ann", None, Some(star)), now);
    let mut me = Peer {
        id: 1,
        rx,
        view: None,
    };
    let mut other = join(&mut t, 2, "Bob", false, now);
    t.flush(now);
    let lobby = other
        .read()
        .into_iter()
        .find_map(|m| match m {
            ServerMsg::Lobby(l) => Some(l),
            _ => None,
        })
        .expect("a lobby view");
    let ann = lobby.players.iter().find(|p| p.name == "Ann").unwrap();
    assert_eq!(ann.symbol, Some(star));
    t.handle(1, ClientMsg::Ready, now);
    t.handle(2, ClientMsg::Ready, now);
    t.flush(now);
    me.read();
    other.read();
    let v = other.view.clone().expect("the game started");
    let ann = v.seats.iter().find(|s| s.name == "Ann").unwrap();
    assert_eq!(ann.symbol, Some(star));
    let bob = v.seats.iter().find(|s| s.name == "Bob").unwrap();
    assert_eq!(bob.symbol, None, "no choice: the name's symbol");
    assert!(
        v.seats
            .iter()
            .filter(|s| !s.is_human)
            .all(|s| s.symbol.is_none()),
        "bots have none"
    );
}

#[test]
fn the_table_notes_who_comes_and_goes() {
    let now = Instant::now();
    let mut t = Table::new(settings(), Rng::seeded(3));
    let _ann = join(&mut t, 1, "Ann", false, now);
    let _ben = join(&mut t, 2, "Ben", false, now);
    t.disconnect(2);
    t.handle(1, ClientMsg::Ready, now);
    assert_eq!(
        t.take_notes(),
        [
            TableEvent::Joined {
                name: "Ann".to_string(),
                people: 1
            },
            TableEvent::Joined {
                name: "Ben".to_string(),
                people: 2
            },
            TableEvent::Left {
                name: "Ben".to_string(),
                people: 1
            },
            TableEvent::GameStarted { people: 1, bots: 3 },
        ]
    );
    assert!(t.take_notes().is_empty(), "each note is reported once");
}
