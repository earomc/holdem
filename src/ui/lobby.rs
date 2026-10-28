//! The lobby of a network table, before the game starts.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use holdem::game::chips_text;

use super::style::{bold, centered, cyan, dim, green, reminder};
use super::text::wrap_text;
use super::{boxed, screen_box, title_block};
use crate::app::App;
use crate::keys::Place;

/// One line per optional rule that is switched on.
fn rules_summary(rules: &holdem::game::Rules, fluctuation: bool) -> String {
    let mut parts = vec![
        format!("{} chips", chips_text(rules.starting_chips)),
        format!("blinds {}", rules.blinds_text()),
    ];
    if rules.hands_per_level > 0 {
        parts.push(format!("up every {} hands", rules.hands_per_level));
    }
    if rules.hand_limit > 0 {
        parts.push(format!("{} hands", rules.hand_limit));
    }
    if rules.antes {
        parts.push("antes".to_string());
    }
    if rules.straddle {
        parts.push("straddle".to_string());
    }
    if rules.rebuy_levels > 0 {
        parts.push("rebuys".to_string());
    }
    if rules.dead_button {
        parts.push("dead button".to_string());
    }
    if rules.muck_losers {
        parts.push("mucking".to_string());
    }
    if fluctuation {
        parts.push("players come and go".to_string());
    }
    parts.join(" · ")
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let Some(session) = app.session.as_ref() else {
        return;
    };
    let rect = centered(area, 76, 20);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = vec![Line::from("")];
    match &session.lobby {
        None => lines.push(Line::from(" Connecting to the table…")),
        Some(l) => {
            if let Some(addr) = &session.share {
                lines.push(Line::from(vec![
                    Span::raw(" Others join at  "),
                    Span::styled(addr.clone(), cyan()),
                ]));
                lines.push(Line::from(Span::styled(
                    " (on the same network; for internet games forward this port in your router)",
                    dim(),
                )));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!(" Players ({} of {} seats):", l.players.len(), l.seats),
                bold(),
            )));
            for p in &l.players {
                let mark = if p.ready {
                    "✔ ready"
                } else {
                    "… not ready"
                };
                let you = if p.you { "  (you)" } else { "" };
                let style = if p.ready { green() } else { Style::new() };
                lines.push(Line::from(vec![
                    Span::raw(format!("   {:<18}", p.name)),
                    Span::styled(mark, style),
                    Span::styled(you, dim()),
                ]));
            }
            lines.push(Line::from(""));
            let bots = if l.fill_with_bots {
                "Bots take the free seats."
            } else {
                "No bots: only people play."
            };
            lines.push(Line::from(Span::styled(format!(" {bots}"), dim())));
            for w in wrap_text(
                &rules_summary(&l.rules, l.fluctuation),
                inner_w.saturating_sub(2),
                0,
            ) {
                lines.push(Line::from(Span::styled(format!(" {w}"), dim())));
            }
        }
    }
    let footer = vec![
        Line::from(" The game starts when everyone is ready."),
        Line::from(reminder(&[], Place::Lobby)),
    ];
    let (rect, lines) = screen_box(area, 76, 20, lines, footer);
    let title = if session.table.is_empty() {
        "Lobby".to_string()
    } else {
        session.table.clone()
    };
    frame.render_widget(boxed(lines, title_block(&title, rect), rect), rect);
}
