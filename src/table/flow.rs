//! The game going on: starting games and hands, seating, the bots and time
//! limits as time passes, and going back to the lobby.

use std::time::{Duration, Instant};

use super::{Client, ClientId, Table};
use crate::bot::{self, Personality};
use crate::card::Rng;
use crate::game::{Action, Game, MIN_PLAYERS, State};

impl Table {
    pub(super) fn back_to_lobby(&mut self) {
        self.game = None;
        self.paused = None;
        self.clients.retain(|_, c| c.connected);
        for c in self.clients.values_mut() {
            c.seat = None;
            c.spectator = false;
            c.ready = false;
            c.log_seq = 0;
            c.events.clear();
        }
        self.dirty = true;
    }

    pub(super) fn try_start(&mut self, now: Instant) {
        let people: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| c.joined && c.connected)
            .map(|(id, _)| *id)
            .collect();
        if people.is_empty() || !people.iter().all(|id| self.clients[id].ready) {
            return;
        }
        let seats = self.settings.seats;
        let humans = people.len().min(seats);
        let total = if self.settings.fill_with_bots {
            seats
        } else {
            humans
        };
        if total < MIN_PLAYERS {
            for id in &people {
                self.notice(
                    *id,
                    "At least two players are needed: wait for someone to join, or turn on bots.",
                );
                if let Some(c) = self.clients.get_mut(id) {
                    c.ready = false;
                }
            }
            return;
        }
        let rules = self.settings.rules.clone();
        let mut g = Game::empty(rules, seats, Rng::seeded(self.rng.next_u64()));
        g.bot_samples = self.settings.bot_samples.max(1);
        g.stats_min_stack_bb = self.settings.stats_min_stack_bb;
        let chips = g.rules.starting_chips;
        for (seat, id) in people.iter().take(humans).enumerate() {
            let name = self.clients[id].name.clone();
            g.seat_player(seat, &name, true, Personality::default(), chips)
                .expect("the seat is free");
            if let Some(c) = self.clients.get_mut(id) {
                c.seat = Some(seat);
            }
        }
        if self.settings.fill_with_bots {
            let mut taken: Vec<String> = self.clients.values().map(|c| c.name.clone()).collect();
            let bots = bot::roster_from(&self.settings.bot_names, seats - humans, &mut self.rng);
            for (k, (_, style)) in bots.into_iter().enumerate() {
                let name = bot::unused_name(&self.settings.bot_names, &taken, &mut self.rng);
                taken.push(name.clone());
                g.seat_player(humans + k, &name, false, style, chips)
                    .expect("the seat is free");
            }
        }
        g.take_events();
        for c in self.clients.values_mut() {
            c.log_seq = 0;
            c.events.clear();
        }
        self.game = Some(g);
        self.start_next_hand(now);
    }

    /// Seat changes between hands, then the deal.
    fn start_next_hand(&mut self, now: Instant) {
        if self.game.as_ref().is_some_and(|g| g.hand_no > 0) {
            self.reseat();
        }
        let Some(g) = self.game.as_mut() else {
            return;
        };
        g.start_hand();
        for c in self.clients.values_mut() {
            c.ready = false;
        }
        self.hand_over_since = None;
        self.next_step = now + self.delay();
        self.dirty = true;
        self.collect_events();
    }

    pub(super) fn delay(&self) -> Duration {
        Duration::from_millis(u64::from(self.settings.speed_ms))
    }

    fn client_at(&self, seat: usize) -> Option<(&ClientId, &Client)> {
        self.clients.iter().find(|(_, c)| c.seat == Some(seat))
    }

    /// People leaving, bots coming and going, and waiting people sitting down.
    fn reseat(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        // People whose connection dropped leave the table.
        let gone: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| !c.connected)
            .map(|(id, _)| *id)
            .collect();
        for id in gone {
            if let Some(c) = self.clients.remove(&id)
                && let Some(s) = c.seat
            {
                let _ = g.remove_player(s);
            }
        }
        // Seats of people who were knocked out are free for others now; they watch.
        for c in self.clients.values_mut() {
            if let Some(s) = c.seat
                && g.players[s].out
            {
                c.seat = None;
                c.spectator = true;
            }
        }
        let starting = g.rules.starting_chips;
        if let Some(f) = self.settings.fluctuation.clone() {
            for s in 0..g.players.len() {
                let p = &g.players[s];
                if !p.is_human
                    && !p.out
                    && !p.rebuy_pending
                    && g.alive_count() >= 3
                    && self.rng.chance(f.leave_chance)
                {
                    let _ = g.remove_player(s);
                }
            }
        }
        // People waiting for a seat get one; if the table is full, a bot makes room.
        let waiting: Vec<ClientId> = self
            .clients
            .iter()
            .filter(|(_, c)| c.joined && c.connected && c.seat.is_none() && !c.spectator)
            .map(|(id, _)| *id)
            .collect();
        for id in waiting {
            let free =
                (0..g.players.len()).find(|&s| g.players[s].out && !g.players[s].rebuy_pending);
            let seat = match free {
                Some(s) => Some(s),
                None => {
                    let bots: Vec<usize> = (0..g.players.len())
                        .filter(|&s| {
                            !g.players[s].is_human
                                && !g.players[s].out
                                && !g.players[s].rebuy_pending
                        })
                        .collect();
                    if bots.is_empty() {
                        None
                    } else {
                        let s = bots[self.rng.below(bots.len() as u64) as usize];
                        let _ = g.remove_player(s);
                        Some(s)
                    }
                }
            };
            if let Some(s) = seat {
                let name = self.clients[&id].name.clone();
                if g.seat_player(s, &name, true, Personality::default(), starting)
                    .is_ok()
                    && let Some(c) = self.clients.get_mut(&id)
                {
                    c.seat = Some(s);
                }
            }
        }
        if let Some(f) = self.settings.fluctuation.clone() {
            for s in 0..g.players.len() {
                if g.players[s].out && !g.players[s].rebuy_pending && self.rng.chance(f.join_chance)
                {
                    let taken: Vec<String> = g
                        .players
                        .iter()
                        .map(|p| p.name.clone())
                        .chain(self.clients.values().map(|c| c.name.clone()))
                        .collect();
                    let (name, style) =
                        bot::newcomer(&self.settings.bot_names, &taken, &mut self.rng);
                    let _ = g.seat_player(s, &name, false, style, starting);
                }
            }
        }
    }

    /// Moves events from the game into every client's queue.
    pub(super) fn collect_events(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        let events = g.take_events();
        if events.is_empty() {
            return;
        }
        for c in self.clients.values_mut() {
            c.events.extend_from_slice(&events);
            // A client that is not reading its queue must not make us run out of memory.
            if c.events.len() > 500 {
                let extra = c.events.len() - 500;
                c.events.drain(0..extra);
            }
        }
        self.dirty = true;
    }

    /// People whose "next hand" vote counts: seated and connected. (Someone
    /// knocked out in the last hand still votes once, so they see how it ended.)
    pub(super) fn voters(&self) -> Vec<ClientId> {
        self.clients
            .iter()
            .filter(|(_, c)| c.connected && c.seat.is_some())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Advances time: bots move, timers run out, the next hand is dealt.
    pub fn tick(&mut self, now: Instant) {
        if self.paused.is_some() {
            if self.connected_people() > 1 {
                // Someone else came in: they must not wait for a window to close.
                self.set_paused(false, now);
                self.dirty = true;
            } else {
                return;
            }
        }
        let Some(state) = self.game.as_ref().map(|g| g.state) else {
            return;
        };
        match state {
            State::Betting => self.tick_betting(now),
            State::Runout => {
                if now >= self.next_step
                    && let Some(g) = self.game.as_mut()
                {
                    g.runout_step();
                    self.next_step =
                        now + self.delay() * self.settings.runout_pace_percent.max(10) / 100;
                }
            }
            State::HandOver => {
                if self.hand_over_since.is_none() {
                    self.hand_over_since = Some(now);
                    self.bot_rebuys();
                    self.dirty = true;
                }
                let voters = self.voters();
                let everyone_ready = voters.iter().all(|id| self.clients[id].ready);
                let timed_out = self.settings.next_hand_seconds > 0
                    && self.hand_over_since.is_some_and(|t| {
                        now.duration_since(t)
                            >= Duration::from_secs(u64::from(self.settings.next_hand_seconds))
                    });
                if everyone_ready || timed_out {
                    self.start_next_hand(now);
                }
            }
            State::GameOver => self.lobby_when_everyone_is_done(),
        }
        self.collect_events();
    }

    fn tick_betting(&mut self, now: Instant) {
        let people = self.connected_people();
        let Some(g) = self.game.as_ref() else {
            return;
        };
        let seat = g.to_act;
        let key = (g.hand_no, g.action_count);
        if self.turn.0 != key {
            self.turn = (key, now);
        }
        if !g.players[seat].is_human {
            if now >= self.next_step
                && let Some(g) = self.game.as_mut()
            {
                g.bot_move();
                self.next_step = now + self.delay();
            }
            return;
        }
        let connected = self.client_at(seat).is_some_and(|(_, c)| c.connected);
        let limit = if !connected {
            Some(Duration::from_secs(u64::from(self.settings.away_seconds)))
        } else if self.settings.turn_seconds > 0 && people > 1 {
            Some(Duration::from_secs(u64::from(self.settings.turn_seconds)))
        } else {
            None
        };
        if let Some(limit) = limit
            && now.duration_since(self.turn.1) >= limit
        {
            let action = if g.legal(seat).can_check {
                Action::Check
            } else {
                Action::Fold
            };
            if connected && let Some((id, _)) = self.client_at(seat) {
                self.notice(*id, "Time is up: you were checked or folded automatically.");
            }
            if let Some(g) = self.game.as_mut() {
                let _ = g.act(action);
                self.next_step = now + self.delay();
            }
        }
    }

    fn bot_rebuys(&mut self) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        for s in 0..g.players.len() {
            if g.players[s].rebuy_pending && !g.players[s].is_human {
                if bot::wants_rebuy(&g.players[s].personality, &mut self.rng) {
                    let _ = g.rebuy(s);
                } else {
                    g.decline_rebuy(s);
                }
            }
        }
    }
}
