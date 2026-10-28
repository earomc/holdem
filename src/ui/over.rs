//! The final standings.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use holdem::game::ordinal;
use holdem::stats;

use super::style::{dim, framed, key_for, reminder, yellow};
use super::text::{fit, wrap_text};
use super::{boxed, fitted};
use crate::app::App;
use crate::keys::{Cmd, Place};
use crate::session::Kind;

/// "Tight-aggressive (84% match): tight, aggressive, bluffs sometimes".
pub(super) fn style_line(stats: &holdem::stats::Stats, min_hands: u32) -> String {
    match stats::profile(stats, min_hands) {
        Some(p) => format!(
            "{} ({}% match): {}",
            p.ranking[0].0, p.ranking[0].1, p.summary
        ),
        None => format!(
            "not enough hands yet ({} of {} counted)",
            stats.hands, min_hands
        ),
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
    let width = 72u16.min(area.width);
    let inner_w = width.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(headline, yellow())),
        Line::from(""),
    ];
    for s in v.standings.iter().take(12) {
        let place = s.place.map(ordinal).unwrap_or_else(|| "-".to_string());
        let style_name = if s.is_human {
            "(person)".to_string()
        } else {
            s.style.clone()
        };
        let text = format!(
            "{:<5}{:<17}{:<18}{}",
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
        lines.push(Line::from(Span::styled(text, style)));
    }
    if v.standings.len() > 12 {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", v.standings.len() - 12),
            dim(),
        )));
    }
    lines.push(Line::from(""));
    if let Some(m) = mine {
        let text = format!(
            "Your style: {}",
            style_line(&m.stats, app.config.client.style_min_hands)
        );
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
