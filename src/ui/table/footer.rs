//! The footer: whose turn it is, your actions and the messages.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use holdem::game::{State, chips_text};
use holdem::protocol::TableView;

use crate::app::App;
use crate::keys::{self, Cmd, Place};
use crate::ui::style::{dim, green, key, key_for, yellow};
use crate::ui::text::{fit, wrap_lines};

pub(super) fn action_spans(v: &TableView) -> Vec<Span<'static>> {
    let Some(l) = v.legal else {
        return Vec::new();
    };
    let k = |cmd: Cmd| Span::styled(format!("[{}]", key_for(Place::Turn, cmd)), yellow());
    let mut out: Vec<Span<'static>> = Vec::new();
    if !l.can_check {
        out.push(k(Cmd::Fold));
        out.push(Span::raw(" Fold   "));
    }
    out.push(k(Cmd::Call));
    out.push(Span::raw(if l.can_check {
        " Check   ".to_string()
    } else {
        format!(" Call {}   ", chips_text(l.to_call))
    }));
    if l.can_raise {
        out.push(k(Cmd::Raise));
        let word = if v.current_bet == 0 {
            "Bet"
        } else {
            "Raise to"
        };
        out.push(Span::raw(format!(
            " {} (min {})   ",
            word,
            chips_text(l.min_to)
        )));
    }
    if l.can_go_all_in() {
        out.push(k(Cmd::AllIn));
        out.push(Span::raw(format!(" All-in {}", chips_text(l.stack))));
    }
    out
}

/// The three lines inside the bottom panel, each fitted to `width` columns.
pub(super) fn footer_lines(app: &App, v: &TableView, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    match v.state {
        State::HandOver | State::GameOver => {
            if let Some(first) = v.result_lines.first() {
                lines.push(Line::from(Span::styled(
                    fit(first, width),
                    green().add_modifier(Modifier::BOLD),
                )));
            }
            if v.result_lines.len() > 1 {
                let mut text = v.result_lines[1].clone();
                if v.result_lines.len() > 2 {
                    text = format!("{}  (+{} more in the log)", text, v.result_lines.len() - 2);
                }
                lines.push(Line::from(Span::styled(fit(&text, width), green())));
            }
            let mut spans: Vec<Span<'static>> = Vec::new();
            if let Some(chips) = v.rebuy_offer {
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Rebuy))));
                spans.push(Span::raw(format!(
                    " Rebuy for {} chips   ",
                    chips_text(chips)
                )));
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Ready))));
                spans.push(Span::raw(" Give up"));
            } else if v.you_ready {
                spans.push(Span::styled(
                    format!(
                        "Waiting for the others ({} of {} ready)",
                        v.ready.0, v.ready.1
                    ),
                    Style::new().fg(Color::Yellow),
                ));
            } else {
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Ready))));
                spans.push(Span::raw(if v.last_hand {
                    " Final results"
                } else {
                    " Next hand"
                }));
                if v.ready.1 > 1 {
                    spans.push(Span::styled(
                        format!("   ({} of {} ready)", v.ready.0, v.ready.1),
                        dim(),
                    ));
                }
            }
            if let Some(secs) = v.next_hand_seconds_left {
                spans.push(Span::styled(format!("   · starts in {secs}s"), dim()));
            }
            lines.push(Line::from(spans));
        }
        State::Runout => {
            lines.push(Line::from(Span::styled(
                "Everyone is all-in: the rest of the board is being dealt…",
                Style::new().fg(Color::Yellow),
            )));
        }
        State::Betting => {
            if let Some(l) = v.legal {
                lines.push(Line::from(action_spans(v)));
                let mut s = format!("Pot {}", chips_text(v.pot));
                if l.to_call > 0 {
                    let odds = 100.0 * l.to_call as f32 / (v.pot + l.to_call) as f32;
                    s.push_str(&format!(
                        "   ·   To call {}   ·   Pot odds {:.0}%",
                        chips_text(l.to_call),
                        odds
                    ));
                } else {
                    s.push_str("   ·   Nothing to call");
                }
                lines.push(Line::from(Span::styled(
                    fit(&s, width),
                    Style::new().fg(Color::Cyan),
                )));
            } else if let Some(actor) = v.to_act {
                let who = &v.seats[actor].name;
                let folded = v.you.is_some_and(|s| v.seats[s].folded);
                let prefix = if folded {
                    "You folded this hand.  "
                } else {
                    ""
                };
                let mut text = format!("{prefix}{who} is thinking…");
                if let Some(secs) = v.turn_seconds_left {
                    text.push_str(&format!(" ({secs}s left)"));
                }
                lines.push(Line::from(Span::styled(
                    fit(&text, width),
                    Style::new().fg(Color::Yellow),
                )));
            }
        }
    }

    // The third line: a confirmation request, a status message, or the key reminder.
    let extra = if app.all_in_armed {
        let stack = v.legal.map(|l| l.stack).unwrap_or(0);
        Some(Line::from(Span::styled(
            fit(
                &format!(
                    "Press {} again to put all {} chips in; any other key cancels.",
                    key_for(Place::Turn, Cmd::AllIn),
                    chips_text(stack)
                ),
                width,
            ),
            Style::new()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )))
    } else {
        app.toast_text().map(|t| {
            Line::from(Span::styled(
                t.to_string(),
                Style::new().fg(Color::LightRed),
            ))
        })
    };
    if let Some(line) = extra {
        // A message is shown whole: the oldest lines make room for it.
        let message: Vec<Line<'static>> =
            wrap_lines(vec![line], width).into_iter().take(3).collect();
        while lines.len() + message.len() > 3 {
            lines.remove(0);
        }
        lines.extend(message);
    } else if lines.len() < 3 {
        lines.push(Line::from(Span::styled(
            fit(&keys::reminder_text(Place::Table), width),
            dim(),
        )));
    }
    lines
}
