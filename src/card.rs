//! Cards, the deck, and a small self-contained random number generator.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn symbol(self) -> char {
        match self {
            Suit::Clubs => '♣',
            Suit::Diamonds => '♦',
            Suit::Hearts => '♥',
            Suit::Spades => '♠',
        }
    }

    pub fn is_red(self) -> bool {
        matches!(self, Suit::Diamonds | Suit::Hearts)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub struct Card {
    /// 2..=14, where 11 = Jack, 12 = Queen, 13 = King and 14 = Ace.
    pub rank: u8,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: u8, suit: Suit) -> Card {
        debug_assert!((2..=14).contains(&rank));
        Card { rank, suit }
    }

    pub fn rank_symbol(self) -> &'static str {
        match self.rank {
            2 => "2",
            3 => "3",
            4 => "4",
            5 => "5",
            6 => "6",
            7 => "7",
            8 => "8",
            9 => "9",
            10 => "10",
            11 => "J",
            12 => "Q",
            13 => "K",
            14 => "A",
            _ => "?",
        }
    }

    /// A unique index in `0..52`.
    pub fn index(self) -> usize {
        (self.rank as usize - 2) * 4 + self.suit.index()
    }

    /// Parses short notation such as `As`, `Td`, `10h` or `2c`.
    pub fn parse(text: &str) -> Option<Card> {
        let chars: Vec<char> = text.trim().chars().collect();
        let (rank_chars, suit_char) = match chars.len() {
            2 => (&chars[..1], chars[1]),
            3 if chars[0] == '1' && chars[1] == '0' => (&chars[..2], chars[2]),
            _ => return None,
        };
        let rank_text: String = rank_chars.iter().collect();
        let rank = match rank_text.to_ascii_uppercase().as_str() {
            "2" => 2,
            "3" => 3,
            "4" => 4,
            "5" => 5,
            "6" => 6,
            "7" => 7,
            "8" => 8,
            "9" => 9,
            "10" | "T" => 10,
            "J" => 11,
            "Q" => 12,
            "K" => 13,
            "A" => 14,
            _ => return None,
        };
        let suit = match suit_char.to_ascii_lowercase() {
            'c' => Suit::Clubs,
            'd' => Suit::Diamonds,
            'h' => Suit::Hearts,
            's' => Suit::Spades,
            _ => return None,
        };
        Some(Card::new(rank, suit))
    }
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.rank_symbol(), self.suit.symbol())
    }
}

/// Parses a whitespace separated list of cards, e.g. `"As Kd 9h"`.
pub fn parse_cards(text: &str) -> Option<Vec<Card>> {
    text.split_whitespace().map(Card::parse).collect()
}

/// Space separated card list for log messages, e.g. `A♠ K♦ 9♥`.
pub fn cards_text(cards: &[Card]) -> String {
    let parts: Vec<String> = cards.iter().map(std::string::ToString::to_string).collect();
    parts.join(" ")
}

/// SplitMix64: tiny, fast and statistically solid for a card game.
/// No external crates are required.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn seeded(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// Seeds from operating-system randomness (via `RandomState`) and the clock.
    pub fn from_entropy() -> Rng {
        let mut hasher = RandomState::new().build_hasher();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        hasher.write_u64(nanos);
        Rng::seeded(hasher.finish())
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..n` (unbiased, by rejection sampling). `n` must be > 0.
    pub fn below(&mut self, n: u64) -> u64 {
        assert!(n > 0, "Rng::below(0)");
        let limit = u64::MAX - u64::MAX % n;
        loop {
            let x = self.next_u64();
            if x < limit {
                return x % n;
            }
        }
    }

    /// Uniform integer in `lo..=hi`.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below(u64::from(hi - lo) + 1) as u32
    }

    /// Uniform float in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / 16_777_216.0
    }

    pub fn chance(&mut self, probability: f32) -> bool {
        self.unit() < probability
    }
}

/// A 52 card deck. Cards are drawn from the end of the vector.
#[derive(Clone, Debug)]
pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    pub fn new() -> Deck {
        let mut cards = Vec::with_capacity(52);
        for &suit in &Suit::ALL {
            for rank in 2..=14u8 {
                cards.push(Card::new(rank, suit));
            }
        }
        Deck { cards }
    }

    /// Fisher–Yates shuffle.
    pub fn shuffle(&mut self, rng: &mut Rng) {
        for i in (1..self.cards.len()).rev() {
            let j = rng.below(i as u64 + 1) as usize;
            self.cards.swap(i, j);
        }
    }

    pub fn draw(&mut self) -> Card {
        self.cards.pop().expect("the deck is empty")
    }

    pub fn remaining(&self) -> usize {
        self.cards.len()
    }

    /// A deck whose first draws are exactly `top`, in order (used by tests and
    /// scripted scenarios). `top` must not contain duplicate cards.
    pub fn stacked(top: &[Card]) -> Deck {
        let mut cards: Vec<Card> = Deck::new()
            .cards
            .into_iter()
            .filter(|c| !top.contains(c))
            .collect();
        cards.extend(top.iter().rev().copied());
        Deck { cards }
    }
}

impl Default for Deck {
    fn default() -> Self {
        Deck::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_display() {
        let c = Card::parse("As").unwrap();
        assert_eq!((c.rank, c.suit), (14, Suit::Spades));
        assert_eq!(Card::parse("10h").unwrap(), Card::parse("Th").unwrap());
        assert_eq!(Card::parse("2c").unwrap().to_string(), "2♣");
        assert!(Card::parse("1s").is_none());
        assert!(Card::parse("Ax").is_none());
        assert!(Card::parse("é").is_none());
        assert_eq!(parse_cards("As Kd 9h").unwrap().len(), 3);
        assert!(parse_cards("As Zz").is_none());
    }

    #[test]
    fn card_index_is_a_bijection() {
        let mut seen = [false; 52];
        let mut deck = Deck::new();
        for _ in 0..52 {
            let c = deck.draw();
            assert!(!seen[c.index()]);
            seen[c.index()] = true;
        }
        assert!(seen.iter().all(|&s| s));
        assert_eq!(deck.remaining(), 0);
    }

    #[test]
    fn shuffle_keeps_all_cards_and_is_not_identity() {
        let mut rng = Rng::seeded(42);
        let mut deck = Deck::new();
        deck.shuffle(&mut rng);
        let mut drawn: Vec<usize> = (0..52).map(|_| deck.draw().index()).collect();
        let shuffled = drawn.clone();
        drawn.sort_unstable();
        assert_eq!(drawn, (0..52).collect::<Vec<usize>>());
        assert_ne!(shuffled, drawn, "a shuffled deck should not be sorted");
    }

    #[test]
    fn stacked_deck_draws_in_order() {
        let top = parse_cards("As Kd 9h 2c").unwrap();
        let mut deck = Deck::stacked(&top);
        assert_eq!(deck.remaining(), 52);
        for c in top {
            assert_eq!(deck.draw(), c);
        }
    }

    #[test]
    fn rng_is_roughly_uniform() {
        let mut rng = Rng::seeded(7);
        let mut buckets = [0u32; 10];
        for _ in 0..100_000 {
            buckets[rng.below(10) as usize] += 1;
        }
        for &b in &buckets {
            assert!((9_000..11_000).contains(&b), "bucket count {b}");
        }
        for _ in 0..1000 {
            let u = rng.unit();
            assert!((0.0..1.0).contains(&u));
            let r = rng.range(5, 8);
            assert!((5..=8).contains(&r));
        }
    }
}
