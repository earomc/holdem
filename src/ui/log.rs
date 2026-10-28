//! The hand log, as shown at the table and in the log window.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::text::wrap_text;

fn log_style(text: &str) -> Style {
    if text.starts_with("---") {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if text.contains(" wins ") || text.contains(" split ") {
        Style::new().fg(Color::Green)
    } else if text.contains("eliminated") || text.contains("busted") {
        Style::new().fg(Color::Red)
    } else if text.starts_with("Showdown") || text.contains(" shows ") || text.contains(" mucks") {
        Style::new().fg(Color::Magenta)
    } else if text.starts_with("Flop") || text.starts_with("Turn") || text.starts_with("River") {
        Style::new().fg(Color::Yellow)
    } else if text.contains(" joins ") || text.contains(" leaves ") || text.contains(" rebuys ") {
        Style::new().fg(Color::LightBlue)
    } else {
        Style::new()
    }
}

/// The log wrapped to `width` columns. Returns the `height` lines that end
/// `scroll` lines above the newest one, and the largest useful `scroll`.
pub(super) fn log_view(
    log: &[String],
    height: usize,
    width: usize,
    scroll: usize,
) -> (Vec<Line<'static>>, usize) {
    let mut all: Vec<Line<'static>> = Vec::new();
    for entry in log {
        let style = log_style(entry);
        for piece in wrap_text(entry, width, 2) {
            all.push(Line::from(Span::styled(piece, style)));
        }
    }
    let max = all.len().saturating_sub(height);
    let scroll = scroll.min(max);
    let end = all.len() - scroll;
    let start = end.saturating_sub(height);
    let shown: Vec<Line<'static>> = all.drain(start..end).collect();
    (shown, max)
}
