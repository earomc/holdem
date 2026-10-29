//! The window for choosing your symbol: every shape and colour at once.
//! Shapes not unlocked yet are hidden behind a block.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Clear;

use holdem::player::symbol::{COLORS, GLYPHS, Symbol};

use super::avatar::avatar;
use super::popups::window;
use super::style::{bold, dim, reminder, yellow};
use super::{boxed, fitted};
use crate::app::{App, PICKER_COLUMNS, Picker};
use crate::keys::Place;

/// What a locked shape is drawn as.
const LOCKED: &str = "▒";

/// One cell: the content between spaces, or between brackets under the cursor.
fn cell(content: Span<'static>, under_cursor: bool) -> Vec<Span<'static>> {
    let edge = |s: &'static str| {
        if under_cursor {
            Span::styled(s, yellow().add_modifier(Modifier::BOLD))
        } else {
            Span::raw(" ")
        }
    };
    vec![edge("["), content, edge("]")]
}

fn shape_rows(app: &App, p: &Picker) -> Vec<Line<'static>> {
    let open = app.available_glyphs();
    let mut lines = Vec::new();
    for row in GLYPHS.chunks(PICKER_COLUMNS).enumerate() {
        let (r, glyphs) = row;
        let mut spans = vec![Span::raw("  ")];
        for (c, glyph) in glyphs.iter().enumerate() {
            let i = r * PICKER_COLUMNS + c;
            let content = if open.contains(&(i as u8)) {
                let style = if i == usize::from(p.preview.glyph) {
                    bold()
                } else {
                    Style::new()
                };
                Span::styled(*glyph, style)
            } else {
                Span::styled(LOCKED, dim())
            };
            spans.extend(cell(content, p.cursor == i));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn color_row(app: &App, p: &Picker) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for c in 0..COLORS.len() {
        let s = Symbol {
            glyph: p.preview.glyph,
            color: c as u8,
        };
        let on_it = p.color() == Some(c as u8);
        spans.extend(cell(avatar(&app.join.name, Some(s)), on_it));
    }
    Line::from(spans)
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let Some(p) = app.picker else {
        return;
    };
    let mut lines = vec![Line::from(""), Line::from(Span::styled(" Shape", bold()))];
    lines.extend(shape_rows(app, &p));
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(" Colour", bold())));
    lines.push(color_row(app, &p));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        avatar(&app.join.name, Some(p.preview)),
        Span::styled(format!(" {}", app.join.name.trim()), bold()),
        Span::styled(format!("   {}", p.preview.color_name()), dim()),
    ]));
    match app.picker_locked_note() {
        Some(note) => lines.push(Line::from(Span::styled(
            format!("  {note}"),
            Style::new().fg(Color::LightRed),
        ))),
        None => lines.push(Line::from("")),
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::SymbolPicker)));
    let (rect, lines) = fitted(area, 46, lines);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        boxed(
            lines,
            window(" Your symbol ".to_string(), Color::Cyan),
            rect,
        ),
        rect,
    );
}
