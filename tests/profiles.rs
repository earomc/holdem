//! The style estimate recognises each bot archetype from its own play.

use holdem::bot::{self, ARCHETYPES};
use holdem::card::Rng;
use holdem::game::{Game, Rules, State};
use holdem::stats::{MIN_HANDS, Stats, profile};

/// Plays bot-only games and returns each archetype's statistics, split into
/// samples of about `chunk` counted hands.
fn samples(games: u64, chunk: u32) -> Vec<(&'static str, Stats)> {
    let names: Vec<String> = bot::DEFAULT_NAMES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let mut out = Vec::new();
    for game_no in 0..games {
        let seats = 6;
        let rules = Rules {
            hand_limit: 400,
            hands_per_level: 0,
            ..Rules::default()
        };
        let mut rng = Rng::seeded(77 + game_no);
        let mut g = Game::empty(rules, seats, rng.clone());
        g.end_when_humans_out = false;
        g.bot_samples = 120;
        for (i, (name, style)) in bot::roster_from(&names, seats, &mut rng)
            .into_iter()
            .enumerate()
        {
            g.seat_player(i, &name, false, style, 1500).unwrap();
        }
        let mut last: Vec<Stats> = vec![Stats::default(); seats];
        loop {
            for i in 0..seats {
                // Take a sample whenever a player has enough new hands; refill busted seats.
                let s = g.players[i].stats;
                if s.hands - last[i].hands >= chunk {
                    let d = Stats {
                        hands: s.hands - last[i].hands,
                        vpip: s.vpip - last[i].vpip,
                        pfr: s.pfr - last[i].pfr,
                        bets: s.bets - last[i].bets,
                        calls: s.calls - last[i].calls,
                        postflop_bets: s.postflop_bets - last[i].postflop_bets,
                        bluffs: s.bluffs - last[i].bluffs,
                    };
                    out.push((g.players[i].personality.style, d));
                    last[i] = s;
                }
                if g.players[i].out {
                    let taken: Vec<String> = g.players.iter().map(|p| p.name.clone()).collect();
                    let style = ARCHETYPES[i % ARCHETYPES.len()].personality;
                    let name = bot::unused_name(&names, &taken, &mut rng);
                    g.seat_player(i, &name, false, style, 1500).unwrap();
                    last[i] = Stats::default();
                }
            }
            g.start_hand();
            if g.state == State::GameOver {
                break;
            }
            while matches!(g.state, State::Betting | State::Runout) {
                if g.state == State::Runout {
                    g.runout_step();
                } else {
                    g.bot_move();
                }
            }
        }
    }
    out
}

#[test]
fn styles_are_recognised_from_a_few_hundred_hands() {
    let data = samples(12, 200);
    let mut right = 0;
    let mut top_two = 0;
    for (style, stats) in &data {
        let p = profile(stats, MIN_HANDS).expect("enough hands");
        if p.ranking[0].0 == *style {
            right += 1;
        }
        if p.ranking[..2].iter().any(|(name, _)| name == style) {
            top_two += 1;
        }
    }
    let n = data.len();
    assert!(n >= 40, "only {n} samples");
    println!("{n} samples: {right} first, {top_two} in the top two");
    // Neighbouring styles (Rock/Nit, Loose-aggressive/Gambler) are genuinely similar,
    // so the right style must be first most of the time and nearly always in the top two.
    assert!(right * 100 >= n * 60, "{right} of {n} samples recognised");
    assert!(top_two * 100 >= n * 85, "{top_two} of {n} in the top two");
}
