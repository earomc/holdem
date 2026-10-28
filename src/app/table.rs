//! Keys at the table: your actions and the raise box.

use ratatui::crossterm::event::KeyCode;

use holdem::game::{Action, State, chips_text};
use holdem::protocol::ClientMsg;

use super::{App, MAX_AMOUNT_DIGITS, Outcome, RaisePrompt, pot_raise_to};
use crate::keys::{self, Cmd, Place};

impl App {
    /// Answers the current decision, once: repeated presses (or a key held
    /// down) are ignored until the table asks again.
    pub(super) fn act(&mut self, action: Action) {
        let Some(turn) = self.view().filter(|v| v.legal.is_some()).map(|v| v.turn) else {
            return;
        };
        if self.answered == Some(turn) {
            return;
        }
        self.answered = Some(turn);
        self.send(ClientMsg::Act { action, turn });
    }

    pub(super) fn send(&self, msg: ClientMsg) {
        if let Some(s) = self.session.as_ref() {
            s.send(msg);
        }
    }

    pub(super) fn table_key(&mut self, code: KeyCode) {
        let armed = std::mem::take(&mut self.all_in_armed);
        if self.raise.is_some() {
            self.raise_key(code);
            return;
        }
        if let Some(cmd) = keys::command(Place::Table, code) {
            return self.common(cmd);
        }
        let Some(v) = self.view() else {
            return;
        };
        if v.state == State::HandOver {
            match keys::command(Place::HandOver, code) {
                Some(Cmd::Ready) if !v.you_ready => self.send(ClientMsg::Ready),
                Some(Cmd::Rebuy) if v.rebuy_offer.is_some() => self.send(ClientMsg::Rebuy(true)),
                _ => {}
            }
            return;
        }
        let Some(legal) = v.legal else {
            return;
        };
        if self.answered == Some(v.turn) {
            // Already answered; waiting for the table.
            return;
        }
        match keys::command(Place::Turn, code) {
            Some(Cmd::Fold) => {
                if legal.can_check {
                    self.notify(format!(
                        "Checking is free, so folding is not offered. Press {} to check.",
                        keys::key(Place::Turn, Cmd::Call)
                    ));
                } else {
                    self.act(Action::Fold);
                }
            }
            Some(Cmd::Call) => self.act(Action::Call),
            Some(Cmd::Raise) => {
                if legal.can_raise {
                    self.raise = Some(RaisePrompt {
                        text: legal.min_to.to_string(),
                        fresh: true,
                        error: None,
                    });
                } else {
                    self.notify(
                        "You cannot raise here: the betting was not reopened, or nobody can respond.",
                    );
                }
            }
            Some(Cmd::AllIn) => {
                if !legal.can_go_all_in() {
                    self.notify("All-in is not possible here: you can only check or call.");
                } else if armed {
                    self.act(Action::AllIn);
                } else {
                    self.all_in_armed = true;
                }
            }
            _ => {}
        }
    }

    fn raise_key(&mut self, code: KeyCode) {
        let outcome = {
            let Some(v) = self.session.as_ref().and_then(|s| s.view.as_ref()) else {
                self.raise = None;
                return;
            };
            let Some(prompt) = self.raise.as_mut() else {
                return;
            };
            let Some(legal) = v.legal else {
                self.raise = None;
                return;
            };
            let bb = v.bb.max(1);
            let current: u32 = prompt.text.parse().unwrap_or(legal.min_to);
            let set = |prompt: &mut RaisePrompt, value: u32| {
                prompt.text = value.to_string();
                prompt.fresh = false;
                prompt.error = None;
                Outcome::Keep
            };
            match keys::command(Place::Raise, code) {
                Some(Cmd::Back) => Outcome::Close,
                Some(Cmd::Select) => match prompt.text.parse::<u32>() {
                    Err(_) => {
                        prompt.error = Some("Type an amount first.".to_string());
                        Outcome::Keep
                    }
                    Ok(x) if x < legal.min_to => {
                        prompt.error = Some(format!(
                            "The smallest raise is to {}.",
                            chips_text(legal.min_to)
                        ));
                        Outcome::Keep
                    }
                    Ok(x) if x > legal.max_to => {
                        prompt.error = Some(format!(
                            "You only have {} to bet.",
                            chips_text(legal.max_to)
                        ));
                        Outcome::Keep
                    }
                    Ok(x) => Outcome::Submit(x),
                },
                Some(Cmd::Erase) => {
                    prompt.text.pop();
                    prompt.fresh = false;
                    prompt.error = None;
                    Outcome::Keep
                }
                Some(Cmd::Up) => set(prompt, (current.saturating_add(bb)).min(legal.max_to)),
                Some(Cmd::Down) => set(prompt, current.saturating_sub(bb).max(legal.min_to)),
                Some(Cmd::MinRaise) => set(prompt, legal.min_to),
                Some(Cmd::HalfPot) => set(prompt, pot_raise_to(v, 0.5)),
                Some(Cmd::Pot) => set(prompt, pot_raise_to(v, 1.0)),
                Some(Cmd::MaxRaise) => set(prompt, legal.max_to),
                _ => {
                    if let KeyCode::Char(c) = code
                        && c.is_ascii_digit()
                    {
                        if prompt.fresh {
                            prompt.text.clear();
                            prompt.fresh = false;
                        }
                        if prompt.text.len() < MAX_AMOUNT_DIGITS {
                            prompt.text.push(c);
                        }
                        prompt.error = None;
                    }
                    Outcome::Keep
                }
            }
        };
        match outcome {
            Outcome::Keep => {}
            Outcome::Close => self.raise = None,
            Outcome::Submit(amount) => {
                self.raise = None;
                self.act(Action::RaiseTo(amount));
            }
        }
    }
}
