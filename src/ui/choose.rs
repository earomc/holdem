//! Choosing one of the tables at a server with several.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use super::join::table_line;
use super::style::{bold, reminder};
use super::text::text_width;
use super::{boxed, fitted, title_block};
use crate::app::App;
use crate::keys::Place;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let tables = app
        .session
        .as_ref()
        .and_then(|s| s.choices.clone())
        .unwrap_or_default();
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " This server has {} tables. Which one do you want to join?",
                tables.len()
            ),
            bold(),
        )),
        Line::from(""),
    ];
    let name_width = tables
        .iter()
        .map(|t| text_width(&t.table))
        .max()
        .unwrap_or(0);
    for (k, t) in tables.iter().enumerate() {
        lines.push(table_line(t, None, k == app.choose_row, name_width));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::Choose)));
    let (rect, lines) = fitted(area, 76, lines);
    frame.render_widget(
        boxed(lines, title_block("Choose a table", rect), rect),
        rect,
    );
}
