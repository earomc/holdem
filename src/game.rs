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

use serde::{Deserialize, Serialize};

use crate::bot::{self, Personality};
use crate::card::{Card, Deck, Rng, cards_text};
use crate::eval::{self, Category, Score};
use crate::stats::Stats;

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

/// The table rules. Everything optional is off by default.
///
/// Rules travel between server and clients, so unknown fields are ignored and
/// missing ones get their defaults: a slightly newer or older peer still
/// understands the rules it knows about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub starting_chips: u32,
    pub small_blind: u32,
    /// Hands played at each blind level; 0 keeps the blinds fixed.
    pub hands_per_level: u32,
    /// The small blind at each level, in percent of the starting small blind.
    pub blind_schedule: Vec<u32>,
    /// After the last level of the schedule, each level is this many percent of the previous one.
    pub blind_growth_percent: u32,
    /// The game ends after this many hands (the biggest stack wins); 0 = no limit.
    pub hand_limit: u32,
    /// Every player posts an ante each hand.
    pub antes: bool,
    /// The ante is the big blind divided by this (at least 1 chip).
    pub ante_divisor: u32,
    /// The player left of the big blind posts a straddle and acts last before the flop.
    pub straddle: bool,
    /// The straddle, in big blinds.
    pub straddle_multiplier: u32,
    /// Fewest players at the table for a straddle to be posted.
    pub straddle_min_players: usize,
    /// A busted player may buy back in while the blind level is at most this; 0 = no rebuys.
    pub rebuy_levels: u32,
    /// Chips a rebuy gives; 0 = the starting stack.
    pub rebuy_chips: u32,
    /// Dead-button rule: the big blind always moves to the next player, so the
    /// small blind or the button can fall on an empty seat.
    pub dead_button: bool,
    /// At a showdown, players who cannot win may throw their cards away unseen.
    pub muck_losers: bool,
}

/// The default blind schedule: level 1 is the starting small blind (100%).
pub const DEFAULT_BLIND_SCHEDULE: [u32; 12] = [
    100, 150, 250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000, 6000,
];

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            starting_chips: 1500,
            small_blind: 10,
            hands_per_level: 8,
            blind_schedule: DEFAULT_BLIND_SCHEDULE.to_vec(),
            blind_growth_percent: 150,
            hand_limit: 0,
            antes: false,
            ante_divisor: 8,
            straddle: false,
            straddle_multiplier: 2,
            straddle_min_players: 4,
            rebuy_levels: 0,
            rebuy_chips: 0,
            dead_button: false,
            muck_losers: false,
        }
    }
}

impl Rules {
    /// The ante for a given big blind (big blind / divisor, at least 1).
    pub fn ante_for(&self, bb: u32) -> u32 {
        if self.antes {
            (bb / self.ante_divisor.max(1)).max(1)
        } else {
            0
        }
    }

    /// Chips a rebuy gives.
    pub fn rebuy_stack(&self) -> u32 {
        if self.rebuy_chips > 0 {
            self.rebuy_chips
        } else {
            self.starting_chips
        }
    }

    /// The small blind at a level (1 = first), following the schedule and then
    /// growing by `blind_growth_percent` per level.
    pub fn small_blind_at(&self, level: u32) -> u32 {
        let base = self.small_blind.max(1) as u64;
        let schedule: &[u32] = if self.blind_schedule.is_empty() {
            &[100]
        } else {
            &self.blind_schedule
        };
        let i = level.saturating_sub(1) as usize;
        let mut sb = if i < schedule.len() {
            base * schedule[i] as u64 / 100
        } else {
            let mut v = base * *schedule.last().unwrap_or(&100) as u64 / 100;
            let growth = self.blind_growth_percent.max(100) as u64;
            for _ in schedule.len()..=i {
                v = v * growth / 100;
                if v > (u32::MAX / 8) as u64 {
                    break;
                }
            }
            v
        };
        sb = sb.clamp(1, (u32::MAX / 8) as u64);
        sb as u32
    }

    // The rules in plain words, shared by the setup screen and `holdem server`.

    /// "10/20": the small and big blind at the start.
    pub fn blinds_text(&self) -> String {
        format!("{}/{}", self.small_blind, self.small_blind * 2)
    }

    /// The smallest and largest step from one blind level to the next, in
    /// percent (e.g. 20 and 100), over the schedule and the growth after it.
    pub fn level_steps(&self) -> (u32, u32) {
        let mut steps: Vec<u32> = self
            .blind_schedule
            .windows(2)
            .map(|w| (w[1] * 100 / w[0].max(1)).saturating_sub(100))
            .collect();
        steps.push(self.blind_growth_percent.saturating_sub(100));
        let low = steps.iter().copied().min().unwrap_or(0);
        let high = steps.iter().copied().max().unwrap_or(0);
        (low, high)
    }

    /// "every 8 hands" or "never".
    pub fn levels_text(&self) -> String {
        match self.hands_per_level {
            0 => "never".to_string(),
            1 => "every hand".to_string(),
            n => format!("every {} hands", n),
        }
    }

    /// "50 hands" or "no limit".
    pub fn hand_limit_text(&self) -> String {
        match self.hand_limit {
            0 => "no limit".to_string(),
            n => format!("{} hands", n),
        }
    }

    /// The ante's size in words: "1/8 of the big blind".
    pub fn ante_size_text(&self) -> String {
        format!("1/{} of the big blind", self.ante_divisor)
    }

    /// "on: 1/8 of the big blind" or "off".
    pub fn antes_text(&self) -> String {
        if self.antes {
            format!("on: {}", self.ante_size_text())
        } else {
            "off".to_string()
        }
    }

    /// The straddle's terms in words: "2 big blinds, with 4 or more players".
    pub fn straddle_size_text(&self) -> String {
        format!(
            "{} big blinds, with {} or more players",
            self.straddle_multiplier, self.straddle_min_players
        )
    }

    /// "on: 2 big blinds, with 4 or more players" or "off".
    pub fn straddle_text(&self) -> String {
        if self.straddle {
            format!("on: {}", self.straddle_size_text())
        } else {
            "off".to_string()
        }
    }

    /// "during the first 2 levels, for 1,500 chips" or "off".
    pub fn rebuys_text(&self) -> String {
        let when = match self.rebuy_levels {
            0 => return "off".to_string(),
            1 => "during level 1".to_string(),
            n => format!("during the first {} levels", n),
        };
        format!("{}, for {} chips", when, chips_text(self.rebuy_stack()))
    }

    /// "dead button" or "moving button".
    pub fn button_text(&self) -> &'static str {
        if self.dead_button {
            "dead button"
        } else {
            "moving button"
        }
    }

    /// "losers may muck" or "everyone shows".
    pub fn showdown_text(&self) -> &'static str {
        if self.muck_losers {
            "losers may muck"
        } else {
            "everyone shows"
        }
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

/// One pot (main or side) and the players who can win it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pot {
    pub amount: u32,
    pub eligible: Vec<usize>,
}

/// The outcome of one pot: winners (in odd-chip order) and their shares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Award {
    pub amount: u32,
    pub winners: Vec<usize>,
    pub shares: Vec<u32>,
}

/// Splits everything committed into a main pot and side pots.
///
/// `totals[i]` is what seat `i` put in over the hand; `live[i]` says whether the
/// seat can still win. Chips from folded players stay in the pots they paid into.
/// Every returned pot has at least one eligible player as long as one seat is live.
pub fn compute_pots(totals: &[u32], live: &[bool]) -> Vec<Pot> {
    let n = totals.len();
    let mut levels: Vec<u32> = (0..n)
        .filter(|&i| live[i] && totals[i] > 0)
        .map(|i| totals[i])
        .collect();
    levels.sort_unstable();
    levels.dedup();

    let mut pots: Vec<Pot> = Vec::new();
    let mut prev = 0u32;
    for &level in &levels {
        let amount: u32 = totals.iter().map(|&t| t.min(level) - t.min(prev)).sum();
        let eligible: Vec<usize> = (0..n).filter(|&i| live[i] && totals[i] >= level).collect();
        if amount > 0 {
            pots.push(Pot { amount, eligible });
        }
        prev = level;
    }

    // Dead money above the highest live level joins the last pot.
    let assigned: u32 = pots.iter().map(|p| p.amount).sum();
    let total: u32 = totals.iter().sum();
    if total > assigned {
        if let Some(last) = pots.last_mut() {
            last.amount += total - assigned;
        } else {
            let eligible: Vec<usize> = (0..n).filter(|&i| live[i]).collect();
            if !eligible.is_empty() {
                pots.push(Pot {
                    amount: total,
                    eligible,
                });
            }
        }
    }
    pots
}

/// Decides each pot. `scores[i]` is `None` for seats without a hand (folded or
/// never dealt). Ties split the pot; any odd chips go one at a time to the
/// winners in clockwise order starting left of the button.
pub fn award_pots(pots: &[Pot], scores: &[Option<Score>], button: usize) -> Vec<Award> {
    let n = scores.len();
    let first = (button + 1) % n;
    let mut awards = Vec::with_capacity(pots.len());
    for pot in pots {
        let best = pot.eligible.iter().filter_map(|&i| scores[i]).max();
        let mut winners: Vec<usize> = match best {
            Some(b) => pot
                .eligible
                .iter()
                .copied()
                .filter(|&i| scores[i] == Some(b))
                .collect(),
            // Nobody showed a hand (everyone else folded): the eligible player(s) take it.
            None => pot.eligible.clone(),
        };
        winners.sort_by_key(|&i| (i + n - first) % n);
        let k = winners.len() as u32;
        let base = pot.amount / k;
        let rem = pot.amount % k;
        let shares: Vec<u32> = (0..k).map(|j| base + if j < rem { 1 } else { 0 }).collect();
        awards.push(Award {
            amount: pot.amount,
            winners,
            shares,
        });
    }
    awards
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
    format!("{}{}", n, suffix)
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
        self.total_chips = stacks.iter().map(|&s| s as u64).sum();
        self.button = button % self.players.len();
    }

    // ------------------------------------------------------------------ queries

    /// Returns and clears the events that happened since the last call.
    /// Events from a hand are discarded when the next hand starts.
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// Log lines numbered from `seq` on (lines that were dropped are skipped),
    /// and the number to ask for next time.
    pub fn log_since(&self, seq: u64) -> (&[String], u64) {
        let first = self.log_dropped;
        let end = first + self.log.len() as u64;
        let from = seq.clamp(first, end);
        (&self.log[(from - first) as usize..], end)
    }

    /// Chips committed to the pot in the hand being played.
    pub fn pot(&self) -> u32 {
        self.players.iter().map(|p| p.total).sum()
    }

    /// Main pot and side pots as they stand now (amounts only).
    ///
    /// While a round is being bet, players who have not matched the bet yet
    /// still can, so a pot only ends where a live player is all-in: the
    /// separate pots are the ones all-ins have closed, plus the one still open.
    /// (At the showdown [`compute_pots`] splits what everyone finally put in.)
    pub fn pots_now(&self) -> Vec<u32> {
        let mut levels: Vec<u32> = self
            .players
            .iter()
            .filter(|p| p.live() && p.all_in && p.total > 0)
            .map(|p| p.total)
            .collect();
        levels.sort_unstable();
        levels.dedup();
        let mut pots = Vec::new();
        let mut previous = 0;
        for level in levels {
            let amount: u32 = self
                .players
                .iter()
                .map(|p| p.total.min(level) - p.total.min(previous))
                .sum();
            if amount > 0 {
                pots.push(amount);
            }
            previous = level;
        }
        let open = self.pot() - pots.iter().sum::<u32>();
        if open > 0 || pots.is_empty() {
            pots.push(open);
        }
        pots
    }

    pub fn live_count(&self) -> usize {
        self.players.iter().filter(|p| p.live()).count()
    }

    /// Players taking part (with chips, not eliminated or gone).
    pub fn alive_count(&self) -> usize {
        self.players.iter().filter(|p| !p.out).count()
    }

    /// Players still in the tournament, including busted players who may rebuy.
    pub fn contenders(&self) -> usize {
        self.players.iter().filter(|p| p.contending()).count()
    }

    /// The chips that should be in play: stacks plus the pot.
    pub fn total_chips(&self) -> u64 {
        self.total_chips
    }

    /// Chips in stacks plus chips in the pot; always equals `total_chips`.
    pub fn chips_in_play(&self) -> u64 {
        self.players
            .iter()
            .map(|p| (p.chips + p.total) as u64)
            .sum()
    }

    /// The seat whose decision is awaited.
    pub fn current_actor(&self) -> Option<usize> {
        if self.state == State::Betting {
            Some(self.to_act)
        } else {
            None
        }
    }

    /// Identifies the decision currently awaited. It changes with every action
    /// and every new hand, so an answer meant for an earlier decision (a key
    /// pressed twice, a message that crossed a new deal) can be recognised.
    pub fn turn_id(&self) -> u64 {
        ((self.hand_no as u64) << 32) | (self.action_count & 0xFFFF_FFFF)
    }

    pub fn is_human_turn(&self) -> bool {
        self.state == State::Betting && self.players[self.to_act].is_human
    }

    pub fn hand_limit_reached(&self) -> bool {
        self.rules.hand_limit > 0 && self.hand_no >= self.rules.hand_limit
    }

    /// True once the next `start_hand` would end the game.
    pub fn tournament_over(&self) -> bool {
        let humans_left = self.players.iter().any(|p| p.is_human && p.contending());
        self.contenders() <= 1
            || self.hand_limit_reached()
            || (self.end_when_humans_out && !humans_left)
    }

    /// Whether busted players may buy back in at the current level.
    pub fn rebuy_open(&self) -> bool {
        self.rules.rebuy_levels > 0 && self.level <= self.rules.rebuy_levels
    }

    /// The forced bet everyone must call before the flop: the big blind, or the
    /// straddle when there is one.
    pub fn blind_bet(&self) -> u32 {
        if self.straddle_seat.is_some() {
            self.bb * self.rules.straddle_multiplier.max(2)
        } else {
            self.bb
        }
    }

    pub fn legal(&self, seat: usize) -> Legal {
        let p = &self.players[seat];
        let owe = self.current_bet.saturating_sub(p.bet);
        let to_call = owe.min(p.chips);
        let max_to = p.bet + p.chips;
        let others_can_act = self
            .players
            .iter()
            .enumerate()
            .any(|(j, q)| j != seat && q.can_act());
        // A short all-in that is not a full raise does not reopen the betting for
        // players who have already acted, unless the gap has grown to a full raise.
        let reopened = !p.acted || owe >= self.min_raise;
        let can_raise = p.chips > owe && others_can_act && reopened;
        let min_to = (self.current_bet + self.min_raise).min(max_to);
        Legal {
            to_call,
            can_check: owe == 0,
            can_raise,
            min_to,
            max_to,
            stack: p.chips,
        }
    }

    /// Best made hand for a seat on the current board, if at least a flop is out.
    pub fn made_hand(&self, seat: usize) -> Option<String> {
        let p = &self.players[seat];
        if p.hole.len() != 2 || self.board.len() < 3 {
            return None;
        }
        let mut cards = p.hole.clone();
        cards.extend_from_slice(&self.board);
        Some(eval::describe(eval::evaluate(&cards)))
    }

    /// Current place of an eliminated player (places shift if players join).
    pub fn place_of(&self, seat: usize) -> Option<usize> {
        let order = self.players[seat].eliminated?;
        let later = self
            .history
            .iter()
            .filter(|f| !f.left && f.order > order)
            .count();
        Some(self.contenders() + later + 1)
    }

    /// Everyone who played: players still in by chips, then eliminated players
    /// (most recent first), then players who left.
    pub fn standings(&self) -> Vec<Standing> {
        let mut rows: Vec<Standing> = Vec::new();
        let mut seated: Vec<&Player> = self.players.iter().filter(|p| p.contending()).collect();
        seated.sort_by_key(|p| std::cmp::Reverse(p.chips));
        for p in seated {
            rows.push(Standing {
                place: Some(rows.len() + 1),
                name: p.name.clone(),
                is_human: p.is_human,
                style: p.personality.style.to_string(),
                chips: p.chips,
                status: format!("{} chips", chips_text(p.chips)),
                stats: p.stats,
            });
        }
        let mut gone: Vec<&Finish> = self.history.iter().filter(|f| !f.left).collect();
        gone.sort_by_key(|f| std::cmp::Reverse(f.order));
        for f in gone {
            rows.push(Standing {
                place: Some(rows.len() + 1),
                name: f.name.clone(),
                is_human: f.is_human,
                style: f.style.clone(),
                chips: 0,
                status: format!("eliminated in hand {}", f.hand),
                stats: f.stats,
            });
        }
        for f in self.history.iter().filter(|f| f.left) {
            rows.push(Standing {
                place: None,
                name: f.name.clone(),
                is_human: f.is_human,
                style: f.style.clone(),
                chips: f.chips,
                status: format!("left in hand {} with {}", f.hand, chips_text(f.chips)),
                stats: f.stats,
            });
        }
        rows
    }

    // ------------------------------------------------------------------ helpers

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

    // ------------------------------------------------------------ seats

    /// Seats a new player between hands. The seat must be empty or hold an
    /// eliminated player.
    pub fn seat_player(
        &mut self,
        seat: usize,
        name: &str,
        is_human: bool,
        personality: Personality,
        chips: u32,
    ) -> Result<(), String> {
        if seat >= self.players.len() {
            return Err("there is no such seat".to_string());
        }
        if !self.between_hands() {
            return Err("players can only sit down between hands".to_string());
        }
        if !self.players[seat].out {
            return Err("that seat is taken".to_string());
        }
        self.players[seat] = Player::new(name.to_string(), is_human, personality, chips);
        self.total_chips += chips as u64;
        if self.hand_no > 0 {
            self.log(format!(
                "{} joins the table with {} chips",
                name,
                chips_text(chips)
            ));
        }
        self.events.push(GameEvent::Joined(seat));
        Ok(())
    }

    /// A player leaves the table between hands, taking their chips along.
    pub fn remove_player(&mut self, seat: usize) -> Result<u32, String> {
        if !self.between_hands() {
            return Err("players can only leave between hands".to_string());
        }
        let p = &self.players[seat];
        if p.vacant {
            return Err("that seat is already empty".to_string());
        }
        if p.eliminated.is_some() {
            // Already in the standings: just free the seat.
            self.players[seat] = Player::empty_seat();
            return Ok(0);
        }
        let chips = p.chips;
        let name = p.name.clone();
        let record = self.finish_record(seat, true, 0);
        self.history.push(record);
        self.total_chips -= chips as u64;
        self.players[seat] = Player::empty_seat();
        self.log(format!(
            "{} leaves the table with {} chips",
            name,
            chips_text(chips)
        ));
        self.events.push(GameEvent::Left(seat));
        Ok(chips)
    }

    /// A busted player buys back in for the starting stack.
    pub fn rebuy(&mut self, seat: usize) -> Result<(), String> {
        if !self.between_hands() || !self.players[seat].rebuy_pending {
            return Err("a rebuy is not possible now".to_string());
        }
        let chips = self.rules.rebuy_stack();
        let p = &mut self.players[seat];
        p.rebuy_pending = false;
        p.rebuys += 1;
        p.chips = chips;
        let name = p.name.clone();
        self.total_chips += chips as u64;
        self.log(format!("{} rebuys for {} chips", name, chips_text(chips)));
        self.events.push(GameEvent::Rebuy(seat));
        Ok(())
    }

    /// A busted player gives up the chance to rebuy and is eliminated now.
    pub fn decline_rebuy(&mut self, seat: usize) {
        if self.players[seat].rebuy_pending {
            self.players[seat].rebuy_pending = false;
            self.eliminate(vec![seat]);
        }
    }

    /// Knocks the given busted players out. Among players busted together, the
    /// one who started the hand with more chips finishes higher.
    fn eliminate(&mut self, mut seats: Vec<usize>) {
        if seats.is_empty() {
            return;
        }
        seats.sort_by_key(|&i| (self.players[i].start_chips, std::cmp::Reverse(i)));
        for &i in &seats {
            self.eliminations += 1;
            let order = self.eliminations;
            let record = self.finish_record(i, false, order);
            self.history.push(record);
            let p = &mut self.players[i];
            p.out = true;
            p.rebuy_pending = false;
            p.eliminated = Some(order);
        }
        for &i in seats.iter().rev() {
            let name = self.players[i].name.clone();
            let place = self.place_of(i).unwrap_or(0);
            self.log(format!(
                "{} is eliminated in {} place",
                name,
                ordinal(place)
            ));
            self.events.push(GameEvent::Eliminated(i));
        }
    }

    // ------------------------------------------------------------- starting hands

    fn update_blinds(&mut self) {
        let hpl = self.rules.hands_per_level;
        self.level = match (self.hand_no - 1).checked_div(hpl) {
            Some(lvl) => lvl + 1,
            // The blinds never go up.
            None => 1,
        };
        self.sb = self.rules.small_blind_at(self.level);
        self.bb = self.sb * 2;
        self.ante = self.rules.ante_for(self.bb);
    }

    fn finish_tournament(&mut self) {
        self.state = State::GameOver;
        let standings = self.standings();
        if let Some(first) = standings.first() {
            let text = if self.contenders() == 1 {
                format!("{} wins the tournament!", first.name)
            } else if self.hand_limit_reached() {
                format!(
                    "Hand limit reached: {} finishes first with {} chips",
                    first.name,
                    chips_text(first.chips)
                )
            } else {
                format!("Game over: {} is the chip leader", first.name)
            };
            self.log(text);
        }
        self.events.push(GameEvent::TournamentOver);
    }

    /// Picks the button and blind positions for a new hand.
    fn choose_positions(&mut self) {
        let alive = self.alive_count();
        let first_hand = self.hand_no == 1;
        if !first_hand && self.rules.dead_button && alive >= 3 {
            // The big blind moves to the next player; the small blind takes last
            // hand's big blind seat and the button last hand's small blind seat,
            // even if those seats are empty now.
            let bb = self.next_alive(self.bb_seat);
            let sb = self.bb_seat;
            let button = self.sb_seat;
            if bb != sb && bb != button && sb != button {
                self.button = button;
                self.sb_seat = sb;
                self.bb_seat = bb;
                self.sb_dead = self.players[sb].out;
                return;
            }
            // Seats changed too much for the rule to apply: move the button normally.
        }
        if !first_hand || self.players[self.button].out {
            self.button = self.next_alive(self.button);
        }
        self.sb_dead = false;
        if alive == 2 {
            // Heads-up the button posts the small blind.
            self.sb_seat = self.button;
            self.bb_seat = self.next_alive(self.button);
        } else {
            self.sb_seat = self.next_alive(self.button);
            self.bb_seat = self.next_alive(self.sb_seat);
        }
    }

    /// Deals a new hand with a freshly shuffled deck.
    pub fn start_hand(&mut self) {
        let mut deck = Deck::new();
        deck.shuffle(&mut self.rng);
        self.start_hand_with_deck(deck);
    }

    /// Deals a new hand from the given deck (cards are drawn from the top).
    /// Hole cards go out one at a time starting left of the button, then the
    /// flop, turn and river each follow a burn card.
    pub fn start_hand_with_deck(&mut self, deck: Deck) {
        if self.state == State::GameOver {
            return;
        }
        self.events.clear();
        // Busted players who did not rebuy are out now.
        let pending: Vec<usize> = (0..self.players.len())
            .filter(|&i| {
                self.players[i].rebuy_pending
                    || (!self.players[i].out && self.players[i].chips == 0)
            })
            .collect();
        for &i in &pending {
            self.players[i].rebuy_pending = false;
        }
        self.eliminate(pending);
        if self.tournament_over() {
            self.finish_tournament();
            return;
        }

        self.hand_no += 1;
        self.update_blinds();
        self.choose_positions();
        let deep = self.bb as u64 * self.stats_min_stack_bb as u64;
        for p in self.players.iter_mut() {
            p.hole.clear();
            p.bet = 0;
            p.total = 0;
            p.folded = false;
            p.all_in = false;
            p.acted = false;
            p.in_hand = !p.out;
            p.start_chips = p.chips;
            p.payout = 0;
            p.score = None;
            p.shown = false;
            p.last_action.clear();
            p.played_this_hand = false;
            p.raised_this_hand = false;
            // Short stacks must play push-or-fold whatever their style: not counted.
            p.tracked = p.in_hand && p.chips as u64 >= deep;
            if p.tracked {
                p.stats.hands += 1;
            }
        }
        self.deck = deck;
        self.board.clear();
        self.street = Street::PreFlop;
        self.awards.clear();
        self.result_lines.clear();
        self.refund = None;
        self.showdown = false;
        self.reveal_all = false;
        self.river_aggressor = None;
        self.straddle_seat = None;

        let (sb, bb, ante) = (self.sb, self.bb, self.ante);
        let dealer = self.players[self.button].name.clone();
        let dealer = if dealer.is_empty() {
            "an empty seat".to_string()
        } else {
            dealer
        };
        let hand_no = self.hand_no;
        let level = self.level;
        self.log(format!(
            "--- Hand #{} (level {}, blinds {}/{}) - the button is on {} ---",
            hand_no, level, sb, bb, dealer
        ));

        if ante > 0 {
            for i in 0..self.players.len() {
                if self.players[i].in_hand {
                    let p = &mut self.players[i];
                    let paid = ante.min(p.chips);
                    p.chips -= paid;
                    p.total += paid;
                    if p.chips == 0 {
                        p.all_in = true;
                    }
                }
            }
            self.log(format!("Everyone antes {}", ante));
        }

        let sb_seat = self.sb_seat;
        let bb_seat = self.bb_seat;
        if self.sb_dead {
            self.log("The small blind is dead (empty seat)".to_string());
        } else {
            let paid = self.commit(sb_seat, sb);
            let name = self.players[sb_seat].name.clone();
            self.players[sb_seat].last_action = format!("SB {}", paid);
            self.log(format!("{} posts the small blind ({})", name, paid));
        }
        let paid_bb = self.commit(bb_seat, bb);
        let bb_name = self.players[bb_seat].name.clone();
        self.players[bb_seat].last_action = format!("BB {}", paid_bb);
        self.log(format!("{} posts the big blind ({})", bb_name, paid_bb));

        // The call amount is the full blind even if the poster was short.
        self.current_bet = bb;
        self.min_raise = bb;
        self.last_actor = bb_seat;
        let straddle = bb * self.rules.straddle_multiplier.max(2);
        if self.rules.straddle && self.alive_count() >= self.rules.straddle_min_players.max(3) {
            let s = self.next_alive(bb_seat);
            let paid = self.commit(s, straddle);
            let name = self.players[s].name.clone();
            self.players[s].last_action = format!("Straddle {}", paid);
            self.log(format!("{} straddles ({})", name, paid));
            self.straddle_seat = Some(s);
            self.current_bet = straddle;
            self.min_raise = straddle;
            self.last_actor = s;
        }

        let first = self.next_alive(self.button);
        for _ in 0..2 {
            let mut i = first;
            loop {
                let c = self.deck.draw();
                self.players[i].hole.push(c);
                i = self.next_alive(i);
                if i == first {
                    break;
                }
            }
        }
        self.events.push(GameEvent::HandStarted);

        self.state = State::Betting;
        self.advance();
    }

    // ------------------------------------------------------------ betting rounds

    /// The next player who still has to act after `from`, if any.
    fn next_actor(&self, from: usize) -> Option<usize> {
        let n = self.players.len();
        let can_act = self.players.iter().filter(|p| p.can_act()).count();
        if can_act == 0 {
            return None;
        }
        for k in 1..=n {
            let i = (from + k) % n;
            let p = &self.players[i];
            if !p.can_act() {
                continue;
            }
            // With a single player left able to act, only a bet another live player
            // really made needs an answer. (The full big blind is owed while others
            // can still act, even if the big blind is all-in for less, but once
            // everyone else is all-in a player who already covers them is done.)
            let owes = if can_act == 1 {
                let top = self
                    .players
                    .iter()
                    .enumerate()
                    .filter(|&(j, q)| j != i && q.live())
                    .map(|(_, q)| q.bet)
                    .max()
                    .unwrap_or(0);
                p.bet < top
            } else {
                !p.acted || p.bet < self.current_bet
            };
            if owes {
                return Some(i);
            }
        }
        None
    }

    /// Moves the hand forward until a decision is needed, the board must be run
    /// out, or the hand is settled.
    fn advance(&mut self) {
        loop {
            if self.live_count() <= 1 {
                self.settle(false);
                return;
            }
            if let Some(i) = self.next_actor(self.last_actor) {
                self.to_act = i;
                self.state = State::Betting;
                return;
            }
            // The betting round is complete.
            for p in self.players.iter_mut() {
                p.bet = 0;
                p.acted = false;
            }
            self.current_bet = 0;
            self.min_raise = self.bb;
            if self.street == Street::River {
                self.settle(true);
                return;
            }
            if self.players.iter().filter(|p| p.can_act()).count() <= 1 {
                // Nobody (or only one player) can bet any more: show the cards and deal out.
                self.reveal_all = true;
                for p in self.players.iter_mut() {
                    if p.live() {
                        p.shown = true;
                    }
                }
                self.state = State::Runout;
                self.log("All-in: cards are turned up".to_string());
                return;
            }
            self.deal_street();
            self.last_actor = self.button;
        }
    }

    fn deal_street(&mut self) {
        self.deck.draw(); // burn card
        let mut dealt = Vec::new();
        match self.street {
            Street::PreFlop => {
                for _ in 0..3 {
                    dealt.push(self.deck.draw());
                }
                self.street = Street::Flop;
            }
            Street::Flop => {
                dealt.push(self.deck.draw());
                self.street = Street::Turn;
            }
            Street::Turn => {
                dealt.push(self.deck.draw());
                self.street = Street::River;
            }
            Street::River => unreachable!("no street follows the river"),
        }
        self.board.extend_from_slice(&dealt);
        for p in self.players.iter_mut() {
            p.last_action = if p.folded {
                "Fold".to_string()
            } else if p.all_in {
                "All-in".to_string()
            } else {
                String::new()
            };
        }
        let street = self.street.name();
        let board = cards_text(&self.board);
        self.log(format!("{}: {}", street, board));
        self.events.push(GameEvent::Street(self.street));
    }

    /// While the state is `Runout`, deals the next street (or settles after the river).
    pub fn runout_step(&mut self) {
        if self.state != State::Runout {
            return;
        }
        if self.street == Street::River {
            self.settle(true);
        } else {
            self.deal_street();
        }
    }

    fn raise_to(&mut self, seat: usize, to: u32) {
        {
            let p = &mut self.players[seat];
            let add = to - p.bet;
            p.chips -= add;
            p.bet = to;
            p.total += add;
            if p.chips == 0 {
                p.all_in = true;
            }
        }
        let increment = to - self.current_bet;
        // A short all-in raise does not change the minimum raise size.
        if increment >= self.min_raise {
            self.min_raise = increment;
        }
        self.current_bet = to;
    }

    /// Statistics bookkeeping for a voluntary action.
    fn record(&mut self, seat: usize, aggressive: bool, paid: u32) {
        let preflop = self.street == Street::PreFlop;
        let weak = !preflop && {
            let p = &self.players[seat];
            let mut cards = p.hole.clone();
            cards.extend_from_slice(&self.board);
            cards.len() >= 5 && eval::category(eval::evaluate(&cards)) == Category::HighCard
        };
        let p = &mut self.players[seat];
        if !p.tracked {
            return;
        }
        if preflop && (aggressive || paid > 0) && !p.played_this_hand {
            p.played_this_hand = true;
            p.stats.vpip += 1;
        }
        if preflop && aggressive && !p.raised_this_hand {
            p.raised_this_hand = true;
            p.stats.pfr += 1;
        }
        if aggressive {
            p.stats.bets += 1;
            if !preflop {
                p.stats.postflop_bets += 1;
                if weak {
                    p.stats.bluffs += 1;
                }
            }
        } else if paid > 0 {
            p.stats.calls += 1;
        }
    }

    /// Performs an action for the player whose turn it is.
    /// An illegal action returns an error and leaves the game untouched.
    pub fn act(&mut self, action: Action) -> Result<(), String> {
        if self.state != State::Betting {
            return Err("there is no betting round in progress".to_string());
        }
        let seat = self.to_act;
        let legal = self.legal(seat);
        let name = self.players[seat].name.clone();
        let action = if action == Action::Call && legal.can_check {
            Action::Check
        } else {
            action
        };

        match action {
            Action::Fold => {
                self.players[seat].folded = true;
                self.say(seat, "Fold".to_string(), format!("{} folds", name));
                self.events.push(GameEvent::Fold(seat));
            }
            Action::Check => {
                if !legal.can_check {
                    return Err(format!("{} cannot check: there is a bet to call", name));
                }
                self.say(seat, "Check".to_string(), format!("{} checks", name));
                self.events.push(GameEvent::Check(seat));
            }
            Action::Call => {
                let paid = self.commit(seat, legal.to_call);
                self.record(seat, false, paid);
                let all_in = self.players[seat].all_in;
                self.events.push(if all_in {
                    GameEvent::AllIn(seat)
                } else {
                    GameEvent::Call(seat)
                });
                let suffix = if all_in { " (all-in)" } else { "" };
                self.say(
                    seat,
                    if all_in {
                        format!("All-in {}", paid)
                    } else {
                        format!("Call {}", paid)
                    },
                    format!("{} calls {}{}", name, paid, suffix),
                );
            }
            Action::AllIn => {
                if legal.can_raise {
                    self.record(seat, true, legal.max_to);
                    self.apply_raise(seat, legal.max_to, &name);
                } else if legal.calls_all_in() {
                    let paid = self.commit(seat, legal.to_call);
                    self.record(seat, false, paid);
                    self.events.push(GameEvent::AllIn(seat));
                    self.say(
                        seat,
                        format!("All-in {}", paid),
                        format!("{} calls {} (all-in)", name, paid),
                    );
                } else {
                    return Err(format!(
                        "{} cannot go all-in here: nobody could answer a raise, so only {} is possible",
                        name,
                        if legal.can_check {
                            "a check"
                        } else {
                            "a call or a fold"
                        }
                    ));
                }
            }
            Action::RaiseTo(to) => {
                if !legal.can_raise {
                    return Err(format!("{} cannot raise here", name));
                }
                if to < legal.min_to || to > legal.max_to {
                    return Err(format!(
                        "raise must be between {} and {}",
                        legal.min_to, legal.max_to
                    ));
                }
                self.record(seat, true, to);
                self.apply_raise(seat, to, &name);
            }
        }

        self.players[seat].acted = true;
        self.last_actor = seat;
        self.action_count += 1;
        self.advance();
        Ok(())
    }

    fn apply_raise(&mut self, seat: usize, to: u32, name: &str) {
        let opening = self.current_bet == 0;
        self.raise_to(seat, to);
        if self.street == Street::River {
            self.river_aggressor = Some(seat);
        }
        let all_in = self.players[seat].all_in;
        self.events.push(if all_in {
            GameEvent::AllIn(seat)
        } else {
            GameEvent::Bet(seat)
        });
        let suffix = if all_in { " (all-in)" } else { "" };
        let (short, long) = if all_in {
            (
                format!("All-in {}", to),
                format!(
                    "{} {} {}{}",
                    name,
                    if opening { "bets" } else { "raises to" },
                    to,
                    suffix
                ),
            )
        } else if opening {
            (format!("Bet {}", to), format!("{} bets {}", name, to))
        } else {
            (
                format!("Raise {}", to),
                format!("{} raises to {}", name, to),
            )
        };
        self.say(seat, short, long);
    }

    /// Lets the bot in the current seat decide and act. Returns the action taken.
    /// If the bot ever proposes something illegal, falls back to check or fold.
    pub fn bot_move(&mut self) -> Action {
        let seat = self.to_act;
        let mut rng = self.rng.clone();
        let proposed = bot::decide(self, seat, &mut rng);
        self.rng = rng;
        if self.act(proposed).is_ok() {
            return proposed;
        }
        let fallback = if self.legal(seat).can_check {
            Action::Check
        } else {
            Action::Fold
        };
        // A check or a fold is always legal here, so this cannot fail.
        let _ = self.act(fallback);
        fallback
    }

    // ---------------------------------------------------------------- settlement

    /// Gives back chips nobody matched (the excess of the biggest contribution).
    fn return_uncalled(&mut self) -> Option<(usize, u32)> {
        let mut order: Vec<(u32, usize)> = self
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| (p.total, i))
            .collect();
        order.sort_by(|a, b| b.cmp(a));
        if order.len() < 2 {
            return None;
        }
        let (top, seat) = order[0];
        let (second, _) = order[1];
        if top > second {
            let diff = top - second;
            self.players[seat].total -= diff;
            self.players[seat].chips += diff;
            Some((seat, diff))
        } else {
            None
        }
    }

    /// Decides who turns their cards up at a showdown. Without the muck rule
    /// everyone shows. With it, players show in order (the last river bettor
    /// first, otherwise the first player left of the button) and a player who
    /// is already beaten for every pot they could win mucks instead.
    fn reveal_hands(&mut self) {
        let n = self.players.len();
        if !self.rules.muck_losers || self.reveal_all {
            for p in self.players.iter_mut() {
                if p.live() {
                    p.shown = true;
                }
            }
            return;
        }
        let start = match self.river_aggressor {
            Some(s) if self.players[s].live() => s,
            _ => self.next_alive(self.button),
        };
        let mut shown: Vec<(Score, u32)> = Vec::new();
        for k in 0..n {
            let i = (start + k) % n;
            let p = &self.players[i];
            if !p.live() {
                continue;
            }
            let score = p.score.unwrap_or(0);
            let beaten = shown.iter().any(|&(s, t)| s > score && t >= p.total);
            if beaten {
                let name = p.name.clone();
                self.log(format!("{} mucks", name));
            } else {
                shown.push((score, p.total));
                self.players[i].shown = true;
            }
        }
    }

    fn settle(&mut self, contested: bool) {
        for p in self.players.iter_mut() {
            p.bet = 0;
        }
        self.current_bet = 0;

        self.last_totals = self.players.iter().map(|p| p.total).collect();
        self.refund = self.return_uncalled();
        if let Some((seat, amount)) = self.refund {
            let name = self.players[seat].name.clone();
            self.log(format!("Uncalled bet of {} returned to {}", amount, name));
        }

        self.showdown = contested;
        if contested {
            let board = self.board.clone();
            for p in self.players.iter_mut() {
                if p.live() {
                    let mut cards = p.hole.clone();
                    cards.extend_from_slice(&board);
                    p.score = Some(eval::evaluate(&cards));
                }
            }
            let board_text = cards_text(&board);
            self.log(format!("Showdown (board: {})", board_text));
            self.reveal_hands();
            for i in 0..self.players.len() {
                if self.players[i].live() && self.players[i].shown {
                    let hole = cards_text(&self.players[i].hole);
                    let desc = eval::describe(self.players[i].score.unwrap_or(0));
                    let name = self.players[i].name.clone();
                    self.log(format!("{} shows {}: {}", name, hole, desc));
                }
            }
        }

        let totals: Vec<u32> = self.players.iter().map(|p| p.total).collect();
        let live: Vec<bool> = self.players.iter().map(|p| p.live()).collect();
        let scores: Vec<Option<Score>> = self.players.iter().map(|p| p.score).collect();
        self.last_pot = totals.iter().sum();

        let pots = compute_pots(&totals, &live);
        let awards = award_pots(&pots, &scores, self.button);
        let many = awards.len() > 1;
        let mut lines = Vec::new();
        for (k, award) in awards.iter().enumerate() {
            for (&w, &share) in award.winners.iter().zip(award.shares.iter()) {
                self.players[w].chips += share;
                self.players[w].payout += share;
            }
            let label = if !many {
                "the pot".to_string()
            } else if k == 0 {
                "the main pot".to_string()
            } else {
                format!("side pot {}", k)
            };
            let names: Vec<String> = award
                .winners
                .iter()
                .map(|&w| self.players[w].name.clone())
                .collect();
            let how = if contested {
                let score = self.players[award.winners[0]].score.unwrap_or(0);
                format!(" with {}", eval::describe(score))
            } else {
                " (everyone else folded)".to_string()
            };
            if names.len() == 1 {
                lines.push(format!(
                    "{} wins {} ({}){}",
                    names[0], label, award.amount, how
                ));
            } else {
                lines.push(format!(
                    "{} split {} ({}){}",
                    names.join(" and "),
                    label,
                    award.amount,
                    how
                ));
            }
        }
        for line in &lines {
            self.log(line.clone());
        }
        self.result_lines = lines;
        self.awards = awards;
        self.events.push(GameEvent::HandSettled);

        // The pot has been paid out.
        for p in self.players.iter_mut() {
            p.total = 0;
        }

        // Busted players either get a chance to rebuy or are knocked out.
        let busted: Vec<usize> = (0..self.players.len())
            .filter(|&i| self.players[i].in_hand && self.players[i].chips == 0)
            .collect();
        if self.rebuy_open() {
            for &i in &busted {
                self.players[i].rebuy_pending = true;
                let name = self.players[i].name.clone();
                self.log(format!("{} is busted and may rebuy", name));
            }
        } else {
            self.eliminate(busted);
        }

        self.state = State::HandOver;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_in_words() {
        let mut r = Rules::default();
        assert_eq!(r.blinds_text(), "10/20");
        assert_eq!(r.level_steps(), (20, 100));
        assert_eq!(r.levels_text(), "every 8 hands");
        assert_eq!(r.hand_limit_text(), "no limit");
        assert_eq!(r.antes_text(), "off");
        r.antes = true;
        assert_eq!(r.antes_text(), "on: 1/8 of the big blind");
        r.straddle = true;
        assert_eq!(
            r.straddle_text(),
            "on: 2 big blinds, with 4 or more players"
        );
        r.rebuy_levels = 2;
        assert_eq!(
            r.rebuys_text(),
            "during the first 2 levels, for 1,500 chips"
        );
        assert_eq!(r.button_text(), "moving button");
        assert_eq!(r.showdown_text(), "everyone shows");
    }

    #[test]
    fn unmatched_bets_do_not_make_side_pots() {
        let mut g = Game::new(Rules::default(), 4, Rng::seeded(1));
        g.setup(&[1500, 1500, 5, 1500], 0);
        g.start_hand();
        assert_eq!(g.players[2].bet, 5, "seat 2 is short in the big blind");
        g.players[2].all_in = false; // pretend for a moment it is not
        assert_eq!(g.pots_now(), vec![g.pot()]);
        g.players[2].all_in = true;
        // The short big blind's all-in closes a pot of 10 (5 from each blind);
        // the rest of the small blind is in the open pot...
        assert_eq!(g.pots_now(), vec![10, 5]);
        // ...and calls above it go to the open pot, even before others match.
        g.act(Action::Call).unwrap();
        assert_eq!(g.pots_now(), vec![15, 20]);
        assert_eq!(g.pots_now().iter().sum::<u32>(), g.pot());
    }

    #[test]
    fn pots_with_three_all_ins() {
        let pots = compute_pots(&[100, 300, 500], &[true, true, true]);
        assert_eq!(
            pots,
            vec![
                Pot {
                    amount: 300,
                    eligible: vec![0, 1, 2]
                },
                Pot {
                    amount: 400,
                    eligible: vec![1, 2]
                },
                Pot {
                    amount: 200,
                    eligible: vec![2]
                },
            ]
        );
    }

    #[test]
    fn folded_chips_stay_in_the_pot() {
        let pots = compute_pots(&[100, 300, 300, 40], &[true, true, true, false]);
        assert_eq!(
            pots,
            vec![
                Pot {
                    amount: 340,
                    eligible: vec![0, 1, 2]
                },
                Pot {
                    amount: 400,
                    eligible: vec![1, 2]
                },
            ]
        );
    }

    #[test]
    fn odd_chip_goes_left_of_the_button() {
        let pots = vec![Pot {
            amount: 101,
            eligible: vec![1, 3],
        }];
        let scores = vec![None, Some(5), None, Some(5)];
        let a = award_pots(&pots, &scores, 2);
        assert_eq!(a[0].winners, vec![3, 1]);
        assert_eq!(a[0].shares, vec![51, 50]);
        let a = award_pots(&pots, &scores, 0);
        assert_eq!(a[0].winners, vec![1, 3]);
        assert_eq!(a[0].shares, vec![51, 50]);
    }

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
    fn the_default_blind_schedule_matches_the_classic_levels() {
        let r = Rules::default();
        let levels: Vec<u32> = (1..=14).map(|l| r.small_blind_at(l)).collect();
        assert_eq!(
            levels,
            vec![
                10, 15, 25, 50, 75, 100, 150, 200, 300, 400, 500, 600, 900, 1350
            ]
        );
        let custom = Rules {
            small_blind: 5,
            blind_schedule: vec![100, 200],
            blind_growth_percent: 200,
            ..Rules::default()
        };
        let levels: Vec<u32> = (1..=4).map(|l| custom.small_blind_at(l)).collect();
        assert_eq!(levels, vec![5, 10, 20, 40]);
        // Huge levels stop growing instead of overflowing.
        assert!(r.small_blind_at(10_000) <= u32::MAX / 8);
    }

    #[test]
    fn log_numbering_survives_trimming() {
        let mut g = Game::new(Rules::default(), 3, Rng::seeded(1));
        for i in 0..(LOG_LIMIT + 50) {
            g.log(format!("line {}", i));
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
