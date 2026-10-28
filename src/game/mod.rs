//! The rules of no-limit Texas Hold'em: blinds, antes, straddles, betting
//! rounds, raise sizing, all-ins, side pots, showdowns (with optional mucking),
//! rebuys, players joining and leaving, and tournament standings.
//!
//! `Game` is a plain state machine with no I/O and no notion of time. A driver
//! (the table server in `table.rs`, or a test) repeatedly
//!
//! * calls [`Game::start_hand`] while the state is `HandOver`,
//! * calls [`Game::act`] (or [`Game::bot_move`]) while the state is `Betting`,
//! * calls [`Game::runout_step`] while the state is `Runout`,
//!
//! and may seat or remove players, or process rebuys, between hands.
//!
//! Chip conservation is an invariant: chips in stacks plus chips in the pot
//! always equal [`Game::total_chips`], which only changes when players join,
//! leave or rebuy.
//!
//! This module holds the types and `Game` itself; its methods are split by
//! stage: `seats` (joining, leaving, rebuys), `hand` (starting a hand),
//! `betting` (the betting rounds), `showdown` (settling the pots) and `queries`
//! (questions that change nothing). `rules` has the table rules and `pots` the
//! side-pot arithmetic.

use serde::{Deserialize, Serialize};

use crate::bot::{self, Personality};
use crate::card::{Card, Deck, Rng};
use crate::eval::Score;
use crate::stats::Stats;

mod betting;
mod hand;
mod pots;
mod queries;
mod rules;
mod seats;
mod showdown;

pub use pots::{Award, Pot, award_pots, compute_pots};
pub use rules::{DEFAULT_BLIND_SCHEDULE, Rules};

/// Seat of the human player in games made with [`Game::new`].
pub const HUMAN: usize = 0;
pub const MIN_PLAYERS: usize = 2;
pub const MAX_PLAYERS: usize = 9;
const LOG_LIMIT: usize = 400;
/// Default number of Monte Carlo samples per bot decision.
pub const DEFAULT_BOT_SAMPLES: u32 = 220;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Street {
    #[default]
    PreFlop,
    Flop,
    Turn,
    River,
}

impl Street {
    pub fn name(self) -> &'static str {
        match self {
            Street::PreFlop => "Pre-flop",
            Street::Flop => "Flop",
            Street::Turn => "Turn",
            Street::River => "River",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum State {
    /// Waiting for `to_act` to make a decision.
    Betting,
    /// Betting is over because everyone left is all-in; the board is being dealt out.
    Runout,
    /// The hand has been settled (or none was dealt yet); waiting for the next one.
    #[default]
    HandOver,
    /// The tournament has ended.
    GameOver,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Action {
    Fold,
    Check,
    /// Match the current bet (or go all-in for less). Treated as a check when nothing is owed.
    Call,
    /// Raise (or bet) so that the player's total bet this round becomes the given amount.
    RaiseTo(u32),
    /// Push the whole stack. Only legal when that is a raise or a call for the whole stack.
    AllIn,
}

/// Something that happened at the table, for front ends that want to react
/// (for example with sound). Collected with [`Game::take_events`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum GameEvent {
    /// Blinds posted and hole cards dealt.
    HandStarted,
    Fold(usize),
    Check(usize),
    Call(usize),
    /// A bet or a raise that is not all-in.
    Bet(usize),
    /// Any action that put the player's last chip in.
    AllIn(usize),
    /// The flop, turn or river was dealt.
    Street(Street),
    /// The pot has been paid out.
    HandSettled,
    Eliminated(usize),
    Rebuy(usize),
    Joined(usize),
    Left(usize),
    TournamentOver,
}

/// What the player to act may do right now.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Legal {
    /// Chips needed to call (capped at the player's stack).
    pub to_call: u32,
    pub can_check: bool,
    pub can_raise: bool,
    /// Smallest legal total bet when raising (the all-in amount if the stack is shorter).
    pub min_to: u32,
    /// Largest legal total bet: everything the player has.
    pub max_to: u32,
    pub stack: u32,
}

impl Legal {
    /// Whether going all-in is allowed: as a raise, or as a call of a bet that
    /// takes the whole stack.
    pub fn can_go_all_in(&self) -> bool {
        self.can_raise || self.calls_all_in()
    }

    /// Calling puts the whole stack in.
    pub fn calls_all_in(&self) -> bool {
        !self.can_check && self.to_call == self.stack
    }
}

#[derive(Clone, Debug)]
pub struct Player {
    pub name: String,
    pub is_human: bool,
    pub personality: Personality,
    pub chips: u32,
    pub hole: Vec<Card>,
    /// Chips put in during the current betting round (antes not included).
    pub bet: u32,
    /// Chips put in during the whole hand.
    pub total: u32,
    pub in_hand: bool,
    pub folded: bool,
    pub all_in: bool,
    /// Has acted since the last full raise (used to decide who still owes an action).
    pub acted: bool,
    /// Not taking part: eliminated, gone, or an empty seat.
    pub out: bool,
    /// Nobody sits here (the seat was never filled, or its player left).
    pub vacant: bool,
    pub start_chips: u32,
    /// Chips won in the last settled hand.
    pub payout: u32,
    pub score: Option<Score>,
    /// The hole cards have been turned face up for everyone this hand.
    pub shown: bool,
    /// Short text for the table view ("Raise 120", "Fold", ...).
    pub last_action: String,
    /// Busted, but may still rebuy before the next hand.
    pub rebuy_pending: bool,
    pub rebuys: u32,
    /// Elimination order (higher = knocked out later = better place).
    pub eliminated: Option<u32>,
    pub stats: Stats,
    /// This hand counts for the statistics (the player was not short-stacked).
    tracked: bool,
    played_this_hand: bool,
    raised_this_hand: bool,
}

impl Player {
    fn new(name: String, is_human: bool, personality: Personality, chips: u32) -> Player {
        Player {
            name,
            is_human,
            personality,
            chips,
            hole: Vec::new(),
            bet: 0,
            total: 0,
            in_hand: false,
            folded: false,
            all_in: false,
            acted: false,
            out: false,
            vacant: false,
            start_chips: chips,
            payout: 0,
            score: None,
            shown: false,
            last_action: String::new(),
            rebuy_pending: false,
            rebuys: 0,
            eliminated: None,
            stats: Stats::default(),
            tracked: false,
            played_this_hand: false,
            raised_this_hand: false,
        }
    }

    fn empty_seat() -> Player {
        let mut p = Player::new(String::new(), false, Personality::default(), 0);
        p.out = true;
        p.vacant = true;
        p
    }

    /// Still contesting the pot.
    pub fn live(&self) -> bool {
        self.in_hand && !self.folded
    }

    /// Contesting the pot and able to bet more.
    pub fn can_act(&self) -> bool {
        self.live() && !self.all_in
    }

    /// Still in the tournament (has chips, or may rebuy).
    pub fn contending(&self) -> bool {
        !self.out && (self.chips > 0 || self.rebuy_pending)
    }
}

/// A player who is no longer at the table, for the final standings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finish {
    pub name: String,
    pub is_human: bool,
    pub style: String,
    pub stats: Stats,
    /// Chips taken away (players who left) or 0 (eliminated players).
    pub chips: u32,
    /// Left the table instead of being knocked out.
    pub left: bool,
    /// Elimination order; only meaningful when `left` is false.
    pub order: u32,
    pub hand: u32,
}

/// One row of the standings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Standing {
    /// None for players who left the table.
    pub place: Option<usize>,
    pub name: String,
    pub is_human: bool,
    pub style: String,
    pub chips: u32,
    /// "1,250 chips", "eliminated in hand 31", "left in hand 12 with 900"...
    pub status: String,
    pub stats: Stats,
}

/// 1 -> "1st", 2 -> "2nd", 11 -> "11th", ...
pub fn ordinal(n: usize) -> String {
    let suffix = match (n % 100, n % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// 1234567 -> "1,234,567".
pub fn chips_text(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[derive(Clone, Debug)]
pub struct Game {
    pub rules: Rules,
    pub rng: Rng,
    pub players: Vec<Player>,
    pub button: usize,
    /// Small blind position; no small blind is posted if `sb_dead`.
    pub sb_seat: usize,
    pub sb_dead: bool,
    pub bb_seat: usize,
    pub straddle_seat: Option<usize>,
    pub hand_no: u32,
    /// Blind level, starting at 1.
    pub level: u32,
    pub sb: u32,
    pub bb: u32,
    pub ante: u32,
    pub state: State,
    pub street: Street,
    pub board: Vec<Card>,
    deck: Deck,
    /// The largest bet any player has made this round.
    pub current_bet: u32,
    /// Minimum size of a raise increment this round.
    pub min_raise: u32,
    pub to_act: usize,
    last_actor: usize,
    /// The last player to bet or raise on the river (shows first at a showdown).
    river_aggressor: Option<usize>,
    /// The newest log lines (older ones are dropped).
    pub log: Vec<String>,
    /// Number of log lines dropped so far, so lines can be numbered.
    pub log_dropped: u64,
    /// Summary lines for the hand that was just settled.
    pub result_lines: Vec<String>,
    pub awards: Vec<Award>,
    /// Uncalled chips handed back at settlement: (seat, amount).
    pub refund: Option<(usize, u32)>,
    /// True when the last hand went to a showdown.
    pub showdown: bool,
    /// True when an all-in run-out revealed every live hand.
    pub reveal_all: bool,
    /// Pot size of the last settled hand, after uncalled chips were returned.
    pub last_pot: u32,
    /// What each seat put in during the last settled hand, before any refund.
    pub last_totals: Vec<u32>,
    /// Monte Carlo samples each bot uses per decision (more is smarter but slower).
    pub bot_samples: u32,
    /// Number of successful actions over the whole game (never decreases).
    pub action_count: u64,
    /// Players who were knocked out or left, in the order it happened.
    pub history: Vec<Finish>,
    /// End the game once no human player is left in it (true by default).
    pub end_when_humans_out: bool,
    /// Hands started with fewer big blinds than this do not count for the statistics.
    pub stats_min_stack_bb: u32,
    eliminations: u32,
    events: Vec<GameEvent>,
    total_chips: u64,
}

impl Game {
    /// A game with the human in seat 0 ("You") and bots in the other seats.
    pub fn new(rules: Rules, seats: usize, mut rng: Rng) -> Game {
        let n = seats.clamp(MIN_PLAYERS, MAX_PLAYERS);
        let mut game = Game::empty(rules, n, rng.clone());
        let chips = game.rules.starting_chips;
        game.seat_player(0, "You", true, Personality::default(), chips)
            .expect("seat 0 is free");
        for (i, (name, personality)) in bot::roster(n - 1, &mut rng).into_iter().enumerate() {
            game.seat_player(i + 1, &name, false, personality, chips)
                .expect("the seat is free");
        }
        game.rng = rng;
        game.events.clear();
        game.log.clear();
        game
    }

    /// A table with `seats` empty seats. Fill it with [`Game::seat_player`].
    pub fn empty(mut rules: Rules, seats: usize, mut rng: Rng) -> Game {
        let n = seats.clamp(MIN_PLAYERS, MAX_PLAYERS);
        rules.starting_chips = rules.starting_chips.max(1);
        rules.small_blind = rules.small_blind.max(1);
        let sb = rules.small_blind;
        let button = rng.below(n as u64) as usize;
        Game {
            rules,
            rng,
            players: (0..n).map(|_| Player::empty_seat()).collect(),
            button,
            sb_seat: 0,
            sb_dead: false,
            bb_seat: 0,
            straddle_seat: None,
            hand_no: 0,
            level: 1,
            sb,
            bb: sb * 2,
            ante: 0,
            state: State::HandOver,
            street: Street::PreFlop,
            board: Vec::new(),
            deck: Deck::new(),
            current_bet: 0,
            min_raise: sb * 2,
            to_act: 0,
            last_actor: 0,
            river_aggressor: None,
            log: Vec::new(),
            log_dropped: 0,
            result_lines: Vec::new(),
            awards: Vec::new(),
            refund: None,
            showdown: false,
            reveal_all: false,
            last_pot: 0,
            last_totals: Vec::new(),
            bot_samples: DEFAULT_BOT_SAMPLES,
            action_count: 0,
            history: Vec::new(),
            end_when_humans_out: true,
            stats_min_stack_bb: crate::stats::MIN_STACK_BB,
            eliminations: 0,
            events: Vec::new(),
            total_chips: 0,
        }
    }

    /// Overrides every stack and the button. Meant for scripted scenarios and
    /// tests; call it before the first hand is dealt.
    pub fn setup(&mut self, stacks: &[u32], button: usize) {
        assert_eq!(stacks.len(), self.players.len(), "one stack per seat");
        for (p, &s) in self.players.iter_mut().zip(stacks) {
            p.chips = s;
            p.start_chips = s;
        }
        self.total_chips = stacks.iter().map(|&s| u64::from(s)).sum();
        self.button = button % self.players.len();
    }

    fn log(&mut self, text: String) {
        self.log.push(text);
        if self.log.len() > LOG_LIMIT {
            let extra = self.log.len() - LOG_LIMIT;
            self.log.drain(0..extra);
            self.log_dropped += extra as u64;
        }
    }

    fn say(&mut self, seat: usize, short: String, long: String) {
        self.players[seat].last_action = short;
        self.log(long);
    }

    /// Next seat after `from` whose player is taking part.
    fn next_alive(&self, from: usize) -> usize {
        let n = self.players.len();
        let mut i = from;
        for _ in 0..n {
            i = (i + 1) % n;
            if !self.players[i].out {
                return i;
            }
        }
        from
    }

    /// Moves up to `amount` chips from a stack into the current bet; returns what moved.
    fn commit(&mut self, seat: usize, amount: u32) -> u32 {
        let p = &mut self.players[seat];
        let amount = amount.min(p.chips);
        p.chips -= amount;
        p.bet += amount;
        p.total += amount;
        if p.chips == 0 {
            p.all_in = true;
        }
        amount
    }

    fn between_hands(&self) -> bool {
        matches!(self.state, State::HandOver | State::GameOver)
    }

    fn finish_record(&self, seat: usize, left: bool, order: u32) -> Finish {
        let p = &self.players[seat];
        Finish {
            name: p.name.clone(),
            is_human: p.is_human,
            style: p.personality.style.to_string(),
            stats: p.stats,
            chips: if left { p.chips } else { 0 },
            left,
            order,
            hand: self.hand_no,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_and_chip_text() {
        assert_eq!(ordinal(1), "1st");
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(ordinal(21), "21st");
        assert_eq!(chips_text(0), "0");
        assert_eq!(chips_text(999), "999");
        assert_eq!(chips_text(1000), "1,000");
        assert_eq!(chips_text(1234567), "1,234,567");
    }

    #[test]
    fn log_numbering_survives_trimming() {
        let mut g = Game::new(Rules::default(), 3, Rng::seeded(1));
        for i in 0..(LOG_LIMIT + 50) {
            g.log(format!("line {i}"));
        }
        let (lines, next) = g.log_since(0);
        assert_eq!(lines.len(), LOG_LIMIT);
        assert_eq!(next, (LOG_LIMIT + 50) as u64);
        assert_eq!(lines[0], "line 50");
        let (lines, _) = g.log_since(next - 2);
        assert_eq!(lines, &["line 448".to_string(), "line 449".to_string()]);
        assert!(g.log_since(next).0.is_empty());
    }
}
