//! The join screen: your name, an address, and the tables found nearby.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use holdem::protocol::Announcement;

use super::avatar::avatar;
use super::style::{bold, centered, dim, key_for, reminder, yellow};
use super::text::wrap_text;
use super::{boxed, screen_box, title_block};
use crate::app::{App, JOIN_ADDRESS, JOIN_FOUND, JOIN_NAME, JOIN_SYMBOL};
use crate::keys::{Cmd, Place};

fn input_line(label: &str, value: &str, active: bool) -> Line<'static> {
    let cursor = if active { "▏" } else { "" };
    let style = if active { yellow() } else { Style::new() };
    Line::from(vec![
        Span::raw(format!(" {} {:<10}", if active { "▶" } else { " " }, label)),
        Span::styled(format!("{value}{cursor}"), style),
    ])
}

/// Your symbol, in its color, with its color's name.
fn symbol_line(app: &App, active: bool) -> Line<'static> {
    let s = app.shown_symbol();
    let style = if active { yellow() } else { Style::new() };
    Line::from(vec![
        Span::raw(format!(
            " {} {:<10}",
            if active { "▶" } else { " " },
            "Symbol"
        )),
        avatar(&app.join.name, Some(s)),
        Span::styled(format!(" {}", s.color_name()), style),
    ])
}

/// One table in a list: its name (padded to `name_width`), where it is, how
/// full, and whether it plays.
pub(super) fn table_line(
    info: &Announcement,
    address: Option<&str>,
    active: bool,
    name_width: usize,
) -> Line<'static> {
    let state = match info.compatibility() {
        Err(m) => m.about_table(),
        Ok(_) if info.playing => "playing".to_string(),
        Ok(_) => "waiting".to_string(),
    };
    let place = address.map(|a| format!("  {a}")).unwrap_or_default();
    let text = format!(
        " {} {:<name_width$}{}  {} connected, {} seats, {}",
        if active { "▶" } else { " " },
        info.table,
        place,
        info.players,
        info.seats,
        state
    );
    let style = if active { yellow() } else { Style::new() };
    Line::from(Span::styled(text, style))
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let j = &app.join;
    let height = (14 + j.found.len().min(6)) as u16;
    let rect = centered(area, 76, height);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        input_line("Your name", &j.name, j.row == JOIN_NAME),
        symbol_line(app, j.row == JOIN_SYMBOL),
        input_line("Address", &j.address, j.row == JOIN_ADDRESS),
        Line::from(Span::styled(
            "              e.g. 192.168.1.20, or myserver.net:7777",
            dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(" Tables on your network:", bold())),
    ];
    if j.found.is_empty() {
        lines.push(Line::from(Span::styled(
            "   none found yet (searching…)",
            dim(),
        )));
    }
    for (k, f) in j.found.iter().enumerate().take(6) {
        let place = f.address.to_string();
        lines.push(table_line(
            &f.info,
            Some(&place),
            j.row == k + JOIN_FOUND,
            0,
        ));
    }
    lines.push(Line::from(""));
    let status = if app.connecting() {
        format!("Connecting… ({} cancels)", key_for(Place::Join, Cmd::Back))
    } else {
        j.status.clone()
    };
    for l in wrap_text(&status, inner_w.saturating_sub(2), 0) {
        lines.push(Line::from(Span::styled(
            format!(" {l}"),
            Style::new().fg(Color::LightRed),
        )));
    }
    let mut spans = reminder(&[], Place::Join);
    if j.row == JOIN_SYMBOL {
        spans.push(Span::raw("   Enter or ← → choose your symbol"));
    } else {
        spans.push(Span::raw("   type to edit"));
    }
    let (rect, lines) = screen_box(area, 76, height, lines, vec![Line::from(spans)]);
    frame.render_widget(
        boxed(lines, title_block("Join a network game", rect), rect),
        rect,
    );
}
