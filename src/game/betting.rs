//! The betting rounds: whose turn it is, legal actions and raises, dealing the
//! next street, and the bots' moves.

use super::{Action, Game, GameEvent, State, Street};
use crate::bot::{self};
use crate::card::cards_text;
use crate::eval::{self, Category};

impl Game {
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
    pub(super) fn advance(&mut self) {
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
            for p in &mut self.players {
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
                for p in &mut self.players {
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
        for p in &mut self.players {
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
        self.log(format!("{street}: {board}"));
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

    pub(super) fn raise_to(&mut self, seat: usize, to: u32) {
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
    pub(super) fn record(&mut self, seat: usize, aggressive: bool, paid: u32) {
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
                self.say(seat, "Fold".to_string(), format!("{name} folds"));
                self.events.push(GameEvent::Fold(seat));
            }
            Action::Check => {
                if !legal.can_check {
                    return Err(format!("{name} cannot check: there is a bet to call"));
                }
                self.say(seat, "Check".to_string(), format!("{name} checks"));
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
                        format!("All-in {paid}")
                    } else {
                        format!("Call {paid}")
                    },
                    format!("{name} calls {paid}{suffix}"),
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
                        format!("All-in {paid}"),
                        format!("{name} calls {paid} (all-in)"),
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
                    return Err(format!("{name} cannot raise here"));
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
                format!("All-in {to}"),
                format!(
                    "{} {} {}{}",
                    name,
                    if opening { "bets" } else { "raises to" },
                    to,
                    suffix
                ),
            )
        } else if opening {
            (format!("Bet {to}"), format!("{name} bets {to}"))
        } else {
            (format!("Raise {to}"), format!("{name} raises to {to}"))
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
}
