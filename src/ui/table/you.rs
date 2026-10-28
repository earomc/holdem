//! Your own seat: your cards, stack and hand.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use holdem::game::{State, chips_text};
use holdem::protocol::TableView;

use super::seats::seat_title;
use crate::app::App;
use crate::ui::cards::cards_line;
use crate::ui::style::{bold, dim, framed};

pub(super) fn draw_you(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let Some(me) = v.you else {
        let text = if app
            .session
            .as_ref()
            .is_some_and(|s| v.waiting.contains(&s.name))
        {
            "You are waiting for a free seat: you will be dealt in at the next hand."
        } else {
            "You are watching. You will play again in the next game."
        };
        let block = framed(Line::from(" You "), Color::DarkGray);
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
            area,
        );
        return;
    };
    let s = &v.seats[me];
    let acting = v.legal.is_some();
    let color = if s.out || s.folded {
        Color::DarkGray
    } else if v.state == State::HandOver && s.payout > 0 {
        Color::Green
    } else if acting {
        Color::Yellow
    } else {
        Color::White
    };
    let block = framed(seat_title(v, me), color);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if s.out && !s.rebuy_pending {
        lines.push(Line::from(Span::styled("You are out of the game", dim())));
    } else {
        if s.cards.len() == 2 {
            let mut spans = cards_line(&s.cards).spans;
            if let Some(d) = &v.your_hand {
                spans.push(Span::raw("   "));
                spans.push(Span::styled(d.clone(), bold()));
            }
            if s.folded {
                spans.push(Span::styled("   (folded)", dim()));
            }
            lines.push(Line::from(spans));
        }
        let mut info = format!(
            "Stack {}    In this round {}    {}",
            chips_text(s.chips),
            chips_text(s.bet),
            s.last_action
        );
        if s.rebuys > 0 {
            info.push_str(&format!("    Rebuys {}", s.rebuys));
        }
        lines.push(Line::from(info));
        let mut parts: Vec<Span<'static>> = Vec::new();
        if let Some(h) = app.current_hint() {
            parts.push(Span::styled(
                format!(
                    "Win chance vs {} random hand{}: about {:.0}%",
                    h.opponents,
                    if h.opponents == 1 { "" } else { "s" },
                    h.win * 100.0
                ),
                Style::new().fg(Color::Cyan),
            ));
        }
        if acting && let Some(secs) = v.turn_seconds_left {
            if !parts.is_empty() {
                parts.push(Span::raw("    "));
            }
            let style = if secs <= 5 {
                Style::new()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::Yellow)
            };
            parts.push(Span::styled(format!("{secs}s left to act"), style));
        }
        if !parts.is_empty() {
            lines.push(Line::from(parts));
        }
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}
