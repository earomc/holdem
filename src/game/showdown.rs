//! The end of a hand: returning uncalled bets, showing hands and paying the pots.

use super::pots::{Award, Pot, award_pots, compute_pots};
use super::{Game, GameEvent, HandResult, Outcome, Player, State};
use crate::card::cards_text;
use crate::eval::{self, Score};

impl Game {
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
            for p in &mut self.players {
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
                self.log(format!("{name} mucks"));
            } else {
                shown.push((score, p.total));
                self.players[i].shown = true;
            }
        }
    }

    pub(super) fn settle(&mut self, contested: bool) {
        for p in &mut self.players {
            p.bet = 0;
        }
        self.current_bet = 0;

        self.last_totals = self.players.iter().map(|p| p.total).collect();
        self.refund = self.return_uncalled();
        if let Some((seat, amount)) = self.refund {
            let name = self.players[seat].name.clone();
            self.log(format!("Uncalled bet of {amount} returned to {name}"));
        }

        self.showdown = contested;
        if contested {
            self.show_hands();
        }
        self.pay_pots(contested);
        self.record_results(contested);
        self.events.push(GameEvent::HandSettled);

        // The pot has been paid out.
        for p in &mut self.players {
            p.total = 0;
        }
        self.handle_busted();

        self.state = State::HandOver;
    }

    /// Scores every hand still in, and shows them in order (see [`Game::reveal_hands`]).
    fn show_hands(&mut self) {
        let board = self.board.clone();
        for p in &mut self.players {
            if p.live() {
                let mut cards = p.hole.clone();
                cards.extend_from_slice(&board);
                p.score = Some(eval::evaluate(&cards));
            }
        }
        let board_text = cards_text(&board);
        self.log(format!("Showdown (board: {board_text})"));
        self.reveal_hands();
        for i in 0..self.players.len() {
            if self.players[i].live() && self.players[i].shown {
                let hole = cards_text(&self.players[i].hole);
                let desc = eval::describe(self.players[i].score.unwrap_or(0));
                let name = self.players[i].name.clone();
                self.log(format!("{name} shows {hole}: {desc}"));
            }
        }
    }

    /// How the hand went for each player dealt in (after the pots are paid).
    fn record_results(&mut self, contested: bool) {
        let live: Vec<usize> = (0..self.players.len())
            .filter(|&i| self.players[i].live())
            .collect();
        for i in 0..self.players.len() {
            let p = &self.players[i];
            if !p.in_hand {
                self.players[i].last_result = None;
                continue;
            }
            let won_alone = self
                .awards
                .iter()
                .any(|a| a.winners.len() == 1 && a.winners[0] == i);
            let shared = self.awards.iter().any(|a| a.winners.contains(&i));
            let outcome = match p.folded_on {
                Some(street) => Outcome::Folded(street),
                None if !contested => Outcome::WonUncontested,
                None if won_alone => Outcome::WonShowdown,
                None if shared => Outcome::Split,
                None => Outcome::LostShowdown,
            };
            let at_showdown = contested && p.live();
            let result = HandResult {
                hand: self.hand_no,
                net: i64::from(p.chips) - i64::from(p.start_chips),
                hole: p.hole.clone(),
                made: at_showdown.then(|| p.score.map(eval::describe)).flatten(),
                outcome,
                bluffed: p.bluffed_this_hand,
                against: if at_showdown {
                    live.iter()
                        .filter(|&&j| j != i)
                        .map(|&j| self.players[j].name.clone())
                        .collect()
                } else {
                    Vec::new()
                },
            };
            self.players[i].last_result = Some(result);
        }
    }

    /// Splits the chips into the main and side pots and pays their winners.
    fn pay_pots(&mut self, contested: bool) {
        let totals: Vec<u32> = self.players.iter().map(|p| p.total).collect();
        let live: Vec<bool> = self.players.iter().map(Player::live).collect();
        let scores: Vec<Option<Score>> = self.players.iter().map(|p| p.score).collect();
        self.last_pot = totals.iter().sum();

        let pots = compute_pots(&totals, &live);
        let awards = award_pots(&pots, &scores, self.button);
        let many = awards.len() > 1;
        let mut lines = Vec::new();
        for (k, (award, pot)) in awards.iter().zip(&pots).enumerate() {
            for (&w, &share) in award.winners.iter().zip(award.shares.iter()) {
                self.players[w].chips += share;
                self.players[w].payout += share;
            }
            lines.push(self.award_text(k, many, award, pot, contested));
        }
        for line in &lines {
            self.log(line.clone());
        }
        self.result_lines = lines;
        self.awards = awards;
    }

    /// The log line for pot number `k` (of several if `many`).
    fn award_text(
        &self,
        k: usize,
        many: bool,
        award: &Award,
        pot: &Pot,
        contested: bool,
    ) -> String {
        let label = if !many {
            "the pot".to_string()
        } else if k == 0 {
            "the main pot".to_string()
        } else {
            format!("side pot {k}")
        };
        let names: Vec<String> = award
            .winners
            .iter()
            .map(|&w| self.players[w].name.clone())
            .collect();
        let how = if contested {
            let score = self.players[award.winners[0]].score.unwrap_or(0);
            // The best hand shown against it: mucked cards stay secret.
            let runner_up = pot
                .eligible
                .iter()
                .filter(|i| !award.winners.contains(i) && self.players[**i].shown)
                .filter_map(|&i| self.players[i].score)
                .max();
            match runner_up.and_then(|r| eval::deciding_kicker(score, r)) {
                Some(rank) => format!(
                    " with {}, {} kicker",
                    eval::describe(score),
                    eval::rank_name(rank)
                ),
                None => format!(" with {}", eval::describe(score)),
            }
        } else {
            " (everyone else folded)".to_string()
        };
        if names.len() == 1 {
            format!("{} wins {} ({}){}", names[0], label, award.amount, how)
        } else {
            format!(
                "{} split {} ({}){}",
                names.join(" and "),
                label,
                award.amount,
                how
            )
        }
    }

    /// Busted players either get a chance to rebuy or are knocked out.
    fn handle_busted(&mut self) {
        let busted: Vec<usize> = (0..self.players.len())
            .filter(|&i| self.players[i].in_hand && self.players[i].chips == 0)
            .collect();
        if self.rebuy_open() {
            for &i in &busted {
                self.players[i].rebuy_pending = true;
                let name = self.players[i].name.clone();
                self.log(format!("{name} is busted and may rebuy"));
            }
        } else {
            self.eliminate(busted);
        }
    }
}
