//! Seating and removing players, rebuys, and eliminations.

use super::{Game, GameEvent, Player, chips_text, ordinal};
use crate::bot::Personality;

impl Game {
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
        self.total_chips += u64::from(chips);
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
        self.total_chips -= u64::from(chips);
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
        self.total_chips += u64::from(chips);
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
    pub(super) fn eliminate(&mut self, mut seats: Vec<usize>) {
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
}
