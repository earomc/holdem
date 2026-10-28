//! Questions about the game that change nothing: pots, players, whose turn it
//! is, what they may do, standings.

use super::{Finish, Game, GameEvent, Legal, Player, Standing, State, chips_text};
use crate::eval;

impl Game {
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
            .map(|p| u64::from(p.chips + p.total))
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
        (u64::from(self.hand_no) << 32) | (self.action_count & 0xFFFF_FFFF)
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
}
