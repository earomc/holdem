//! Scenario tests for the optional table rules (antes, straddle, dead button,
//! mucking, rebuys, hand limit) and for players joining and leaving.

use holdem::bot::Personality;
use holdem::card::{Deck, Rng, parse_cards};
use holdem::game::{Action, Game, GameEvent, Rules, State, Street};

fn table(stacks: &[u32], button: usize, rules: Rules) -> Game {
    let rules = Rules {
        starting_chips: 1000,
        small_blind: 10,
        hands_per_level: 0,
        ..rules
    };
    let mut g = Game::new(rules, stacks.len(), Rng::seeded(1));
    g.setup(stacks, button);
    g
}

fn stacked(cards: &str) -> Deck {
    Deck::stacked(&parse_cards(cards).expect("valid cards"))
}

fn chips(g: &Game) -> Vec<u32> {
    g.players.iter().map(|p| p.chips).collect()
}

fn fold_around(g: &mut Game) {
    while g.state == State::Betting {
        g.act(Action::Fold).unwrap();
    }
}

#[test]
fn antes_are_dead_money_and_do_not_count_as_a_bet() {
    let rules = Rules {
        antes: true,
        ..Rules::default()
    };
    let mut g = table(&[1000, 1000, 1000], 0, rules);
    g.start_hand();
    assert_eq!(g.ante, 2, "an eighth of the big blind of 20");
    assert_eq!(g.pot(), 3 * 2 + 10 + 20);
    let bets: Vec<u32> = g.players.iter().map(|p| p.bet).collect();
    assert_eq!(bets, vec![0, 10, 20]);
    assert_eq!(g.legal(0).to_call, 20);
    fold_around(&mut g);
    // The big blind wins everyone's ante and the small blind.
    assert_eq!(chips(&g), vec![998, 988, 1014]);
}

#[test]
fn a_player_can_be_all_in_from_the_ante() {
    let rules = Rules {
        antes: true,
        ..Rules::default()
    };
    let mut g = table(&[1000, 1000, 1000, 1], 0, rules);
    g.start_hand();
    assert!(g.players[3].all_in);
    assert!(g.players[3].live());
    assert_eq!(g.players[3].total, 1);
    assert_eq!(g.chips_in_play(), g.total_chips());
}

#[test]
fn the_straddle_acts_last_before_the_flop() {
    let rules = Rules {
        straddle: true,
        ..Rules::default()
    };
    let mut g = table(&[1000; 4], 0, rules);
    g.start_hand();
    // Button 0, small blind 1, big blind 2, straddle 3; seat 0 acts first.
    assert_eq!(g.straddle_seat, Some(3));
    assert_eq!(g.players[3].bet, 40);
    assert_eq!(g.to_act, 0);
    assert_eq!(g.blind_bet(), 40);
    let l = g.legal(0);
    assert_eq!((l.to_call, l.min_to), (40, 80));
    g.act(Action::Call).unwrap();
    g.act(Action::Call).unwrap();
    g.act(Action::Call).unwrap();
    // The straddler gets the option to raise.
    assert_eq!(g.to_act, 3);
    let l = g.legal(3);
    assert!(l.can_check && l.can_raise);
    g.act(Action::Check).unwrap();
    assert_eq!(g.street, Street::Flop);
    assert_eq!(g.to_act, 1);
}

#[test]
fn no_straddle_with_three_players() {
    let rules = Rules {
        straddle: true,
        ..Rules::default()
    };
    let mut g = table(&[1000; 3], 0, rules);
    g.start_hand();
    assert_eq!(g.straddle_seat, None);
    assert_eq!(g.current_bet, 20);
}

#[test]
fn dead_button_moves_the_big_blind_one_player_at_a_time() {
    let rules = Rules {
        dead_button: true,
        ..Rules::default()
    };
    let mut g = table(&[1000; 5], 0, rules);
    g.start_hand();
    assert_eq!((g.button, g.sb_seat, g.bb_seat), (0, 1, 2));
    fold_around(&mut g);
    // The small blind leaves: the button lands on the empty seat next hand.
    g.remove_player(1).unwrap();
    g.start_hand();
    assert_eq!((g.button, g.sb_seat, g.bb_seat), (1, 2, 3));
    assert!(g.players[1].vacant && !g.sb_dead);
    assert_eq!(g.to_act, 4, "first to act is left of the big blind");
    fold_around(&mut g);
    g.start_hand();
    assert_eq!((g.button, g.sb_seat, g.bb_seat), (2, 3, 4));
}

#[test]
fn dead_small_blind_when_the_big_blind_leaves() {
    let rules = Rules {
        dead_button: true,
        ..Rules::default()
    };
    let mut g = table(&[1000; 4], 0, rules);
    g.start_hand();
    fold_around(&mut g);
    g.remove_player(2).unwrap(); // last hand's big blind
    g.start_hand();
    assert_eq!((g.button, g.sb_seat, g.bb_seat), (1, 2, 3));
    assert!(g.sb_dead);
    assert_eq!(g.pot(), 20, "only the big blind is posted");
    assert_eq!(g.chips_in_play(), g.total_chips());
}

#[test]
fn without_dead_button_the_button_just_moves_on() {
    let mut g = table(&[1000; 5], 0, Rules::default());
    g.start_hand();
    fold_around(&mut g);
    g.remove_player(1).unwrap();
    g.start_hand();
    assert_eq!((g.button, g.sb_seat, g.bb_seat), (2, 3, 4));
}

/// Seat 1: A♠ A♥, seat 2: 7♣ 2♦, seat 0: 8♠ 3♥; board K♣ 9♦ 4♠ J♥ 6♣.
const MUCK_DECK: &str = "As 7c 8s Ah 2d 3h 5d Kc 9d 4s 5h Jh 5c 6c";

fn play_to_showdown(rules: Rules, river_bet: bool) -> Game {
    let mut g = table(&[1000; 3], 0, rules);
    g.start_hand_with_deck(stacked(MUCK_DECK));
    g.act(Action::Call).unwrap(); // seat 0
    g.act(Action::Call).unwrap(); // seat 1
    g.act(Action::Check).unwrap(); // seat 2
    for _ in 0..2 {
        for _ in 0..3 {
            g.act(Action::Check).unwrap();
        }
    }
    assert_eq!(g.street, Street::River);
    if river_bet {
        g.act(Action::RaiseTo(20)).unwrap(); // seat 1 bets
        g.act(Action::Call).unwrap();
        g.act(Action::Call).unwrap();
    } else {
        for _ in 0..3 {
            g.act(Action::Check).unwrap();
        }
    }
    assert_eq!(g.state, State::HandOver);
    g
}

#[test]
fn losers_may_muck_after_the_winner_shows() {
    let rules = Rules {
        muck_losers: true,
        ..Rules::default()
    };
    let g = play_to_showdown(rules, true);
    let shown: Vec<bool> = g.players.iter().map(|p| p.shown).collect();
    assert_eq!(shown, vec![false, true, false]);
    assert_eq!(chips(&g), vec![960, 1080, 960]);
    assert!(g.log.iter().any(|l| l.contains("mucks")));
}

#[test]
fn without_a_river_bet_showing_starts_left_of_the_button() {
    let rules = Rules {
        muck_losers: true,
        ..Rules::default()
    };
    let g = play_to_showdown(rules, false);
    // Seat 1 is first and wins; seats 2 and 0 are beaten and muck.
    let shown: Vec<bool> = g.players.iter().map(|p| p.shown).collect();
    assert_eq!(shown, vec![false, true, false]);
}

#[test]
fn without_the_muck_rule_everyone_shows() {
    let g = play_to_showdown(Rules::default(), true);
    assert!(g.players.iter().all(|p| p.shown));
    assert_eq!(chips(&g), vec![960, 1080, 960]);
}

/// Seat 1: K♦ Q♦, seat 2: 7♣ 2♦, seat 0: A♠ A♥; board K♣ 9♦ 4♠ J♥ 6♣.
const BUST_DECK: &str = "Kd 7c As Qd 2d Ah 5d Kc 9d 4s 5h Jh 5c 6c";

fn bust_seat_two(rules: Rules) -> Game {
    let mut g = table(&[1000, 1000, 50], 0, rules);
    g.start_hand_with_deck(stacked(BUST_DECK));
    g.act(Action::AllIn).unwrap(); // seat 0
    g.act(Action::Fold).unwrap(); // seat 1
    g.act(Action::Call).unwrap(); // seat 2 calls all-in
    while g.state == State::Runout {
        g.runout_step();
    }
    assert_eq!(chips(&g), vec![1060, 990, 0]);
    g
}

#[test]
fn a_busted_player_can_rebuy_during_the_rebuy_period() {
    let rules = Rules {
        rebuy_levels: 1,
        ..Rules::default()
    };
    let mut g = bust_seat_two(rules);
    assert!(g.players[2].rebuy_pending);
    assert!(!g.players[2].out);
    let before = g.total_chips();
    g.take_events();
    g.rebuy(2).unwrap();
    assert_eq!(g.take_events(), vec![GameEvent::Rebuy(2)]);
    assert_eq!(g.players[2].chips, 1000);
    assert_eq!(g.players[2].rebuys, 1);
    assert_eq!(g.total_chips(), before + 1000);
    assert!(g.rebuy(2).is_err(), "only one rebuy per bust");
    g.start_hand();
    assert!(g.players[2].in_hand);
    assert_eq!(g.chips_in_play(), g.total_chips());
}

#[test]
fn a_player_who_does_not_rebuy_is_eliminated_at_the_next_hand() {
    let rules = Rules {
        rebuy_levels: 1,
        ..Rules::default()
    };
    let mut g = bust_seat_two(rules);
    g.start_hand();
    assert!(g.players[2].out);
    assert_eq!(g.place_of(2), Some(3));
    assert!(!g.players[2].in_hand);
}

#[test]
fn declining_a_rebuy_eliminates_at_once() {
    let rules = Rules {
        rebuy_levels: 1,
        ..Rules::default()
    };
    let mut g = bust_seat_two(rules);
    g.decline_rebuy(2);
    assert!(g.players[2].out);
    assert!(!g.players[2].rebuy_pending);
}

#[test]
fn no_rebuys_after_the_rebuy_period() {
    let rules = Rules {
        rebuy_levels: 0,
        ..Rules::default()
    };
    let mut g = bust_seat_two(rules);
    assert!(g.players[2].out, "eliminated straight away");
    assert!(g.rebuy(2).is_err());
}

#[test]
fn the_game_ends_at_the_hand_limit() {
    let rules = Rules {
        hand_limit: 3,
        ..Rules::default()
    };
    let mut g = table(&[1000; 3], 0, rules);
    for _ in 0..3 {
        g.start_hand();
        assert_eq!(g.state, State::Betting);
        fold_around(&mut g);
    }
    assert!(g.tournament_over());
    g.start_hand();
    assert_eq!(g.state, State::GameOver);
    assert_eq!(g.hand_no, 3);
    let standings = g.standings();
    assert_eq!(standings.len(), 3);
    assert!(standings[0].chips >= standings[1].chips);
    assert!(standings[1].chips >= standings[2].chips);
    assert!(g.log.last().unwrap().contains("Hand limit reached"));
}

#[test]
fn players_join_and_leave_only_between_hands() {
    let mut g = table(&[1000; 4], 0, Rules::default());
    g.start_hand();
    assert!(g.remove_player(3).is_err());
    assert!(
        g.seat_player(3, "Zed", false, Personality::default(), 500)
            .is_err()
    );
    fold_around(&mut g);
    g.take_events();
    let before = g.total_chips();
    let gone = g.remove_player(3).unwrap();
    assert_eq!(g.total_chips(), before - u64::from(gone));
    assert!(g.players[3].vacant && g.players[3].out);
    assert!(
        g.seat_player(1, "Zed", false, Personality::default(), 500)
            .is_err(),
        "taken"
    );
    g.seat_player(3, "Zed", false, Personality::default(), 500)
        .unwrap();
    assert_eq!(
        g.take_events(),
        vec![GameEvent::Left(3), GameEvent::Joined(3)]
    );
    g.start_hand();
    assert!(g.players[3].in_hand);
    assert_eq!(g.chips_in_play(), g.total_chips());
    let standings = g.standings();
    let left = standings.iter().find(|s| s.place.is_none()).unwrap();
    assert!(left.status.contains("left in hand 1"));
}

#[test]
fn statistics_count_voluntary_play_only() {
    let mut g = table(&[1000; 3], 0, Rules::default());
    g.start_hand();
    g.act(Action::RaiseTo(60)).unwrap(); // seat 0 raises
    g.act(Action::Call).unwrap(); // seat 1 (small blind) calls
    g.act(Action::Fold).unwrap(); // seat 2 (big blind) folds
    let s0 = g.players[0].stats;
    let s1 = g.players[1].stats;
    let s2 = g.players[2].stats;
    assert_eq!((s0.hands, s0.vpip, s0.pfr, s0.bets), (1, 1, 1, 1));
    assert_eq!((s1.vpip, s1.pfr, s1.calls), (1, 0, 1));
    assert_eq!(
        (s2.hands, s2.vpip, s2.calls),
        (1, 0, 0),
        "posting a blind is not playing"
    );
    // Re-raising in the same hand does not count the hand twice.
    g.act(Action::RaiseTo(40)).unwrap(); // seat 1 bets the flop
    g.act(Action::RaiseTo(200)).unwrap(); // seat 0 raises
    assert_eq!(g.players[0].stats.vpip, 1);
    assert_eq!(g.players[0].stats.bets, 2);
    assert_eq!(g.players[0].stats.postflop_bets, 1);
}

#[test]
fn nobody_folds_to_a_big_blind_that_is_all_in_from_the_ante() {
    // The big blind (seat 2) has only the ante, so it posts no blind at all.
    let rules = Rules {
        antes: true,
        ..Rules::default()
    };
    let mut g = table(&[1000, 1000, 2], 0, rules);
    g.start_hand();
    assert!(g.players[2].all_in);
    assert_eq!(g.players[2].bet, 0);
    assert_eq!(g.to_act, 0);
    g.act(Action::Fold).unwrap();
    // The small blind already covers the all-in player and is not asked to act.
    assert_eq!(g.state, State::Runout);
    while g.state == State::Runout {
        g.runout_step();
    }
    assert_eq!(g.chips_in_play(), g.total_chips());
    // The all-in player can win at most 2 from each opponent: 2 + 2 + 2 = 6.
    assert!(g.players[2].chips <= 6);
}
