//! Colours, frames and the key reminders shared by every screen.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};

use crate::keys::{self, Cmd, Place};

pub(super) fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub(super) fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub(super) fn yellow() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

pub(super) fn cyan() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub(super) fn green() -> Style {
    Style::new().fg(Color::Green)
}

/// A rectangle of at most `width` x `height` in the middle of `area`.
pub(super) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

pub(super) fn framed(title: Line<'static>, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(title)
}

pub(super) fn key(k: &str) -> Span<'static> {
    Span::styled(k.to_string(), yellow())
}

/// A key reminder: `lead` entries (key, words) for keys whose words depend on
/// the moment, then the place's reminder from the key table.
pub(super) fn reminder(lead: &[(String, &str)], place: Place) -> Vec<Span<'static>> {
    reminder_spans(lead.iter().cloned().chain(keys::reminder(place)))
}

/// Key reminder spans from (key, words) entries.
/// A space that line wrapping does not break at (see `text::wrap_lines`).
pub(super) const NO_BREAK: char = '\u{a0}';

pub(super) fn reminder_spans<'a>(
    entries: impl Iterator<Item = (String, &'a str)>,
) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (k, words)) in entries.enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(key(&k));
        // A no-break space: wrapping keeps a key and its word on one line.
        spans.push(Span::raw(format!("{NO_BREAK}{words}")));
    }
    spans
}

/// The key that does `cmd` in `place`, as shown on screen.
pub(super) fn key_for(place: Place, cmd: Cmd) -> String {
    keys::key(place, cmd)
}
