//! Starting a hand: blind levels, positions, blinds, antes, the straddle and
//! dealing; and the end of the game.

use super::{Game, GameEvent, State, Street, chips_text};
use crate::card::Deck;

impl Game {
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
        self.drop_busted_players();
        if self.tournament_over() {
            self.finish_tournament();
            return;
        }

        self.hand_no += 1;
        self.update_blinds();
        self.choose_positions();
        self.reset_for_hand(deck);
        self.log_hand_header();
        self.post_antes();
        self.post_blinds();
        self.post_straddle();
        self.deal_hole_cards();
        self.events.push(GameEvent::HandStarted);

        self.state = State::Betting;
        self.advance();
    }

    /// Busted players who did not rebuy leave the game.
    fn drop_busted_players(&mut self) {
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
    }

    /// Clears the last hand: every player's cards and bets, the board, the pots.
    fn reset_for_hand(&mut self, deck: Deck) {
        let deep = u64::from(self.bb) * u64::from(self.stats_min_stack_bb);
        for p in &mut self.players {
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
            p.folded_on = None;
            p.bluffed_this_hand = false;
            // Short stacks must play push-or-fold whatever their style: not counted.
            p.tracked = p.in_hand && u64::from(p.chips) >= deep;
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
    }

    /// The first log line of a hand.
    fn log_hand_header(&mut self) {
        let (sb, bb) = (self.sb, self.bb);
        let dealer = self.players[self.button].name.clone();
        let dealer = if dealer.is_empty() {
            "an empty seat".to_string()
        } else {
            dealer
        };
        let hand_no = self.hand_no;
        let level = self.level;
        self.log(format!(
            "--- Hand #{hand_no} (level {level}, blinds {sb}/{bb}) - the button is on {dealer} ---"
        ));
    }

    /// Everyone in the hand pays the ante, if there is one.
    fn post_antes(&mut self) {
        let ante = self.ante;
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
            self.log(format!("Everyone antes {ante}"));
        }
    }

    /// The small blind (unless it is dead) and the big blind.
    fn post_blinds(&mut self) {
        let (sb, bb) = (self.sb, self.bb);
        let sb_seat = self.sb_seat;
        let bb_seat = self.bb_seat;
        if self.sb_dead {
            self.log("The small blind is dead (empty seat)".to_string());
        } else {
            let paid = self.commit(sb_seat, sb);
            let name = self.players[sb_seat].name.clone();
            self.players[sb_seat].last_action = format!("SB {paid}");
            self.log(format!("{name} posts the small blind ({paid})"));
        }
        let paid_bb = self.commit(bb_seat, bb);
        let bb_name = self.players[bb_seat].name.clone();
        self.players[bb_seat].last_action = format!("BB {paid_bb}");
        self.log(format!("{bb_name} posts the big blind ({paid_bb})"));

        // The call amount is the full blind even if the poster was short.
        self.current_bet = bb;
        self.min_raise = bb;
        self.last_actor = bb_seat;
    }

    /// The optional straddle, with enough players: a third blind of twice the big blind.
    fn post_straddle(&mut self) {
        let bb = self.bb;
        let straddle = bb * self.rules.straddle_multiplier.max(2);
        if self.rules.straddle && self.alive_count() >= self.rules.straddle_min_players.max(3) {
            let s = self.next_alive(self.bb_seat);
            let paid = self.commit(s, straddle);
            let name = self.players[s].name.clone();
            self.players[s].last_action = format!("Straddle {paid}");
            self.log(format!("{name} straddles ({paid})"));
            self.straddle_seat = Some(s);
            self.current_bet = straddle;
            self.min_raise = straddle;
            self.last_actor = s;
        }
    }

    /// Two hole cards each, one at a time, starting left of the button.
    fn deal_hole_cards(&mut self) {
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
    }
}
