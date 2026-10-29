//! The final standings.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use holdem::game::ordinal;
use holdem::stats;

use super::style::{dim, framed, key_for, reminder, yellow};
use super::text::{fit, text_width, wrap_text};
use super::{boxed, fitted};
use crate::app::App;
use crate::keys::{Cmd, Place};
use crate::session::Kind;

/// "Rough guess from 7 hands: more hands make it more accurate (15 or more)."
pub(super) fn rough_note(hands: u32, min_hands: u32) -> String {
    format!(
        "Rough guess from {hands} hand{}: more hands make it more accurate ({min_hands} or more).",
        if hands == 1 { "" } else { "s" }
    )
}

/// "Tight-aggressive (84% match): tight, aggressive, bluffs sometimes".
pub(super) fn style_line(stats: &holdem::stats::Stats, min_hands: u32) -> String {
    match stats::profile(stats, min_hands) {
        Some(p) => {
            let mut text = format!(
                "{} ({}% match): {}",
                p.ranking[0].0, p.ranking[0].1, p.summary
            );
            if p.rough {
                text.push_str(&format!(". {}", rough_note(stats.hands, min_hands)));
            }
            text
        }
        None => format!(
            "not enough hands yet ({} of {} counted)",
            stats.hands,
            stats::ROUGH_HANDS.min(min_hands)
        ),
    }
}

/// The style column of a person's row: the closest bot style, marked as an
/// estimate ("≈ Shark", "≈ Shark?" while rough), or nothing yet.
fn estimated_style(stats: &holdem::stats::Stats, min_hands: u32) -> String {
    match stats::profile(stats, min_hands) {
        Some(p) if p.rough => format!("≈ {}?", p.ranking[0].0),
        Some(p) => format!("≈ {}", p.ranking[0].0),
        None => String::new(),
    }
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let Some(v) = app.view() else {
        return;
    };
    let my_name = app
        .session
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let mine = v.standings.iter().find(|s| s.is_human && s.name == my_name);
    let headline = match mine.and_then(|s| s.place) {
        Some(1) => "You won!".to_string(),
        Some(p) => format!(
            "You finished {} of {}",
            ordinal(p),
            v.standings.iter().filter(|s| s.place.is_some()).count()
        ),
        None => "Game over".to_string(),
    };
    let width = 80u16.min(area.width);
    let min_hands = app.config.client.style_min_hands;
    let inner_w = width.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(headline, yellow())),
        Line::from(""),
    ];
    // The rows form one block, centered as a whole so that the columns line up.
    let rows: Vec<(String, Style)> = v
        .standings
        .iter()
        .take(12)
        .map(|s| {
            let place = s.place.map(ordinal).unwrap_or_else(|| "-".to_string());
            let style_name = if s.is_human {
                estimated_style(&s.stats, min_hands)
            } else {
                s.style.clone()
            };
            let text = format!(
                "{:<5}{:<17}{:<19}{}",
                place,
                fit(&s.name, 16),
                style_name,
                s.status
            );
            let style = if s.is_human && s.name == my_name {
                yellow()
            } else {
                Style::new()
            };
            (text, style)
        })
        .collect();
    let block_w = rows.iter().map(|(t, _)| text_width(t)).max().unwrap_or(0);
    let indent = " ".repeat(inner_w.saturating_sub(block_w) / 2);
    for (text, style) in rows {
        lines.push(Line::from(Span::styled(format!("{indent}{text}"), style)).left_aligned());
    }
    if v.standings.len() > 12 {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", v.standings.len() - 12),
            dim(),
        )));
    }
    lines.push(Line::from(""));
    if let Some(m) = mine {
        let text = format!("Your style: {}", style_line(&m.stats, min_hands));
        for l in wrap_text(&text, inner_w.saturating_sub(2), 2) {
            lines.push(Line::from(Span::styled(l, Style::new().fg(Color::Cyan))));
        }
        lines.push(Line::from(""));
    }
    let solo = app.session.as_ref().is_some_and(|s| s.kind == Kind::Solo);
    let next = (
        key_for(Place::Over, Cmd::Select),
        if solo { "main menu" } else { "play again" },
    );
    lines.push(Line::from(reminder(&[next], Place::Over)));
    let block =
        framed(Line::from(" Final standings "), Color::Green).title_alignment(Alignment::Center);
    let (rect, lines) = fitted(area, width, lines);
    frame.render_widget(boxed(lines, block, rect).alignment(Alignment::Center), rect);
}
