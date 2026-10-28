//! The join screen: your name, an address, and the tables found nearby.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::style::{bold, centered, dim, key_for, reminder, yellow};
use super::text::{fit, wrap_text};
use super::title_block;
use crate::app::App;
use crate::keys::{Cmd, Place};

fn input_line(label: &str, value: &str, active: bool) -> Line<'static> {
    let cursor = if active { "▏" } else { "" };
    let style = if active { yellow() } else { Style::new() };
    Line::from(vec![
        Span::raw(format!(" {} {:<10}", if active { "▶" } else { " " }, label)),
        Span::styled(format!("{value}{cursor}"), style),
    ])
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let j = &app.join;
    let height = (14 + j.found.len().min(6)) as u16;
    let rect = centered(area, 76, height);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        input_line("Your name", &j.name, j.row == 0),
        input_line("Address", &j.address, j.row == 1),
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
        let active = j.row == k + 2;
        let state = match f.info.compatibility() {
            Err(m) => m.about_table(),
            Ok(_) if f.info.playing => "playing".to_string(),
            Ok(_) => "waiting".to_string(),
        };
        let text = format!(
            " {} {}  {}  {} connected, {} seats, {}",
            if active { "▶" } else { " " },
            f.info.table,
            f.address,
            f.info.players,
            f.info.seats,
            state
        );
        let style = if active { yellow() } else { Style::new() };
        lines.push(Line::from(Span::styled(fit(&text, inner_w), style)));
    }
    lines.push(Line::from(""));
    let status = if app.connecting() {
        format!("Connecting… ({} cancels)", key_for(Place::Join, Cmd::Back))
    } else {
        j.status.clone()
    };
    for l in wrap_text(&status, inner_w.saturating_sub(2), 0)
        .into_iter()
        .take(2)
    {
        lines.push(Line::from(Span::styled(
            format!(" {l}"),
            Style::new().fg(Color::LightRed),
        )));
    }
    while lines.len() < (height as usize).saturating_sub(3) {
        lines.push(Line::from(""));
    }
    let mut spans = reminder(&[], Place::Join);
    spans.push(Span::raw("   type to edit"));
    lines.push(Line::from(spans));
    frame.render_widget(
        Paragraph::new(lines).block(title_block("Join a network game")),
        rect,
    );
}
