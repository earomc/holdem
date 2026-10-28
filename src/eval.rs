//! Poker hand evaluation.
//!
//! `evaluate` takes five to seven cards and returns a [`Score`]: a `u32` in which a
//! bigger number is always a better hand. The layout is
//! `category << 20 | k0 << 16 | k1 << 12 | k2 << 8 | k3 << 4 | k4`, where `k0..k4`
//! are the ranks that break ties, most significant first.

use crate::card::Card;

pub type Score = u32;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Category {
    HighCard,
    Pair,
    TwoPair,
    ThreeOfAKind,
    Straight,
    Flush,
    FullHouse,
    FourOfAKind,
    StraightFlush,
}

impl Category {
    pub fn name(self) -> &'static str {
        match self {
            Category::HighCard => "High Card",
            Category::Pair => "Pair",
            Category::TwoPair => "Two Pair",
            Category::ThreeOfAKind => "Three of a Kind",
            Category::Straight => "Straight",
            Category::Flush => "Flush",
            Category::FullHouse => "Full House",
            Category::FourOfAKind => "Four of a Kind",
            Category::StraightFlush => "Straight Flush",
        }
    }
}

fn pack(category: Category, k: [u8; 5]) -> Score {
    ((category as u32) << 20)
        | (u32::from(k[0]) << 16)
        | (u32::from(k[1]) << 12)
        | (u32::from(k[2]) << 8)
        | (u32::from(k[3]) << 4)
        | u32::from(k[4])
}

pub fn category(score: Score) -> Category {
    match score >> 20 {
        0 => Category::HighCard,
        1 => Category::Pair,
        2 => Category::TwoPair,
        3 => Category::ThreeOfAKind,
        4 => Category::Straight,
        5 => Category::Flush,
        6 => Category::FullHouse,
        7 => Category::FourOfAKind,
        _ => Category::StraightFlush,
    }
}

fn tiebreakers(score: Score) -> [u8; 5] {
    [
        ((score >> 16) & 0xF) as u8,
        ((score >> 12) & 0xF) as u8,
        ((score >> 8) & 0xF) as u8,
        ((score >> 4) & 0xF) as u8,
        (score & 0xF) as u8,
    ]
}

/// Highest card of a straight contained in `mask` (bit `r` set = rank `r` present).
/// The ace also counts low, so A-2-3-4-5 is a straight with a high card of 5.
fn straight_high(mask: u16) -> Option<u8> {
    let mut m = mask;
    if m & (1u16 << 14) != 0 {
        m |= 1u16 << 1;
    }
    for high in (5..=14u8).rev() {
        let window: u16 = 0b11111u16 << (high - 4);
        if m & window == window {
            return Some(high);
        }
    }
    None
}

/// The highest `n` ranks (descending) present in `mask`, ignoring ranks in `exclude`.
fn top_ranks(mask: u16, exclude: &[u8], n: usize) -> [u8; 5] {
    let mut out = [0u8; 5];
    let mut count = 0;
    for rank in (2..=14u8).rev() {
        if count == n {
            break;
        }
        if mask & (1u16 << rank) != 0 && !exclude.contains(&rank) {
            out[count] = rank;
            count += 1;
        }
    }
    out
}

/// Scores the best five-card hand that can be made from 5, 6 or 7 distinct cards.
pub fn evaluate(cards: &[Card]) -> Score {
    debug_assert!(
        (5..=7).contains(&cards.len()),
        "evaluate expects between five and seven cards"
    );

    let mut rank_count = [0u8; 15];
    let mut suit_count = [0u8; 4];
    let mut suit_mask = [0u16; 4];
    let mut all_mask: u16 = 0;
    for c in cards {
        let s = c.suit.index();
        rank_count[c.rank as usize] += 1;
        suit_count[s] += 1;
        suit_mask[s] |= 1u16 << c.rank;
        all_mask |= 1u16 << c.rank;
    }

    let flush_suit = (0..4).find(|&s| suit_count[s] >= 5);

    if let Some(s) = flush_suit
        && let Some(high) = straight_high(suit_mask[s])
    {
        return pack(Category::StraightFlush, [high, 0, 0, 0, 0]);
    }

    let mut quad: Option<u8> = None;
    let mut trips = [0u8; 2];
    let mut n_trips = 0usize;
    let mut pairs = [0u8; 3];
    let mut n_pairs = 0usize;
    for rank in (2..=14u8).rev() {
        match rank_count[rank as usize] {
            4 => quad = Some(rank),
            3 => {
                trips[n_trips] = rank;
                n_trips += 1;
            }
            2 => {
                pairs[n_pairs] = rank;
                n_pairs += 1;
            }
            _ => {}
        }
    }

    if let Some(q) = quad {
        let kicker = top_ranks(all_mask, &[q], 1)[0];
        return pack(Category::FourOfAKind, [q, kicker, 0, 0, 0]);
    }

    if n_trips >= 1 && (n_trips >= 2 || n_pairs >= 1) {
        // With two sets of trips the lower one plays as the pair.
        let pair = if n_trips >= 2 {
            if n_pairs >= 1 {
                trips[1].max(pairs[0])
            } else {
                trips[1]
            }
        } else {
            pairs[0]
        };
        return pack(Category::FullHouse, [trips[0], pair, 0, 0, 0]);
    }

    if let Some(s) = flush_suit {
        return pack(Category::Flush, top_ranks(suit_mask[s], &[], 5));
    }

    if let Some(high) = straight_high(all_mask) {
        return pack(Category::Straight, [high, 0, 0, 0, 0]);
    }

    if n_trips == 1 {
        let k = top_ranks(all_mask, &[trips[0]], 2);
        return pack(Category::ThreeOfAKind, [trips[0], k[0], k[1], 0, 0]);
    }

    if n_pairs >= 2 {
        // With three pairs the third pair's rank may supply the kicker.
        let k = top_ranks(all_mask, &[pairs[0], pairs[1]], 1);
        return pack(Category::TwoPair, [pairs[0], pairs[1], k[0], 0, 0]);
    }

    if n_pairs == 1 {
        let k = top_ranks(all_mask, &[pairs[0]], 3);
        return pack(Category::Pair, [pairs[0], k[0], k[1], k[2], 0]);
    }

    pack(Category::HighCard, top_ranks(all_mask, &[], 5))
}

pub fn rank_name(rank: u8) -> &'static str {
    match rank {
        2 => "Two",
        3 => "Three",
        4 => "Four",
        5 => "Five",
        6 => "Six",
        7 => "Seven",
        8 => "Eight",
        9 => "Nine",
        10 => "Ten",
        11 => "Jack",
        12 => "Queen",
        13 => "King",
        14 => "Ace",
        _ => "?",
    }
}

pub fn rank_plural(rank: u8) -> &'static str {
    match rank {
        2 => "Twos",
        3 => "Threes",
        4 => "Fours",
        5 => "Fives",
        6 => "Sixes",
        7 => "Sevens",
        8 => "Eights",
        9 => "Nines",
        10 => "Tens",
        11 => "Jacks",
        12 => "Queens",
        13 => "Kings",
        14 => "Aces",
        _ => "?",
    }
}

/// Human readable name of a score, e.g. "Full House, Kings full of Fours".
pub fn describe(score: Score) -> String {
    let k = tiebreakers(score);
    match category(score) {
        Category::HighCard => format!("High Card, {}", rank_name(k[0])),
        Category::Pair => format!("Pair of {}", rank_plural(k[0])),
        Category::TwoPair => format!("Two Pair, {} and {}", rank_plural(k[0]), rank_plural(k[1])),
        Category::ThreeOfAKind => format!("Three of a Kind, {}", rank_plural(k[0])),
        Category::Straight => format!("Straight, {} high", rank_name(k[0])),
        Category::Flush => format!("Flush, {} high", rank_name(k[0])),
        Category::FullHouse => format!(
            "Full House, {} full of {}",
            rank_plural(k[0]),
            rank_plural(k[1])
        ),
        Category::FourOfAKind => format!("Four of a Kind, {}", rank_plural(k[0])),
        Category::StraightFlush => {
            if k[0] == 14 {
                "Royal Flush".to_string()
            } else {
                format!("Straight Flush, {} high", rank_name(k[0]))
            }
        }
    }
}

/// Describes two hole cards, e.g. "Pocket Kings" or "Ace-King suited".
pub fn describe_hole(a: Card, b: Card) -> String {
    let (hi, lo) = if a.rank >= b.rank { (a, b) } else { (b, a) };
    if hi.rank == lo.rank {
        format!("Pocket {}", rank_plural(hi.rank))
    } else {
        format!(
            "{}-{} {}",
            rank_name(hi.rank),
            rank_name(lo.rank),
            if hi.suit == lo.suit {
                "suited"
            } else {
                "offsuit"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Deck, Rng, Suit, parse_cards};

    fn score(text: &str) -> Score {
        evaluate(&parse_cards(text).expect("valid cards"))
    }

    fn full_deck() -> Vec<Card> {
        let mut v = Vec::new();
        for &suit in &Suit::ALL {
            for rank in 2..=14u8 {
                v.push(Card::new(rank, suit));
            }
        }
        v
    }

    #[test]
    fn describes_every_category() {
        assert_eq!(describe(score("As Ks Qs Js Ts 2d 3c")), "Royal Flush");
        assert_eq!(
            describe(score("9h 8h 7h 6h 5h As Kd")),
            "Straight Flush, Nine high"
        );
        assert_eq!(
            describe(score("Ah 2h 3h 4h 5h 9c Kd")),
            "Straight Flush, Five high"
        );
        assert_eq!(
            describe(score("9s 9h 9d 9c Ah 2c 2d")),
            "Four of a Kind, Nines"
        );
        assert_eq!(
            describe(score("Ks Kh Kd 4c 4h 2c 2d")),
            "Full House, Kings full of Fours"
        );
        assert_eq!(
            describe(score("Ks Kh Kd 4c 4h 4d 2c")),
            "Full House, Kings full of Fours"
        );
        assert_eq!(
            describe(score("Ks Kh Kd Qc Qh 2c 2d")),
            "Full House, Kings full of Queens"
        );
        assert_eq!(describe(score("As 9s 7s 5s 3s Kd Kh")), "Flush, Ace high");
        assert_eq!(
            describe(score("Ts 9h 8d 7c 6s 2c 2d")),
            "Straight, Ten high"
        );
        assert_eq!(
            describe(score("As 2h 3d 4c 5s 9c Kd")),
            "Straight, Five high"
        );
        assert_eq!(
            describe(score("As Kh Qd Jc Ts 2c 3d")),
            "Straight, Ace high"
        );
        assert_eq!(
            describe(score("7s 7h 7d Kc 9h 4c 2d")),
            "Three of a Kind, Sevens"
        );
        assert_eq!(
            describe(score("As Ah Kd Kc Qs Qh 2c")),
            "Two Pair, Aces and Kings"
        );
        assert_eq!(describe(score("Ks Kh As Qd 9c 4s 2h")), "Pair of Kings");
        assert_eq!(describe(score("As Kh Qd 9c 7s 4s 2h")), "High Card, Ace");
    }

    #[test]
    fn flush_beats_straight_and_full_house_beats_flush() {
        // spades 9 8 7 6 2 make a flush; 9 8 7 6 5 is also a straight
        assert_eq!(category(score("9s 8s 7s 6s 2s 5d 4h")), Category::Flush);
        // full house on board with a flush draw complete: full house wins
        assert_eq!(category(score("Ks Kh Kd 4s 4h 9s 2s")), Category::FullHouse);
    }

    #[test]
    fn category_order_is_strictly_increasing() {
        let hands = [
            "As Kh Qd 9c 7s 4s 2h",
            "Ks Kh As Qd 9c 4s 2h",
            "As Ah Kd Kc Jh 4s 2h",
            "7s 7h 7d Kc 9h 4c 2d",
            "Ts 9h 8d 7c 6s 2c 2d",
            "As 9s 7s 5s 3s Kd Qh",
            "Ks Kh Kd 4c 4h 2c 3d",
            "9s 9h 9d 9c Ah 2c 3d",
            "9h 8h 7h 6h 5h As Kd",
            "As Ks Qs Js Ts 2d 3c",
        ];
        let scores: Vec<Score> = hands.iter().map(|h| score(h)).collect();
        for w in scores.windows(2) {
            assert!(w[0] < w[1], "{} !< {}", describe(w[0]), describe(w[1]));
        }
    }

    #[test]
    fn kickers_and_tie_breaks() {
        // Same pair, better kicker wins.
        assert!(score("Ks Kh As Qd 9c 4s 2h") > score("Ks Kh Js Qd 9c 4s 2h"));
        // Two pair: the third pair supplies the kicker.
        assert!(score("As Ah Kd Kc Qs Qh 2c") > score("As Ah Kd Kc Js 2h 3c"));
        // Wheel is the lowest straight; a six-high straight beats it.
        assert!(score("As 2h 3d 4c 5s 9c Kd") < score("2s 3h 4d 5c 6s 9c Kd"));
        // Ace-high straight beats king-high straight.
        assert!(score("As Kh Qd Jc Ts 2c 3d") > score("9s Kh Qd Jc Ts 2c 3d"));
        // Higher trips win the full house, then the pair.
        assert!(score("Ks Kh Kd 2c 2h 9c 3d") > score("Qs Qh Qd Ac Ah 9c 3d"));
        // Board plays: identical scores tie.
        assert_eq!(score("Ks Kh 2c 3d 4h 5s 6c"), score("Qs Qh 2c 3d 4h 5s 6c"));
        // Flush: compare the whole five card flush.
        assert!(score("As Ks 9s 5s 3s 2d 2h") > score("As Ks 9s 5s 2s 4d 4h"));
    }

    #[test]
    fn describe_hole_cards() {
        let c = |s: &str| Card::parse(s).unwrap();
        assert_eq!(describe_hole(c("Kh"), c("Ks")), "Pocket Kings");
        assert_eq!(describe_hole(c("Kh"), c("As")), "Ace-King offsuit");
        assert_eq!(describe_hole(c("Ts"), c("Js")), "Jack-Ten suited");
    }

    // ---- independent brute-force reference used to cross-check `evaluate` ----

    fn eval5(cards: &[Card]) -> Score {
        let mut ranks: Vec<u8> = cards.iter().map(|c| c.rank).collect();
        ranks.sort_by(|a, b| b.cmp(a));
        let flush = cards.iter().all(|c| c.suit == cards[0].suit);
        let mut uniq = ranks.clone();
        uniq.dedup();
        let straight = if uniq.len() == 5 {
            if uniq[0] - uniq[4] == 4 {
                Some(uniq[0])
            } else if uniq == vec![14, 5, 4, 3, 2] {
                Some(5)
            } else {
                None
            }
        } else {
            None
        };
        let mut groups: Vec<(usize, u8)> = uniq
            .iter()
            .map(|&r| (ranks.iter().filter(|&&x| x == r).count(), r))
            .collect();
        groups.sort_by(|a, b| b.cmp(a));
        let g = |i: usize| groups.get(i).map(|x| x.1).unwrap_or(0);
        let r = |i: usize| ranks.get(i).copied().unwrap_or(0);
        if let (Some(h), true) = (straight, flush) {
            return pack(Category::StraightFlush, [h, 0, 0, 0, 0]);
        }
        if groups[0].0 == 4 {
            return pack(Category::FourOfAKind, [g(0), g(1), 0, 0, 0]);
        }
        if groups[0].0 == 3 && groups[1].0 == 2 {
            return pack(Category::FullHouse, [g(0), g(1), 0, 0, 0]);
        }
        if flush {
            return pack(Category::Flush, [r(0), r(1), r(2), r(3), r(4)]);
        }
        if let Some(h) = straight {
            return pack(Category::Straight, [h, 0, 0, 0, 0]);
        }
        if groups[0].0 == 3 {
            return pack(Category::ThreeOfAKind, [g(0), g(1), g(2), 0, 0]);
        }
        if groups[0].0 == 2 && groups[1].0 == 2 {
            return pack(Category::TwoPair, [g(0), g(1), g(2), 0, 0]);
        }
        if groups[0].0 == 2 {
            return pack(Category::Pair, [g(0), g(1), g(2), g(3), 0]);
        }
        pack(Category::HighCard, [r(0), r(1), r(2), r(3), r(4)])
    }

    fn best_of_subsets(cards: &[Card]) -> Score {
        let n = cards.len();
        let mut best = 0;
        for mask in 0u32..(1 << n) {
            if mask.count_ones() != 5 {
                continue;
            }
            let subset: Vec<Card> = (0..n)
                .filter(|&i| mask & (1 << i) != 0)
                .map(|i| cards[i])
                .collect();
            best = best.max(eval5(&subset));
        }
        best
    }

    #[test]
    fn matches_brute_force_on_random_hands() {
        let mut rng = Rng::seeded(2024);
        for i in 0..20_000 {
            let mut deck = Deck::new();
            deck.shuffle(&mut rng);
            let n = [5usize, 6, 7, 7, 7][i % 5];
            let hand: Vec<Card> = (0..n).map(|_| deck.draw()).collect();
            assert_eq!(
                evaluate(&hand),
                best_of_subsets(&hand),
                "mismatch on {hand:?}"
            );
        }
    }

    /// Every one of the 2,598,960 five-card hands, compared with the known counts.
    #[test]
    fn five_card_category_counts() {
        let deck = full_deck();
        let mut counts = [0u64; 9];
        for a in 0..48 {
            for b in (a + 1)..49 {
                for c in (b + 1)..50 {
                    for d in (c + 1)..51 {
                        for e in (d + 1)..52 {
                            let hand = [deck[a], deck[b], deck[c], deck[d], deck[e]];
                            counts[(evaluate(&hand) >> 20) as usize] += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(
            counts,
            [
                1_302_540, 1_098_240, 123_552, 54_912, 10_200, 5_108, 3_744, 624, 40
            ]
        );
    }

    /// Every one of the 133,784,560 seven-card hands. Slow: run with
    /// `cargo test --release -- --ignored`.
    #[test]
    #[ignore]
    fn seven_card_category_counts() {
        let deck = full_deck();
        let mut counts = [0u64; 9];
        for a in 0..46 {
            for b in (a + 1)..47 {
                for c in (b + 1)..48 {
                    for d in (c + 1)..49 {
                        for e in (d + 1)..50 {
                            for f in (e + 1)..51 {
                                for g in (f + 1)..52 {
                                    let hand = [
                                        deck[a], deck[b], deck[c], deck[d], deck[e], deck[f],
                                        deck[g],
                                    ];
                                    counts[(evaluate(&hand) >> 20) as usize] += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(
            counts,
            [
                23_294_460, 58_627_800, 31_433_400, 6_461_620, 6_180_020, 4_047_644, 3_473_184,
                224_848, 41_584
            ]
        );
    }
}
