//! Measures the statistics each bot archetype produces, to calibrate
//! `bot::Archetype::measured`. Run with `cargo run --release --example calibrate`.

use holdem::bot::{self, ARCHETYPES};
use holdem::card::Rng;
use holdem::game::{Game, Rules, State};
use holdem::stats::{Stats, closest};
use std::collections::BTreeMap;

fn main() {
    let mut totals: BTreeMap<&'static str, Stats> = BTreeMap::new();
    let names: Vec<String> = bot::DEFAULT_NAMES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    let games: u64 = std::env::var("GAMES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(80);
    for game_no in 0..games {
        let seats = 3 + (game_no % 7) as usize;
        let rules = Rules {
            hand_limit: 200,
            hands_per_level: std::env::var("HPL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8),
            ..Rules::default()
        };
        let mut rng = Rng::seeded(1000 + game_no);
        let mut g = Game::empty(rules, seats, rng.clone());
        g.end_when_humans_out = false;
        for (i, (name, style)) in bot::roster_from(&names, seats, &mut rng)
            .into_iter()
            .enumerate()
        {
            g.seat_player(i, &name, false, style, 1500).unwrap();
        }
        loop {
            // Keep the table full: busted bots are replaced, stats are kept.
            for i in 0..seats {
                if g.players[i].out && g.players[i].eliminated.is_some() {
                    let taken: Vec<String> = g.players.iter().map(|p| p.name.clone()).collect();
                    let (name, style) = bot::newcomer(&names, &taken, &mut rng);
                    g.seat_player(i, &name, false, style, 1500).unwrap();
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
        let mut add = |style: &'static str, s: &Stats| {
            let t = totals.entry(style).or_default();
            t.hands += s.hands;
            t.vpip += s.vpip;
            t.pfr += s.pfr;
            t.bets += s.bets;
            t.calls += s.calls;
            t.postflop_bets += s.postflop_bets;
            t.bluffs += s.bluffs;
        };
        for p in &g.players {
            if !p.vacant {
                add(p.personality.style, &p.stats);
            }
        }
        for f in &g.history {
            let style = ARCHETYPES
                .iter()
                .find(|a| a.personality.style == f.style)
                .map(|a| a.personality.style)
                .unwrap();
            add(style, &f.stats);
        }
    }
    for a in &ARCHETYPES {
        let s = totals[a.personality.style];
        let x = s.features();
        println!(
            "{:<17} ({:.3}, {:.3}, {:.3}, {:.3})   {} hands, classified as {}",
            a.personality.style,
            x.vpip,
            x.pfr,
            x.aggression,
            x.bluff,
            s.hands,
            closest(&x)
        );
    }
}
