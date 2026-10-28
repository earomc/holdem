//! The main menu.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::style::{dim, reminder, yellow};
use super::{boxed, fitted, title_block};
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
    // Wrapped like everything else: the hint at the end and a long file path both matter.
    lines.push(Line::from(Span::styled(app.config_note.clone(), dim())));
    let (rect, lines) = fitted(area, 60, lines);
    frame.render_widget(
        boxed(lines, title_block("♠ ♥  TEXAS HOLD'EM  ♦ ♣", rect), rect)
            .alignment(Alignment::Center),
        rect,
    );
}
