//! What each client is sent: the lobby or its own view of the table.

use std::time::{Duration, Instant};

use super::{ClientId, Table};
use crate::eval;
use crate::game::State;
use crate::player::symbol::Symbol;
use crate::protocol::{LobbyPlayer, LobbyView, SeatView, ServerMsg, TableView};

impl Table {
    fn seconds_left(&self, now: Instant) -> (Option<u32>, Option<u32>) {
        let Some(g) = self.game.as_ref() else {
            return (None, None);
        };
        let left = |since: Instant, total: u32| {
            let end = since + Duration::from_secs(u64::from(total));
            end.saturating_duration_since(now).as_secs() as u32
        };
        let turn = if g.state == State::Betting
            && g.players[g.to_act].is_human
            && self.settings.turn_seconds > 0
            && self.connected_people() > 1
        {
            Some(left(self.turn.1, self.settings.turn_seconds))
        } else {
            None
        };
        let next = match (g.state, self.hand_over_since) {
            (State::HandOver, Some(t)) if self.settings.next_hand_seconds > 0 => {
                Some(left(t, self.settings.next_hand_seconds))
            }
            _ => None,
        };
        (turn, next)
    }

    /// Sends fresh views to every client if anything changed.
    pub fn flush(&mut self, now: Instant) {
        let seconds = self.seconds_left(now);
        if seconds != self.shown_seconds {
            self.shown_seconds = seconds;
            self.dirty = true;
        }
        self.clients.retain(|_, c| !c.drop);
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let ids: Vec<ClientId> = self.clients.keys().copied().collect();
        for id in ids {
            if !self.clients[&id].joined {
                continue;
            }
            let msg = match self.game.is_some() {
                false => ServerMsg::Lobby(self.lobby_view(id)),
                true => ServerMsg::Table(Box::new(self.table_view(id, seconds))),
            };
            let _ = self.clients[&id].outbox.send(msg);
        }
    }

    fn lobby_view(&self, id: ClientId) -> LobbyView {
        LobbyView {
            table: self.settings.name.clone(),
            players: self
                .clients
                .iter()
                .filter(|(_, c)| c.joined && c.connected)
                .map(|(other, c)| LobbyPlayer {
                    name: c.name.clone(),
                    ready: c.ready,
                    you: *other == id,
                    symbol: c.symbol,
                })
                .collect(),
            seats: self.settings.seats,
            fill_with_bots: self.settings.fill_with_bots,
            fluctuation: self.settings.fluctuation.is_some(),
            rules: self.settings.rules.clone(),
            win_chance_allowed: self.settings.allow_win_chance,
        }
    }

    fn table_view(&mut self, id: ClientId, seconds: (Option<u32>, Option<u32>)) -> TableView {
        let voters = self.voters();
        let ready_count = voters.iter().filter(|v| self.clients[*v].ready).count();
        let people = self.connected_people();
        let waiting: Vec<String> = self
            .clients
            .values()
            .filter(|c| c.joined && c.connected && c.seat.is_none() && !c.spectator)
            .map(|c| c.name.clone())
            .collect();
        let symbols: Vec<(usize, Symbol)> = self
            .clients
            .values()
            .filter_map(|c| Some((c.seat?, c.symbol?)))
            .collect();
        let connected_seats: Vec<(usize, bool)> = self
            .clients
            .values()
            .filter_map(|c| c.seat.map(|s| (s, c.connected)))
            .collect();
        let (paused, speed_ms) = (self.paused.is_some(), self.settings.speed_ms);
        let g = self.game.as_ref().expect("a game is running");
        let client = self.clients.get_mut(&id).expect("the client exists");
        let you = client.seat;
        let (new_log, next_seq) = g.log_since(client.log_seq);
        let new_log = new_log.to_vec();
        client.log_seq = next_seq;
        let events = std::mem::take(&mut client.events);

        let seats: Vec<SeatView> = g
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let visible = (Some(i) == you || p.shown) && p.in_hand;
                SeatView {
                    name: p.name.clone(),
                    is_human: p.is_human,
                    symbol: symbols.iter().find(|(s, _)| *s == i).map(|(_, sym)| *sym),
                    connected: connected_seats
                        .iter()
                        .find(|(s, _)| *s == i)
                        .map(|(_, c)| *c)
                        .unwrap_or(true),
                    chips: p.chips,
                    bet: p.bet,
                    in_hand: p.in_hand,
                    folded: p.folded,
                    all_in: p.all_in,
                    out: p.out,
                    vacant: p.vacant,
                    place: g.place_of(i),
                    last_action: p.last_action.clone(),
                    cards: if visible { p.hole.clone() } else { Vec::new() },
                    hidden_cards: !visible && p.in_hand && !p.folded && p.hole.len() == 2,
                    payout: p.payout,
                    rebuy_pending: p.rebuy_pending,
                    rebuys: p.rebuys,
                }
            })
            .collect();
        let your_hand = you.and_then(|s| {
            let p = &g.players[s];
            if p.hole.len() != 2 || !p.in_hand {
                None
            } else if g.board.len() >= 3 {
                g.made_hand(s)
            } else {
                Some(eval::describe_hole(p.hole[0], p.hole[1]))
            }
        });
        let betting = g.state == State::Betting;
        TableView {
            you,
            seats,
            board: g.board.clone(),
            state: g.state,
            street: g.street,
            pot: g.pot(),
            pots: g.pots_now(),
            last_pot: g.last_pot,
            hand_no: g.hand_no,
            level: g.level,
            sb: g.sb,
            bb: g.bb,
            ante: g.ante,
            rules: g.rules.clone(),
            button: g.button,
            sb_seat: g.sb_seat,
            sb_dead: g.sb_dead,
            bb_seat: g.bb_seat,
            straddle_seat: g.straddle_seat,
            to_act: if betting { Some(g.to_act) } else { None },
            current_bet: g.current_bet,
            legal: match you {
                Some(s) if betting && g.to_act == s => Some(g.legal(s)),
                _ => None,
            },
            turn: g.turn_id(),
            your_hand,
            result_lines: g.result_lines.clone(),
            new_log,
            events,
            rebuy_offer: match you {
                Some(s) if g.players[s].rebuy_pending => Some(g.rules.rebuy_stack()),
                _ => None,
            },
            last_hand: g.state == State::HandOver && g.tournament_over(),
            game_over: g.state == State::GameOver,
            standings: if g.state == State::GameOver {
                g.standings()
            } else {
                Vec::new()
            },
            your_stats: you.map(|s| g.players[s].stats).unwrap_or_default(),
            your_result: you.and_then(|s| g.players[s].last_result.clone()),
            turn_seconds_left: seconds.0,
            next_hand_seconds_left: seconds.1,
            ready: (ready_count, voters.len()),
            you_ready: client.ready,
            waiting,
            paused,
            speed_ms,
            owner: client.owner,
            multiplayer: people > 1,
            win_chance_allowed: self.settings.allow_win_chance,
            stats_min_stack_bb: self.settings.stats_min_stack_bb,
        }
    }
}
