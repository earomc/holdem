//! The main menu.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{centered, dim, reminder, yellow};
use super::text::wrap_text;
use super::title_block;
use crate::app::{App, MenuItem};
use crate::keys::Place;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No-limit Texas Hold'em in the terminal",
            dim(),
        )),
        Line::from(""),
    ];
    for (i, item) in MenuItem::ALL.iter().enumerate() {
        let (text, style) = if i == app.menu_row {
            (format!("▶ {} ◀", item.label()), yellow())
        } else {
            (format!("  {}  ", item.label()), Style::new())
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::Menu)));
    // Wrapped, not cut: the hint at the end and a long file path both matter.
    for l in wrap_text(&app.config_note, 56, 0) {
        lines.push(Line::from(Span::styled(l, dim())));
    }
    let rect = centered(area, 60, lines.len() as u16 + 2);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(title_block("♠ ♥  TEXAS HOLD'EM  ♦ ♣")),
        rect,
    );
}
