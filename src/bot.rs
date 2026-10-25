//! Computer opponents.
//!
//! Every bot estimates its winning chances with a Monte Carlo simulation against
//! random opposing hands, converts that into a table-size-independent strength
//! rating, and then decides what to do according to its personality: how loose
//! or tight it plays, how aggressive it is, and how often it bluffs. Bots only
//! ever look at information a real player at the table could see.

use crate::card::{Card, Rng, Suit};
use crate::eval;
use crate::game::{Action, Game, Legal, Street};
use crate::stats::Features;

/// How a bot plays. All values are between 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    /// Archetype name shown in the interface.
    pub style: &'static str,
    /// Higher means more hands are played.
    pub looseness: f32,
    /// Higher means more betting and raising instead of checking and calling.
    pub aggression: f32,
    /// How often weak hands are bet as bluffs.
    pub bluff: f32,
}

impl Default for Personality {
    fn default() -> Personality {
        Personality {
            style: "Balanced",
            looseness: 0.5,
            aggression: 0.5,
            bluff: 0.1,
        }
    }
}

/// A playing style: how a bot with it plays, what it is like, and the
/// statistics it really produces.
#[derive(Clone, Copy, Debug)]
pub struct Archetype {
    pub personality: Personality,
    /// What the style is like, for the help.
    pub description: &'static str,
    /// Its typical statistics, measured over 400 bot-only games (about 110,000
    /// counted hands) with `cargo run --release --example calibrate`. The
    /// style estimate compares a player's numbers with these.
    pub measured: Features,
}

const fn archetype(
    style: &'static str,
    (looseness, aggression, bluff): (f32, f32, f32),
    description: &'static str,
    (vpip, pfr, aggressive, bluffs): (f32, f32, f32, f32),
) -> Archetype {
    Archetype {
        personality: Personality {
            style,
            looseness,
            aggression,
            bluff,
        },
        description,
        measured: Features {
            vpip,
            pfr,
            aggression: aggressive,
            bluff: bluffs,
        },
    }
}

/// Eight distinct playing styles; a table never has two bots with the same
/// one. Each: name, (looseness, aggression, bluff), description, and the
/// measured (VPIP, PFR, aggression, bluff share).
pub const ARCHETYPES: [Archetype; 8] = [
    archetype(
        "Rock",
        (0.15, 0.35, 0.02),
        "tight and cautious: few hands, mostly called rather than raised",
        (0.134, 0.021, 0.246, 0.032),
    ),
    archetype(
        "Nit",
        (0.05, 0.50, 0.00),
        "plays almost nothing but premium hands",
        (0.073, 0.019, 0.372, 0.004),
    ),
    archetype(
        "Tight-aggressive",
        (0.35, 0.65, 0.08),
        "few hands, played forcefully",
        (0.280, 0.116, 0.444, 0.110),
    ),
    archetype(
        "Shark",
        (0.50, 0.60, 0.12),
        "solid and balanced, the hardest to read",
        (0.387, 0.139, 0.403, 0.145),
    ),
    archetype(
        "Loose-aggressive",
        (0.65, 0.80, 0.18),
        "many hands, lots of raising",
        (0.503, 0.279, 0.556, 0.187),
    ),
    archetype(
        "Gambler",
        (0.70, 0.55, 0.22),
        "plays many hands and bluffs often",
        (0.527, 0.161, 0.390, 0.226),
    ),
    archetype(
        "Calling station",
        (0.80, 0.15, 0.02),
        "calls a lot, rarely raises, hard to bluff",
        (0.576, 0.019, 0.101, 0.061),
    ),
    archetype(
        "Maniac",
        (0.85, 0.95, 0.30),
        "raises and bluffs constantly",
        (0.656, 0.434, 0.650, 0.211),
    ),
];

/// Bot names used when the config file does not list any.
pub const DEFAULT_NAMES: [&str; 16] = [
    "Ava", "Ben", "Chloe", "Dmitri", "Elena", "Frank", "Gina", "Hiro", "Ivan", "Jade", "Kofi",
    "Lena", "Mateo", "Nora", "Omar", "Priya",
];

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        items.swap(i, j);
    }
}

/// Picks `count` bots with the default names: unique names and, for up to
/// eight bots, unique playing styles.
pub fn roster(count: usize, rng: &mut Rng) -> Vec<(String, Personality)> {
    let names: Vec<String> = DEFAULT_NAMES.iter().map(|n| n.to_string()).collect();
    roster_from(&names, count, rng)
}

/// Like [`roster`], but with names from the given list. If there are fewer
/// names than bots, numbers are added ("Ava 2").
pub fn roster_from(names: &[String], count: usize, rng: &mut Rng) -> Vec<(String, Personality)> {
    let mut styles: Vec<Personality> = ARCHETYPES.iter().map(|a| a.personality).collect();
    shuffle(&mut styles, rng);
    let mut taken: Vec<String> = Vec::new();
    (0..count)
        .map(|i| {
            let name = unused_name(names, &taken, rng);
            taken.push(name.clone());
            (name, styles[i % styles.len()])
        })
        .collect()
}

/// A random name from `names` that nobody in `taken` uses yet.
pub fn unused_name(names: &[String], taken: &[String], rng: &mut Rng) -> String {
    let free: Vec<&String> = names.iter().filter(|n| !taken.contains(n)).collect();
    if !free.is_empty() {
        return free[rng.below(free.len() as u64) as usize].clone();
    }
    let base = if names.is_empty() {
        "Bot".to_string()
    } else {
        names[rng.below(names.len() as u64) as usize].clone()
    };
    (2..)
        .map(|k| format!("{} {}", base, k))
        .find(|candidate| !taken.contains(candidate))
        .expect("an unused number exists")
}

/// A new bot for an empty seat: an unused name and a random style.
pub fn newcomer(names: &[String], taken: &[String], rng: &mut Rng) -> (String, Personality) {
    let name = unused_name(names, taken, rng);
    let style = ARCHETYPES[rng.below(ARCHETYPES.len() as u64) as usize].personality;
    (name, style)
}

/// Whether a busted bot buys back in. Loose players rebuy more often.
pub fn wants_rebuy(personality: &Personality, rng: &mut Rng) -> bool {
    rng.chance(0.35 + 0.6 * personality.looseness)
}

/// Probability (0..=1) that the given hole cards win the hand against
/// `opponents` random hands, with ties counted pro rata. Monte Carlo over
/// `samples` random run-outs of the remaining board and opposing holdings.
pub fn equity(hole: &[Card], board: &[Card], opponents: usize, samples: u32, rng: &mut Rng) -> f32 {
    if opponents == 0 {
        return 1.0;
    }
    if hole.len() != 2 || board.len() > 5 || samples == 0 {
        return 0.5;
    }
    let mut known = [false; 52];
    for c in hole.iter().chain(board.iter()) {
        known[c.index()] = true;
    }
    let mut pool: Vec<Card> = Vec::with_capacity(52);
    for &suit in Suit::ALL.iter() {
        for rank in 2..=14u8 {
            let c = Card::new(rank, suit);
            if !known[c.index()] {
                pool.push(c);
            }
        }
    }
    let need_board = 5 - board.len();
    let draws = need_board + opponents * 2;
    if draws > pool.len() {
        return 0.5;
    }

    let mut total = 0.0f32;
    let mut cards: Vec<Card> = Vec::with_capacity(7);
    for _ in 0..samples {
        // Partial Fisher-Yates: only the first `draws` positions are needed.
        for i in 0..draws {
            let j = i + rng.below((pool.len() - i) as u64) as usize;
            pool.swap(i, j);
        }
        let mut full_board: Vec<Card> = board.to_vec();
        full_board.extend_from_slice(&pool[..need_board]);

        cards.clear();
        cards.extend_from_slice(hole);
        cards.extend_from_slice(&full_board);
        let mine = eval::evaluate(&cards);

        let mut beaten = false;
        let mut tied = 0u32;
        for o in 0..opponents {
            cards.clear();
            cards.push(pool[need_board + 2 * o]);
            cards.push(pool[need_board + 2 * o + 1]);
            cards.extend_from_slice(&full_board);
            let theirs = eval::evaluate(&cards);
            if theirs > mine {
                beaten = true;
                break;
            }
            if theirs == mine {
                tied += 1;
            }
        }
        if !beaten {
            total += 1.0 / (1 + tied) as f32;
        }
    }
    total / samples as f32
}

/// What a bot wants to do, before it is turned into a legal `Action`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Intent {
    Fold,
    CheckOrCall,
    /// Raise (or bet) so that the total bet this round is the given amount.
    RaiseTo(u32),
    AllIn,
}

/// Turns an intent into an action that is legal in the current spot. Bet sizes
/// are rounded to a multiple of `unit` chips, as people do.
fn resolve(intent: Intent, legal: &Legal, unit: u32) -> Action {
    let passive = if legal.can_check {
        Action::Check
    } else {
        Action::Call
    };
    match intent {
        Intent::Fold => {
            if legal.can_check {
                Action::Check
            } else {
                Action::Fold
            }
        }
        Intent::CheckOrCall => passive,
        Intent::AllIn => {
            if legal.can_raise {
                Action::AllIn
            } else {
                passive
            }
        }
        Intent::RaiseTo(target) => {
            if !legal.can_raise {
                return passive;
            }
            let unit = unit.max(1);
            let rounded = (target.saturating_add(unit / 2) / unit) * unit;
            let t = rounded.clamp(legal.min_to, legal.max_to);
            // Chips this raise adds from the stack.
            let committed = (t + legal.stack - legal.max_to) as u64;
            // Raising away most of the stack is simply an all-in.
            if t >= legal.max_to || committed * 4 >= legal.stack as u64 * 3 {
                Action::AllIn
            } else {
                Action::RaiseTo(t)
            }
        }
    }
}

/// 0.0 for the first player to act after the flop, 1.0 for the button.
fn position_factor(game: &Game, seat: usize) -> f32 {
    let n = game.players.len();
    let order: Vec<usize> = (1..=n)
        .map(|k| (game.button + k) % n)
        .filter(|&i| !game.players[i].out)
        .collect();
    if order.len() <= 1 {
        return 0.5;
    }
    let idx = order.iter().position(|&i| i == seat).unwrap_or(0);
    idx as f32 / (order.len() - 1) as f32
}

/// Decides the move for `seat`, which must be the player to act.
pub fn decide(game: &Game, seat: usize, rng: &mut Rng) -> Action {
    let legal = game.legal(seat);
    let personality = game.players[seat].personality;
    let intent = think(game, seat, &legal, &personality, rng);
    resolve(intent, &legal, (game.bb / 2).max(1))
}

fn think(game: &Game, seat: usize, legal: &Legal, pers: &Personality, rng: &mut Rng) -> Intent {
    let me = &game.players[seat];
    let opponents = game.live_count().saturating_sub(1).max(1);
    let n = opponents + 1;
    let fair = 1.0 / n as f32;
    // Equity of pocket aces against n-1 random hands: the best starting hand at this table size.
    let best_start = 0.85 * 0.875f32.powi(n as i32 - 2);
    let spread = (best_start - fair).max(0.05);

    let raw = equity(&me.hole, &game.board, opponents, game.bot_samples, rng);
    let pos = position_factor(game, seat);

    if game.street == Street::PreFlop {
        // 0 is an average hand, 1 is the best possible starting hand.
        let strength = (raw - fair) / spread + 0.10 * (pos - 0.5);
        preflop(game, seat, legal, pers, strength, pos, rng)
    } else {
        postflop(
            game, seat, legal, pers, raw, fair, spread, pos, opponents, rng,
        )
    }
}

/// The strength score reached by only the best `share` of starting hands.
/// Measured over random hands at 2 to 9 players (`examples/strength.rs`); the
/// curve hardly depends on the table size because strength is normalised.
fn top_share(share: f32) -> f32 {
    const CURVE: [(f32, f32); 11] = [
        (0.00, 0.70),
        (0.05, 0.37),
        (0.10, 0.27),
        (0.20, 0.15),
        (0.30, 0.07),
        (0.40, 0.01),
        (0.50, -0.03),
        (0.60, -0.08),
        (0.75, -0.15),
        (0.90, -0.24),
        (1.00, -1.00),
    ];
    let share = share.clamp(0.0, 1.0);
    for w in CURVE.windows(2) {
        let ((x0, y0), (x1, y1)) = (w[0], w[1]);
        if share <= x1 {
            return y0 + (y1 - y0) * (share - x0) / (x1 - x0);
        }
    }
    -1.0
}

/// Share of starting hands a style plays when nobody has raised yet.
fn play_share(p: &Personality) -> f32 {
    0.08 + 0.75 * p.looseness
}

/// Share of starting hands a style opens with a raise.
fn raise_share(p: &Personality) -> f32 {
    play_share(p) * 0.95 * p.aggression.powf(1.6)
}

fn preflop(
    game: &Game,
    seat: usize,
    legal: &Legal,
    pers: &Personality,
    strength: f32,
    pos: f32,
    rng: &mut Rng,
) -> Intent {
    let bb = game.bb.max(1);
    let aggr = pers.aggression;
    let play_at = top_share(play_share(pers));
    let raise_at = top_share(raise_share(pers));
    let stack_bb = legal.stack as f32 / bb as f32;
    let call_bb = legal.to_call as f32 / bb as f32;

    // Short stacks just push or fold.
    if stack_bb <= 8.0 {
        return if strength >= top_share(0.12 + 0.35 * pers.looseness) {
            Intent::AllIn
        } else {
            Intent::Fold
        };
    }

    // Before the flop the blinds (and a straddle) are not a raise yet.
    let opening_level = game.blind_bet().max(bb);
    let unraised = game.current_bet <= opening_level;
    let open_raise = |rng: &mut Rng| {
        let limpers = game
            .players
            .iter()
            .enumerate()
            .filter(|&(i, p)| {
                i != seat
                    && i != game.bb_seat
                    && Some(i) != game.straddle_seat
                    && p.live()
                    && p.bet >= opening_level
            })
            .count() as u32;
        let size = 2.3 + 0.8 * aggr + 0.3 * rng.unit();
        let open = (size * opening_level as f32) as u32 + limpers * bb;
        Intent::RaiseTo(open.max(2 * opening_level))
    };
    if unraised {
        if strength >= raise_at && rng.chance(0.9) {
            return open_raise(rng);
        }
        if strength >= play_at {
            return Intent::CheckOrCall;
        }
        if rng.chance(pers.bluff * 0.4 * (0.5 + pos)) {
            return open_raise(rng);
        }
        return Intent::Fold;
    }

    // Facing a raise: continue with a narrower range, narrower still for big raises.
    let shrink = 1.0 + 0.06 * (call_bb - 2.0).max(0.0);
    let continue_at = top_share(play_share(pers) * (0.3 + 0.6 * pers.looseness) / shrink);
    let reraise_at = top_share(raise_share(pers) * 0.3);
    if strength >= reraise_at && rng.chance(0.4 + 0.5 * aggr) {
        if stack_bb <= 30.0 {
            return Intent::AllIn;
        }
        return Intent::RaiseTo(game.current_bet * 3);
    }
    if strength >= continue_at {
        // Do not pay half the stack without a premium hand.
        let cost = legal.to_call as f32 / legal.stack.max(1) as f32;
        if cost > 0.5 && strength < top_share(0.08) {
            return Intent::Fold;
        }
        return Intent::CheckOrCall;
    }
    if stack_bb > 20.0 && rng.chance(pers.bluff * 0.1) {
        return Intent::RaiseTo(game.current_bet * 3);
    }
    Intent::Fold
}

#[allow(clippy::too_many_arguments)]
fn postflop(
    game: &Game,
    _seat: usize,
    legal: &Legal,
    pers: &Personality,
    raw: f32,
    fair: f32,
    spread: f32,
    pos: f32,
    opponents: usize,
    rng: &mut Rng,
) -> Intent {
    let bb = game.bb.max(1);
    let pot = game.pot().max(1) as f32;
    let to_call = legal.to_call as f32;
    let aggr = pers.aggression;
    let tight = 1.0 - pers.looseness;

    let value_at = 0.70 - 0.22 * aggr + 0.10 * tight;
    let strong_at = value_at + 0.30;
    let marginal_at = 0.25 + 0.25 * tight;
    let jitter = (rng.unit() - 0.5) * 0.2;

    if legal.can_check {
        // Nobody has bet: take the lead, or take a free card.
        let rel = (raw - fair) / spread;
        let bet_to =
            |frac: f32| -> Intent { Intent::RaiseTo(((pot * (frac + jitter)) as u32).max(bb)) };
        if rel >= strong_at {
            if rng.chance(0.35 + 0.60 * aggr) {
                return bet_to(0.75);
            }
            return Intent::CheckOrCall; // slow play
        }
        if rel >= value_at {
            if rng.chance(0.10 + 0.85 * aggr) {
                return bet_to(0.55);
            }
            return Intent::CheckOrCall;
        }
        if rel >= marginal_at {
            if rng.chance(aggr * aggr * 0.8) {
                return bet_to(0.4);
            }
            return Intent::CheckOrCall;
        }
        let multiway = if opponents <= 2 { 1.0 } else { 0.5 };
        if rng.chance(pers.bluff * (0.7 + 0.8 * pos) * multiway) {
            return bet_to(0.60);
        }
        return Intent::CheckOrCall;
    }

    // Facing a bet. Opponents who bet hold better than random cards, so shade the
    // equity down in proportion to the size of the bet, then compare with pot odds.
    let pressure = (to_call / pot).min(1.0);
    let adjusted = (raw - 0.30 * pressure * (1.25 - 0.60 * pers.looseness)).max(0.0);
    let rel = (adjusted - fair) / spread;
    let pot_odds = to_call / (pot + to_call);
    let required = pot_odds * (1.15 - 0.30 * pers.looseness);

    if adjusted < required {
        if opponents == 1 && rng.chance(pers.bluff * 0.15) {
            return Intent::RaiseTo(game.current_bet + ((pot + to_call) * 0.8) as u32);
        }
        return Intent::Fold;
    }
    let raise_to = |frac: f32| -> Intent {
        Intent::RaiseTo(game.current_bet + ((pot + to_call) * (frac + jitter)) as u32)
    };
    if rel >= strong_at && rng.chance(0.30 + 0.60 * aggr) {
        return raise_to(0.75);
    }
    if rel >= value_at + 0.10 && rng.chance(aggr * 0.60) {
        return raise_to(0.60);
    }
    // Aggressive bluffers sometimes raise as a (semi-)bluff instead of calling.
    if rel < value_at && rng.chance(pers.bluff * aggr * 0.5) {
        return raise_to(0.70);
    }
    Intent::CheckOrCall
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Deck, parse_cards};
    use crate::game::{Rules, State};

    fn hand(text: &str) -> Vec<Card> {
        parse_cards(text).expect("valid cards")
    }

    #[test]
    fn roster_has_unique_names_and_styles() {
        let mut rng = Rng::seeded(5);
        let r = roster(8, &mut rng);
        assert_eq!(r.len(), 8);
        let mut names: Vec<&String> = r.iter().map(|x| &x.0).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 8);
        let mut styles: Vec<&str> = r.iter().map(|x| x.1.style).collect();
        styles.sort_unstable();
        styles.dedup();
        assert_eq!(styles.len(), 8);
        assert!(!names.iter().any(|n| n.as_str() == "You"));
    }

    #[test]
    fn names_run_out_gracefully() {
        let mut rng = Rng::seeded(8);
        let names = vec!["Zed".to_string()];
        let r = roster_from(&names, 3, &mut rng);
        let got: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(got[0], "Zed");
        assert!(got[1].starts_with("Zed ") && got[2].starts_with("Zed "));
        assert_ne!(got[1], got[2]);
        assert_eq!(unused_name(&[], &[], &mut rng), "Bot 2");
        let taken = vec!["Zed".to_string()];
        let (name, _) = newcomer(&names, &taken, &mut rng);
        assert_ne!(name, "Zed");
    }

    #[test]
    fn equity_matches_known_values() {
        let mut rng = Rng::seeded(11);
        // Pocket aces against one random hand win about 85% of the time.
        let aa = equity(&hand("As Ah"), &[], 1, 6000, &mut rng);
        assert!((0.82..0.88).contains(&aa), "AA equity {}", aa);
        // Seven-deuce offsuit wins about 35% of the time heads-up.
        let bad = equity(&hand("7d 2c"), &[], 1, 6000, &mut rng);
        assert!((0.31..0.39).contains(&bad), "72o equity {}", bad);
        // A royal flush cannot lose.
        let nuts = equity(&hand("As Ks"), &hand("Qs Js Ts 2c 3d"), 3, 500, &mut rng);
        assert!(nuts > 0.999, "royal flush equity {}", nuts);
        // The more opponents, the lower the equity.
        let multi = equity(&hand("As Ah"), &[], 5, 3000, &mut rng);
        assert!(multi < aa - 0.2);
    }

    #[test]
    fn resolve_always_produces_a_legal_shape() {
        let open = Legal {
            to_call: 0,
            can_check: true,
            can_raise: true,
            min_to: 40,
            max_to: 1000,
            stack: 1000,
        };
        assert_eq!(resolve(Intent::Fold, &open, 1), Action::Check);
        assert_eq!(resolve(Intent::CheckOrCall, &open, 1), Action::Check);
        assert_eq!(
            resolve(Intent::RaiseTo(100), &open, 1),
            Action::RaiseTo(100)
        );
        assert_eq!(resolve(Intent::RaiseTo(10), &open, 1), Action::RaiseTo(40));
        assert_eq!(resolve(Intent::RaiseTo(900), &open, 1), Action::AllIn);
        assert_eq!(resolve(Intent::AllIn, &open, 1), Action::AllIn);
        // Sizes are rounded to the unit, but never below the minimum raise.
        assert_eq!(resolve(Intent::RaiseTo(59), &open, 10), Action::RaiseTo(60));
        assert_eq!(resolve(Intent::RaiseTo(41), &open, 10), Action::RaiseTo(40));
        assert_eq!(resolve(Intent::RaiseTo(30), &open, 25), Action::RaiseTo(40));

        let locked = Legal {
            to_call: 30,
            can_check: false,
            can_raise: false,
            min_to: 130,
            max_to: 500,
            stack: 500,
        };
        assert_eq!(resolve(Intent::Fold, &locked, 1), Action::Fold);
        assert_eq!(resolve(Intent::RaiseTo(300), &locked, 1), Action::Call);
        assert_eq!(resolve(Intent::AllIn, &locked, 1), Action::Call);
    }

    /// Six players, button 0, so seat 3 is first to act. Seat 3 gets the 3rd and 9th card.
    fn spot(seat3_cards: &str, filler: &str) -> Game {
        let rules = Rules {
            starting_chips: 1500,
            small_blind: 10,
            hands_per_level: 0,
            ..Rules::default()
        };
        let mut g = Game::new(rules, 6, Rng::seeded(3));
        g.setup(&[1500; 6], 0);
        let s3 = hand(seat3_cards);
        let mut top = hand(filler);
        // deal positions 1..=12 -> seats 1,2,3,4,5,0,1,2,3,4,5,0
        top.insert(2, s3[0]);
        top.insert(8, s3[1]);
        g.start_hand_with_deck(Deck::stacked(&top));
        assert_eq!(g.state, State::Betting);
        assert_eq!(g.to_act, 3);
        g
    }

    #[test]
    fn aces_are_never_folded_and_usually_raised() {
        let mut g = spot("As Ah", "2c 3c 4c 5c 6c 8d 9d Td Jd Qd");
        let mut raises = 0;
        let mut trials = 0;
        for style in ARCHETYPES.iter().map(|a| a.personality) {
            g.players[3].personality = style;
            for seed in 0..12 {
                let mut rng = Rng::seeded(seed);
                let a = decide(&g, 3, &mut rng);
                assert_ne!(a, Action::Fold, "{} folded aces", style.style);
                trials += 1;
                if matches!(a, Action::RaiseTo(_) | Action::AllIn) {
                    raises += 1;
                }
            }
        }
        assert!(raises * 2 > trials, "only {} of {} raised", raises, trials);
    }

    #[test]
    fn seven_deuce_is_mostly_folded_from_early_position() {
        let mut g = spot("7s 2d", "2c 3c 4c 5c 6c 8d 9d Td Jd Qd");
        let mut folds = 0;
        let mut trials = 0;
        for style in ARCHETYPES.iter().map(|a| a.personality) {
            g.players[3].personality = style;
            for seed in 0..25 {
                let mut rng = Rng::seeded(seed + 100);
                let a = decide(&g, 3, &mut rng);
                trials += 1;
                if a == Action::Fold {
                    folds += 1;
                }
            }
        }
        assert!(
            folds * 10 >= trials * 8,
            "only {} of {} folded",
            folds,
            trials
        );
    }

    #[test]
    fn decisions_are_always_legal_in_random_spots() {
        let rules = Rules {
            starting_chips: 800,
            small_blind: 10,
            hands_per_level: 4,
            antes: true,
            straddle: true,
            ..Rules::default()
        };
        let mut g = Game::new(rules, 6, Rng::seeded(99));
        g.bot_samples = 40;
        let mut decisions = 0;
        for _ in 0..60 {
            g.start_hand();
            if g.state == State::GameOver {
                break;
            }
            while matches!(g.state, State::Betting | State::Runout) {
                if g.state == State::Runout {
                    g.runout_step();
                    continue;
                }
                let seat = g.to_act;
                let mut rng = Rng::seeded(decisions as u64);
                let a = decide(&g, seat, &mut rng);
                decisions += 1;
                let mut probe = g.clone();
                assert!(
                    probe.act(a).is_ok(),
                    "bot proposed an illegal action {:?}",
                    a
                );
                g.act(a).unwrap();
            }
        }
        assert!(decisions > 100);
    }
}
