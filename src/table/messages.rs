//! Messages from the clients: actions, ready, pause, rebuys.

use std::time::Instant;

use super::{ClientId, Table};
use crate::game::{Action, State};
use crate::protocol::ClientMsg;

impl Table {
    pub fn handle(&mut self, id: ClientId, msg: ClientMsg, now: Instant) {
        if let ClientMsg::Hello {
            name,
            version,
            min_version,
            ..
        } = &msg
        {
            self.hello(id, name, *version, *min_version);
            return;
        }
        let Some(client) = self.clients.get(&id) else {
            return;
        };
        if !client.joined {
            return;
        }
        let seat = client.seat;
        let owner = client.owner;
        match msg {
            ClientMsg::Hello { .. } => {}
            ClientMsg::Act { action, turn } => self.act(id, seat, action, turn),
            ClientMsg::Ready => self.ready(id, seat, now),
            ClientMsg::Rebuy(yes) => {
                if let (Some(g), Some(s)) = (self.game.as_mut(), seat) {
                    if yes {
                        if let Err(e) = g.rebuy(s) {
                            self.notice(id, e);
                        }
                    } else {
                        g.decline_rebuy(s);
                    }
                }
            }
            ClientMsg::Pause(pause) => {
                if owner && self.connected_people() <= 1 {
                    self.set_paused(pause, now);
                } else if pause {
                    self.notice(id, "The game cannot be paused while other people play.");
                }
            }
            ClientMsg::SetSpeed(ms) => {
                if owner {
                    self.settings.speed_ms = ms.clamp(20, 10_000);
                }
            }
        }
        self.dirty = true;
        self.collect_events();
    }

    /// Pauses or resumes. The time limits stand still while paused: they
    /// are moved on by however long the pause lasted.
    pub(super) fn set_paused(&mut self, pause: bool, now: Instant) {
        match (self.paused, pause) {
            (None, true) => self.paused = Some(now),
            (Some(since), false) => {
                let lasted = now.duration_since(since);
                self.turn.1 += lasted;
                if let Some(t) = self.hand_over_since.as_mut() {
                    *t += lasted;
                }
                self.next_step += lasted;
                self.paused = None;
            }
            _ => {}
        }
    }

    pub(super) fn act(&mut self, id: ClientId, seat: Option<usize>, action: Action, turn: u64) {
        let Some(g) = self.game.as_mut() else {
            return;
        };
        if turn != g.turn_id() {
            // Meant for an earlier decision (a key pressed twice, or crossed with a deal).
            return;
        }
        if g.state != State::Betting || Some(g.to_act) != seat {
            self.notice(id, "It is not your turn.");
            return;
        }
        if let Err(e) = g.act(action) {
            self.notice(id, e);
        }
    }

    pub(super) fn ready(&mut self, id: ClientId, seat: Option<usize>, now: Instant) {
        if let Some(c) = self.clients.get_mut(&id) {
            c.ready = match self.game.as_ref() {
                // In the lobby Enter toggles; elsewhere it confirms.
                None => !c.ready,
                Some(_) => true,
            };
        }
        match self.game.as_ref().map(|g| g.state) {
            None => self.try_start(now),
            Some(State::HandOver) => {
                // Saying "next hand" while a rebuy is offered declines it.
                if let (Some(g), Some(s)) = (self.game.as_mut(), seat) {
                    g.decline_rebuy(s);
                }
            }
            Some(State::GameOver) => self.lobby_when_everyone_is_done(),
            _ => {}
        }
    }

    /// After the final standings, back to the lobby once everyone still
    /// connected has seen them (also when the last one not ready leaves).
    pub(super) fn lobby_when_everyone_is_done(&mut self) {
        let mut people = self.clients.values().filter(|c| c.joined && c.connected);
        if people.all(|c| c.ready) {
            self.back_to_lobby();
        }
    }
}
