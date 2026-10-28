//! Rules tests for the game engine: scripted scenarios, a side-pot oracle, and
//! random-play fuzzing that checks the invariants after every single action.

use holdem::bot;
use holdem::card::{Deck, Rng, parse_cards};
use holdem::eval::Score;
use holdem::game::{Action, Game, GameEvent, HUMAN, Outcome, Rules, State, Street};

fn game(n: usize, stacks: &[u32], button: usize, sb: u32, hpl: u32) -> Game {
    game_with(n, stacks, button, sb, hpl, Rules::default())
}

fn game_with(n: usize, stacks: &[u32], button: usize, sb: u32, hpl: u32, rules: Rules) -> Game {
    let rules = Rules {
        starting_chips: 1000,
        small_blind: sb,
        hands_per_level: hpl,
        ..rules
    };
    let mut g = Game::new(rules, n, Rng::seeded(1));
    g.setup(stacks, button);
    g
}

fn stacked(cards: &str) -> Deck {
    Deck::stacked(&parse_cards(cards).expect("valid cards"))
}

fn chips(g: &Game) -> Vec<u32> {
    g.players.iter().map(|p| p.chips).collect()
}

// ------------------------------------------------------------------ order of play

#[test]
fn heads_up_button_is_small_blind_and_acts_first_preflop() {
    let mut g = game(2, &[1000, 1000], 0, 10, 0);
    g.start_hand();
    assert_eq!((g.sb_seat, g.bb_seat), (0, 1));
    assert_eq!(g.to_act, 0);
    assert_eq!((g.players[0].bet, g.players[1].bet), (10, 20));
    g.act(Action::Call).unwrap();
    // The big blind has the option to raise after a limp.
    assert_eq!(g.to_act, 1);
    assert!(g.legal(1).can_raise);
    g.act(Action::Check).unwrap();
    assert_eq!(g.street, Street::Flop);
    assert_eq!(g.board.len(), 3);
    // Post-flop the big blind (non-button) acts first heads-up.
    assert_eq!(g.to_act, 1);
    g.act(Action::Check).unwrap();
    assert_eq!(g.to_act, 0);
}

#[test]
fn three_handed_order_of_play() {
    let mut g = game(3, &[1000; 3], 0, 10, 0);
    g.start_hand();
    assert_eq!((g.sb_seat, g.bb_seat, g.to_act), (1, 2, 0));
    g.act(Action::Call).unwrap();
    g.act(Action::Call).unwrap();
    g.act(Action::Check).unwrap();
    assert_eq!(g.street, Street::Flop);
    // Post-flop the first live player left of the button acts first.
    assert_eq!(g.to_act, 1);
}

#[test]
fn folding_to_the_big_blind_awards_the_small_blind() {
    let mut g = game(3, &[1000; 3], 0, 10, 0);
    g.start_hand();
    g.act(Action::Fold).unwrap();
    g.act(Action::Fold).unwrap();
    assert_eq!(g.state, State::HandOver);
    assert_eq!(chips(&g), vec![1000, 990, 1010]);
    assert!(!g.showdown);
    assert_eq!(g.result_lines.len(), 1);
}

#[test]
fn blind_levels_increase_every_n_hands() {
    let mut g = game(3, &[1000; 3], 0, 10, 2);
    let mut seen = Vec::new();
    for _ in 0..6 {
        g.start_hand();
        seen.push((g.sb, g.bb, g.level));
        while g.state == State::Betting {
            g.act(Action::Fold).unwrap();
        }
        assert_eq!(g.state, State::HandOver);
    }
    assert_eq!(
        seen,
        vec![
            (10, 20, 1),
            (10, 20, 1),
            (15, 30, 2),
            (15, 30, 2),
            (25, 50, 3),
            (25, 50, 3)
        ]
    );
}

#[test]
fn the_button_moves_one_seat_each_hand() {
    let mut g = game(4, &[1000; 4], 2, 10, 0);
    let mut buttons = Vec::new();
    for _ in 0..5 {
        g.start_hand();
        buttons.push(g.button);
        while g.state == State::Betting {
            g.act(Action::Fold).unwrap();
        }
    }
    assert_eq!(buttons, vec![2, 3, 0, 1, 2]);
}

// ------------------------------------------------------------------ raise rules

#[test]
fn illegal_actions_are_rejected_without_changing_the_game() {
    let mut g = game(3, &[1000; 3], 0, 10, 0);
    g.start_hand();
    let before = (g.to_act, g.pot(), g.current_bet);
    assert!(
        g.act(Action::Check).is_err(),
        "cannot check facing the big blind"
    );
    assert!(
        g.act(Action::RaiseTo(30)).is_err(),
        "below the minimum raise of 40"
    );
    assert!(g.act(Action::RaiseTo(5000)).is_err(), "more than the stack");
    assert_eq!((g.to_act, g.pot(), g.current_bet), before);
    let l = g.legal(0);
    assert_eq!((l.to_call, l.min_to, l.max_to), (20, 40, 1000));
    assert!(g.act(Action::RaiseTo(40)).is_ok());
    assert_eq!(g.min_raise, 20);
}

#[test]
fn a_short_all_in_does_not_reopen_the_betting() {
    // Seat 2 is the big blind with only 130 chips.
    let mut g = game(3, &[1000, 1000, 130], 0, 10, 0);
    g.start_hand();
    assert_eq!(g.legal(0).min_to, 40);
    g.act(Action::RaiseTo(100)).unwrap();
    assert_eq!(g.min_raise, 80);
    g.act(Action::Call).unwrap(); // small blind completes to 100
    let l = g.legal(2);
    assert!(l.can_raise);
    assert_eq!(l.min_to, 130);
    g.act(Action::AllIn).unwrap(); // raises by only 30
    assert_eq!(g.current_bet, 130);
    assert_eq!(
        g.min_raise, 80,
        "a short all-in leaves the minimum raise alone"
    );
    assert_eq!(g.to_act, 0);
    let l0 = g.legal(0);
    assert!(
        !l0.can_raise,
        "seat 0 already acted and faces less than a full raise"
    );
    assert_eq!(l0.to_call, 30);
    g.act(Action::Call).unwrap();
    assert_eq!(g.to_act, 1);
    assert!(!g.legal(1).can_raise);
    g.act(Action::Call).unwrap();
    assert_eq!(g.street, Street::Flop);
}

#[test]
fn short_all_ins_that_add_up_to_a_full_raise_reopen_the_betting() {
    // Seats: button 0, small blind 1 (150 chips), big blind 2 (190 chips), under the gun 3.
    let mut g = game(4, &[1000, 150, 190, 1000], 0, 10, 0);
    g.start_hand();
    assert_eq!(g.to_act, 3);
    g.act(Action::RaiseTo(100)).unwrap();
    assert_eq!(g.min_raise, 80);
    g.act(Action::Call).unwrap(); // seat 0
    g.act(Action::AllIn).unwrap(); // seat 1 to 150: a raise of only 50
    assert_eq!((g.current_bet, g.min_raise), (150, 80));
    assert_eq!(g.to_act, 2);
    assert!(g.legal(2).can_raise, "the big blind has not acted yet");
    g.act(Action::AllIn).unwrap(); // seat 2 to 190: another 40
    assert_eq!(g.current_bet, 190);
    // Seats 3 and 0 now face 90 more, which is at least a full raise of 80.
    assert_eq!(g.to_act, 3);
    let l3 = g.legal(3);
    assert!(l3.can_raise);
    assert_eq!(l3.to_call, 90);
    g.act(Action::Call).unwrap();
    assert_eq!(g.to_act, 0);
    assert!(g.legal(0).can_raise);
}

#[test]
fn one_short_all_in_alone_does_not_reopen() {
    let mut g = game(4, &[1000, 150, 1000, 1000], 0, 10, 0);
    g.start_hand();
    g.act(Action::RaiseTo(100)).unwrap(); // seat 3
    g.act(Action::Call).unwrap(); // seat 0
    g.act(Action::AllIn).unwrap(); // seat 1 (small blind) to 150, a raise of 50
    g.act(Action::Fold).unwrap(); // seat 2 (big blind)
    assert_eq!(g.to_act, 3);
    let l = g.legal(3);
    assert!(!l.can_raise);
    assert_eq!(l.to_call, 50);
}

#[test]
fn a_player_facing_an_all_in_for_less_than_a_call_can_still_only_call_or_fold() {
    let mut g = game(2, &[500, 500], 0, 10, 0);
    g.start_hand();
    g.act(Action::AllIn).unwrap(); // button raises to 500
    let l = g.legal(1);
    assert!(!l.can_raise);
    assert!(!l.can_check);
    assert_eq!(l.to_call, 480);
    assert_eq!(l.stack, 480);
    assert!(g.act(Action::RaiseTo(500)).is_err());
    assert!(g.act(Action::Call).is_ok());
    assert_eq!(g.state, State::Runout);
}

// ------------------------------------------------------------------ showdowns and pots

#[test]
fn all_in_runout_takes_four_steps_and_pays_the_winner() {
    // Heads-up, seat 1 is dealt first: K♣ A♠ K♦ A♥. Aces beat kings.
    let mut g = game(2, &[500, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked("Kc As Kd Ah 3d 2c 7d 9h 3h Js 3s 4c"));
    g.act(Action::AllIn).unwrap();
    g.act(Action::Call).unwrap();
    assert_eq!(g.state, State::Runout);
    assert!(g.reveal_all);
    for expected in [Street::Flop, Street::Turn, Street::River] {
        g.runout_step();
        assert_eq!(g.street, expected);
        assert_eq!(g.state, State::Runout);
    }
    g.runout_step();
    assert_eq!(g.state, State::HandOver);
    assert!(g.showdown);
    assert_eq!(chips(&g), vec![1000, 0]);
    assert_eq!(g.place_of(1), Some(2));
    assert!(g.players[1].out);
    assert!(g.tournament_over());
    g.start_hand();
    assert_eq!(g.state, State::GameOver);
    let standings = g.standings();
    assert_eq!(standings[0].name, "You");
    assert_eq!(standings[0].place, Some(1));
    assert_eq!(standings[1].place, Some(2));
}

#[test]
fn a_bet_without_a_pair_that_takes_the_pot_is_a_bluff() {
    // Seat 1: 7♣ 2♦, seat 0: K♠ Q♠; the flop 9♥ 5♣ 3♦ gives nobody a pair.
    let mut g = game(2, &[1000, 1000], 0, 10, 0);
    g.start_hand_with_deck(stacked("7c Ks 2d Qs 4h 9h 5c 3d 8s Jc 6s Td"));
    g.act(Action::Call).unwrap(); // seat 0, the button, limps
    g.act(Action::Check).unwrap(); // seat 1
    assert_eq!(g.street, Street::Flop);
    assert_eq!(g.to_act, 1);
    g.act(Action::RaiseTo(40)).unwrap();
    g.act(Action::Fold).unwrap();
    assert_eq!(g.state, State::HandOver);
    let bluff = g.players[1].last_result.clone().unwrap();
    assert_eq!(bluff.outcome, Outcome::WonUncontested);
    assert!(bluff.bluffed);
    assert_eq!(bluff.net, 20);
    assert_eq!(bluff.made, None, "no showdown, no hand named");
    let folded = g.players[0].last_result.clone().unwrap();
    assert_eq!(folded.outcome, Outcome::Folded(Street::Flop));
    assert_eq!(folded.net, -20);
    assert!(!folded.bluffed);
}

#[test]
fn the_result_names_the_kicker_when_it_decides() {
    // Both have a pair of kings on K♠ 7♦ 2♥ 9♣ 4♠: seat 0's ace beats seat 1's queen.
    let mut g = game(2, &[500, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked("Kc Kd Qd Ah 3d Ks 7d 2h 3h 9c 3s 4s"));
    g.act(Action::AllIn).unwrap();
    g.act(Action::Call).unwrap();
    while g.state == State::Runout {
        g.runout_step();
    }
    assert_eq!(chips(&g), vec![1000, 0]);
    assert!(
        g.result_lines[0].ends_with("with Pair of Kings, Ace kicker"),
        "{}",
        g.result_lines[0]
    );

    let won = g.players[0].last_result.clone().unwrap();
    assert_eq!(won.outcome, Outcome::WonShowdown);
    assert_eq!(won.net, 500);
    assert_eq!(won.made.as_deref(), Some("Pair of Kings"));
    assert_eq!(won.against, vec![g.players[1].name.clone()]);
    let lost = g.players[1].last_result.clone().unwrap();
    assert_eq!((lost.outcome, lost.net), (Outcome::LostShowdown, -500));
    assert_eq!(lost.hole, parse_cards("Kc Qd").unwrap());

    // Aces against kings: the pairs decide, no kicker is named.
    let mut g = game(2, &[500, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked("Kc As Kd Ah 3d 2c 7d 9h 3h Js 3s 4c"));
    g.act(Action::AllIn).unwrap();
    g.act(Action::Call).unwrap();
    while g.state == State::Runout {
        g.runout_step();
    }
    assert!(
        g.result_lines[0].ends_with("with Pair of Aces"),
        "{}",
        g.result_lines[0]
    );
}

/// Seat 0: A♦ A♥, seat 1: K♣ K♥, seat 2: Q♣ J♦, board A♣ K♦ 9♥ 5♠ 2♣ (button 0, three players).
const SIDE_POT_DECK: &str = "Kc Qc Ad Kh Jd Ah 3d Ac Kd 9h 4d 5s 6d 2c";

#[test]
fn an_overbet_above_the_side_pots_is_returned() {
    let mut g = game(3, &[100, 300, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked(SIDE_POT_DECK));
    g.act(Action::AllIn).unwrap(); // seat 0 to 100
    g.act(Action::Call).unwrap(); // seat 1 calls 100 and still has 200 behind
    assert!(g.legal(2).can_raise);
    g.act(Action::AllIn).unwrap(); // seat 2 raises to 500
    g.act(Action::Call).unwrap(); // seat 1 calls all-in for 300 in total
    assert_eq!(g.state, State::Runout);
    while g.state == State::Runout {
        g.runout_step();
    }
    // Seat 2's last 200 was never matched and comes back before the pots are paid.
    assert_eq!(g.refund, Some((2, 200)));
    assert_eq!(chips(&g), vec![300, 400, 200]);
    assert_eq!(g.awards.len(), 2);
    assert_eq!(g.awards[0].amount, 300);
    assert_eq!(g.awards[1].amount, 400);
}

#[test]
fn side_pots_are_paid_to_the_right_players() {
    // Stacks 100 / 300 / 500, button 0. Deal order is seat 1, 2, 0, 1, 2, 0.
    // Seat 0: A♦ A♥, seat 1: K♣ K♥, seat 2: Q♣ J♦, board A♣ K♦ 9♥ 5♠ 2♣.
    // Seat 0 has three aces, seat 1 three kings, seat 2 nothing.
    let mut g = game(3, &[100, 300, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked(SIDE_POT_DECK));
    assert_eq!(g.to_act, 0);
    g.act(Action::AllIn).unwrap(); // seat 0 to 100
    g.act(Action::AllIn).unwrap(); // seat 1 (small blind) to 300
    // Both opponents are all-in, so nobody could answer a raise: call or fold only.
    assert_eq!(g.to_act, 2);
    let l = g.legal(2);
    assert!(!l.can_raise);
    assert_eq!(l.to_call, 280);
    assert!(
        g.act(Action::AllIn).is_err(),
        "pushing 480 would only be a call of 280"
    );
    g.act(Action::Call).unwrap();
    assert_eq!(g.state, State::Runout);
    while g.state == State::Runout {
        g.runout_step();
    }
    assert_eq!(g.state, State::HandOver);
    // Main pot 3 x 100 -> seat 0, side pot 2 x 200 -> seat 1; seat 2 keeps the 200 it never bet.
    assert_eq!(chips(&g), vec![300, 400, 200]);
    assert_eq!(g.refund, None);
    assert_eq!(g.awards.len(), 2);
    assert_eq!(g.awards[0].winners, vec![0]);
    assert_eq!(g.awards[1].winners, vec![1]);
    assert_eq!(g.players[0].payout, 300);
    assert_eq!(g.players[1].payout, 400);
    assert!(
        !g.players[2].out,
        "seat 2 got 200 back and is still in the game"
    );
    assert_eq!(g.players[2].eliminated, None);
}

#[test]
fn a_tied_showdown_splits_the_pot() {
    // Both players hold A-K and the board plays no better: 2♣ 7♦ 9♥ J♠ 4♣.
    let mut g = game(2, &[1000, 1000], 0, 10, 0);
    g.start_hand_with_deck(stacked("Ah As Kc Kd 3d 2c 7d 9h 3h Js 3s 4c"));
    g.act(Action::Call).unwrap();
    g.act(Action::Check).unwrap();
    for _ in 0..3 {
        g.act(Action::Check).unwrap();
        g.act(Action::Check).unwrap();
    }
    assert_eq!(g.state, State::HandOver);
    assert!(g.showdown);
    assert_eq!(chips(&g), vec![1000, 1000]);
    assert_eq!(g.awards.len(), 1);
    assert_eq!(g.awards[0].winners.len(), 2);
    assert_eq!(g.awards[0].shares, vec![20, 20]);
}

#[test]
fn the_uncalled_part_of_a_bet_is_returned() {
    let mut g = game(2, &[1000, 1000], 0, 10, 0);
    g.start_hand();
    g.act(Action::RaiseTo(300)).unwrap();
    g.act(Action::Fold).unwrap();
    assert_eq!(g.state, State::HandOver);
    // Seat 0 gets back the 280 the big blind could not match and wins the 40 pot:
    // 1000 - 300 + 280 + 40 = 1020, while the big blind is down its 20.
    assert_eq!(chips(&g), vec![1020, 980]);
    assert_eq!(g.refund, Some((0, 280)));
}

// ------------------------------------------------------------------ events

#[test]
fn events_describe_the_hand_in_order() {
    // Heads-up, seat 1 gets K♣ K♦, seat 0 gets A♠ A♥: seat 1 busts.
    let mut g = game(2, &[500, 500], 0, 10, 0);
    g.start_hand_with_deck(stacked("Kc As Kd Ah 3d 2c 7d 9h 3h Js 3s 4c"));
    assert_eq!(g.take_events(), vec![GameEvent::HandStarted]);
    assert!(
        g.take_events().is_empty(),
        "events are handed out only once"
    );
    let before = g.action_count;
    g.act(Action::RaiseTo(60)).unwrap();
    assert_eq!(g.take_events(), vec![GameEvent::Bet(0)]);
    g.act(Action::AllIn).unwrap();
    g.act(Action::Call).unwrap();
    assert_eq!(g.action_count, before + 3);
    assert_eq!(
        g.take_events(),
        vec![GameEvent::AllIn(1), GameEvent::AllIn(0)]
    );
    while g.state == State::Runout {
        g.runout_step();
    }
    assert_eq!(
        g.take_events(),
        vec![
            GameEvent::Street(Street::Flop),
            GameEvent::Street(Street::Turn),
            GameEvent::Street(Street::River),
            GameEvent::HandSettled,
            GameEvent::Eliminated(1),
        ]
    );
    // A rejected action records nothing.
    g.start_hand();
    assert_eq!(g.take_events(), vec![GameEvent::TournamentOver]);
    assert!(g.act(Action::Check).is_err());
    assert!(g.take_events().is_empty());
}

#[test]
fn check_call_and_fold_events() {
    let mut g = game(3, &[1000; 3], 0, 10, 0);
    g.start_hand();
    g.take_events();
    g.act(Action::Call).unwrap(); // seat 0
    g.act(Action::Fold).unwrap(); // seat 1
    g.act(Action::Check).unwrap(); // seat 2, the big blind: the flop comes
    assert_eq!(
        g.take_events(),
        vec![
            GameEvent::Call(0),
            GameEvent::Fold(1),
            GameEvent::Check(2),
            GameEvent::Street(Street::Flop),
        ]
    );
}

// ------------------------------------------------------------------ oracle and fuzzing

/// Independent side-pot settlement: peel the pot off layer by layer.
fn oracle(totals: &[u32], live: &[bool], scores: &[Option<Score>], button: usize) -> Vec<u32> {
    let n = totals.len();
    let mut c = totals.to_vec();
    let mut pay = vec![0u32; n];
    while c.iter().any(|&x| x > 0) {
        let elig: Vec<usize> = (0..n).filter(|&i| live[i] && c[i] > 0).collect();
        if elig.is_empty() {
            break;
        }
        let m = elig.iter().map(|&i| c[i]).min().unwrap();
        let pot: u32 = c.iter().map(|&x| x.min(m)).sum();
        for x in &mut c {
            *x -= (*x).min(m);
        }
        let best = elig.iter().filter_map(|&i| scores[i]).max();
        let mut w: Vec<usize> = match best {
            Some(b) => elig
                .iter()
                .copied()
                .filter(|&i| scores[i] == Some(b))
                .collect(),
            None => elig.clone(),
        };
        let first = (button + 1) % n;
        w.sort_by_key(|&i| (i + n - first) % n);
        let k = w.len() as u32;
        for (j, &i) in w.iter().enumerate() {
            pay[i] += pot / k + if (j as u32) < pot % k { 1 } else { 0 };
        }
    }
    // Chips nobody can contest go back to their owner.
    for i in 0..n {
        pay[i] += c[i];
    }
    pay
}

fn random_action(g: &Game, rng: &mut Rng) -> Action {
    let seat = g.to_act;
    let l = g.legal(seat);
    let r = rng.unit();
    let raise = |rng: &mut Rng| {
        if rng.unit() < 0.8 {
            Action::RaiseTo(rng.range(l.min_to, l.max_to))
        } else {
            Action::AllIn
        }
    };
    if l.can_check {
        if l.can_raise && r < 0.35 {
            raise(rng)
        } else if r < 0.02 {
            Action::Fold
        } else {
            Action::Check
        }
    } else if r < 0.2 {
        Action::Fold
    } else if l.can_raise && r < 0.45 {
        raise(rng)
    } else {
        Action::Call
    }
}

fn passive_action(g: &Game, rng: &mut Rng) -> Action {
    let l = g.legal(g.to_act);
    if l.can_check {
        Action::Check
    } else if rng.unit() < 0.15 {
        Action::Fold
    } else {
        Action::Call
    }
}

fn check_settlement_against_oracle(g: &Game) {
    let totals = &g.last_totals;
    let live: Vec<bool> = g.players.iter().map(holdem::game::Player::live).collect();
    let scores: Vec<Option<Score>> = g.players.iter().map(|p| p.score).collect();
    let expected = oracle(totals, &live, &scores, g.button);
    for (i, p) in g.players.iter().enumerate() {
        // final = start - contributed + payout
        let got = i64::from(p.chips) - i64::from(p.start_chips) + i64::from(totals[i]);
        assert_eq!(
            got,
            i64::from(expected[i]),
            "seat {i} payout mismatch: totals {totals:?}, live {live:?}, expected {expected:?}"
        );
    }
}

/// Random table rules for a fuzz run, so every combination gets exercised.
fn fuzz_rules(seed: u64, stack: u32, sb: u32, hpl: u32) -> Rules {
    Rules {
        starting_chips: stack,
        small_blind: sb,
        hands_per_level: hpl,
        hand_limit: if seed % 7 == 3 { 40 } else { 0 },
        antes: seed % 2 == 1,
        straddle: seed.is_multiple_of(3),
        rebuy_levels: if seed % 3 == 1 { 2 } else { 0 },
        dead_button: seed % 4 == 1,
        muck_losers: seed % 5 == 2,
        // The configurable details vary too.
        blind_schedule: if seed % 6 == 5 {
            vec![100, 300]
        } else {
            holdem::game::DEFAULT_BLIND_SCHEDULE.to_vec()
        },
        blind_growth_percent: if seed % 6 == 5 { 200 } else { 150 },
        ante_divisor: [8, 4, 10][(seed % 3) as usize],
        straddle_multiplier: if seed.is_multiple_of(4) { 3 } else { 2 },
        straddle_min_players: if seed.is_multiple_of(5) { 3 } else { 4 },
        rebuy_chips: if seed % 4 == 3 { stack / 2 } else { 0 },
    }
}

/// Between hands: random rebuys, players leaving and new players joining.
fn shuffle_seats(g: &mut Game, rng: &mut Rng) {
    for i in 0..g.players.len() {
        if g.players[i].rebuy_pending && rng.chance(0.5) {
            g.rebuy(i).unwrap();
        }
    }
    let n = g.players.len();
    let seat = rng.below(n as u64) as usize;
    if seat != HUMAN && !g.players[seat].out && g.alive_count() > 2 && rng.chance(0.05) {
        g.remove_player(seat).unwrap();
    }
    let seat = rng.below(n as u64) as usize;
    if g.players[seat].out && !g.players[seat].rebuy_pending && rng.chance(0.1) {
        let taken: Vec<String> = g.players.iter().map(|p| p.name.clone()).collect();
        let names: Vec<String> = bot::DEFAULT_NAMES
            .iter()
            .map(std::string::ToString::to_string)
            .collect();
        let (name, style) = bot::newcomer(&names, &taken, rng);
        let chips = g.rules.starting_chips;
        g.seat_player(seat, &name, false, style, chips).unwrap();
    }
}

fn play_fuzz_game(seed: u64, players: usize, stack: u32, sb: u32, hpl: u32, passive: bool) -> u32 {
    let mut g = Game::new(fuzz_rules(seed, stack, sb, hpl), players, Rng::seeded(seed));
    let mut rng = Rng::seeded(seed ^ 0xABCDEF);
    let mut hands = 0;
    while hands < 400 {
        if hands > 0 {
            shuffle_seats(&mut g, &mut rng);
            assert_eq!(g.chips_in_play(), g.total_chips());
        }
        g.start_hand();
        if g.state == State::GameOver {
            break;
        }
        hands += 1;
        assert_eq!(g.chips_in_play(), g.total_chips());
        let mut steps = 0;
        while matches!(g.state, State::Betting | State::Runout) {
            steps += 1;
            assert!(steps < 500, "the hand did not terminate");
            if g.state == State::Runout {
                g.runout_step();
            } else {
                assert!(g.players[g.to_act].can_act());
                let action = if passive {
                    passive_action(&g, &mut rng)
                } else {
                    random_action(&g, &mut rng)
                };
                g.act(action)
                    .unwrap_or_else(|e| panic!("{action:?} was rejected: {e}"));
            }
            assert_eq!(
                g.chips_in_play(),
                g.total_chips(),
                "chips must be conserved"
            );
            for p in &g.players {
                assert!(p.bet <= p.total);
            }
        }
        assert_eq!(g.state, State::HandOver);
        assert_eq!(g.pot(), 0);
        assert_eq!(g.chips_in_play(), g.total_chips());
        check_settlement_against_oracle(&g);
        for p in &g.players {
            if g.showdown && p.live() && !g.rules.muck_losers {
                assert!(p.shown, "without mucking everyone shows");
            }
        }
    }
    if g.state == State::GameOver && g.contenders() == 1 {
        let w = g.players.iter().find(|p| p.contending()).unwrap();
        assert_eq!(u64::from(w.chips), g.total_chips());
    }
    // Places are 1, 2, 3, ... with no gaps or repeats, and only players who left have none.
    let standings = g.standings();
    let places: Vec<usize> = standings.iter().filter_map(|s| s.place).collect();
    assert_eq!(places, (1..=places.len()).collect::<Vec<usize>>());
    hands
}

#[test]
fn random_play_conserves_chips_and_matches_the_pot_oracle() {
    let mut total_hands = 0;
    for seed in 1..=250u64 {
        let players = 2 + (seed % 8) as usize;
        let stack = [300, 1000, 5000][(seed % 3) as usize];
        let sb = [2, 5, 10][(seed % 3) as usize];
        let hpl = [0, 5, 10][((seed / 3) % 3) as usize];
        total_hands += play_fuzz_game(seed, players, stack, sb, hpl, seed % 2 == 0);
    }
    assert!(total_hands > 500, "only {total_hands} hands were played");
}

#[test]
fn bots_can_play_whole_tournaments() {
    for seed in 1..=40u64 {
        let rules = Rules {
            starting_chips: 500,
            small_blind: 10,
            hands_per_level: 6,
            ..fuzz_rules(seed, 500, 10, 6)
        };
        let mut g = Game::new(rules, 2 + (seed % 8) as usize, Rng::seeded(seed * 7919));
        let mut hands = 0;
        loop {
            g.start_hand();
            if g.state == State::GameOver {
                break;
            }
            hands += 1;
            assert!(hands < 3000, "tournament {seed} never ended");
            let mut steps = 0;
            while matches!(g.state, State::Betting | State::Runout) {
                steps += 1;
                assert!(steps < 500);
                if g.state == State::Runout {
                    g.runout_step();
                } else {
                    // The bot logic also plays the human seat here.
                    g.bot_move();
                }
                assert_eq!(g.chips_in_play(), g.total_chips());
            }
            check_settlement_against_oracle(&g);
        }
        assert!(g.tournament_over());
        assert_eq!(g.standings()[0].place, Some(1));
    }
}
