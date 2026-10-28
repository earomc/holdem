//! The middle of the table: the board and the pot.

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph};

use holdem::game::{State, chips_text};
use holdem::protocol::TableView;

use crate::app::App;
use crate::ui::cards::board_line;
use crate::ui::style::{bold, dim, framed, green};
use crate::ui::text::{fit, text_width};

/// Adds blank lines above `lines` so they sit in the middle of a box `area` tall.
pub(super) fn centre_vertically(mut lines: Vec<Line<'static>>, area: Rect) -> Vec<Line<'static>> {
    let inner = area.height.saturating_sub(2) as usize;
    let pad = inner.saturating_sub(lines.len()) / 2;
    let mut out = vec![Line::from(""); pad];
    out.append(&mut lines);
    out
}

/// Three-line block-character digits.
pub(super) const DIGITS: [[&str; 3]; 10] = [
    ["█▀█", "█ █", "█▄█"],
    ["▀█ ", " █ ", "▄█▄"],
    ["▀▀█", "█▀▀", "█▄▄"],
    ["▀▀█", " ▀█", "▄▄█"],
    ["█ █", "▀▀█", "  █"],
    ["█▀▀", "▀▀█", "▄▄█"],
    ["█▀▀", "█▀█", "█▄█"],
    ["▀▀█", "  █", "  █"],
    ["█▀█", "█▀█", "█▄█"],
    ["█▀█", "▀▀█", "▄▄█"],
];

/// `text` (digits and commas) drawn three lines tall.
pub(super) fn big_number(text: &str) -> [String; 3] {
    let mut rows = [String::new(), String::new(), String::new()];
    for (i, c) in text.chars().enumerate() {
        if i > 0 {
            for r in &mut rows {
                r.push(' ');
            }
        }
        match c.to_digit(10) {
            Some(d) => {
                for (r, part) in rows.iter_mut().zip(DIGITS[d as usize]) {
                    r.push_str(part);
                }
            }
            None => {
                rows[0].push(' ');
                rows[1].push(' ');
                rows[2].push('▄');
            }
        }
    }
    rows
}

pub(super) fn draw_board(frame: &mut Frame, v: &TableView, area: Rect) {
    let block = framed(
        Line::from(vec![
            Span::raw(" "),
            Span::styled(v.street.name(), bold()),
            Span::raw(" "),
        ]),
        Color::Green,
    );
    let inner_w = area.width.saturating_sub(2) as usize;
    let hpl = v.rules.hands_per_level;
    let mut level = format!("Blinds {}/{}", chips_text(v.sb), chips_text(v.bb));
    if v.ante > 0 {
        level.push_str(&format!(" ante {}", v.ante));
    }
    level.push_str(&format!("  ·  Level {}", v.level));
    if hpl > 0 && v.hand_no > 0 {
        let left = hpl - (v.hand_no - 1) % hpl;
        level.push_str(&format!(
            ", up in {} hand{}",
            left,
            if left == 1 { "" } else { "s" }
        ));
    }
    let bet = if v.state == State::Betting && v.current_bet > 0 {
        format!("Bet to match {}", chips_text(v.current_bet))
    } else {
        String::new()
    };
    let lines = centre_vertically(
        vec![
            board_line(&v.board),
            Line::from(""),
            Line::from(Span::styled(fit(&level, inner_w), dim())),
            Line::from(Span::styled(bet, dim())),
        ],
        area,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

/// The pot measured in big blinds, e.g. "worth 9.5 big blinds": how big it is
/// compared with what it costs to play a hand.
pub(super) fn big_blinds_text(amount: u32, bb: u32) -> String {
    let bbs = f64::from(amount) / f64::from(bb.max(1));
    let number = if bbs < 10.0 && bbs.fract() != 0.0 {
        format!("{bbs:.1}")
    } else {
        format!("{}", bbs.round() as u64)
    };
    let unit = if number == "1" {
        "big blind"
    } else {
        "big blinds"
    };
    format!("worth {number} {unit}")
}

pub(super) fn draw_pot(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let finished = matches!(v.state, State::HandOver | State::GameOver);
    let amount = if finished { v.last_pot } else { v.pot };
    let flashing = app.pot_flash.is_some_and(|t| {
        t.elapsed() < Duration::from_millis(u64::from(app.config.client.pot_flash_ms))
    });
    let (main, border) = if finished {
        (Color::Green, Color::Green)
    } else if flashing {
        (Color::LightYellow, Color::LightYellow)
    } else {
        (Color::Yellow, Color::Yellow)
    };
    let title = if finished {
        " ★ POT WON ★ "
    } else {
        " ◆ POT ◆ "
    };
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(border).add_modifier(Modifier::BOLD))
        .title(Line::from(Span::styled(
            title,
            Style::new().fg(main).add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Center);
    let inner_w = area.width.saturating_sub(2) as usize;
    let number = chips_text(amount);
    let big = big_number(&number);
    let style = Style::new().fg(main).add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> = if text_width(&big[0]) <= inner_w {
        big.iter()
            .map(|r| Line::from(Span::styled(r.clone(), style)))
            .collect()
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(format!("{number} chips"), style)),
            Line::from(""),
        ]
    };
    let extra = if finished {
        let winners: Vec<String> = v
            .seats
            .iter()
            .filter(|s| s.payout > 0)
            .map(|s| s.name.clone())
            .collect();
        if winners.is_empty() {
            Line::from("")
        } else {
            Line::from(Span::styled(
                fit(&format!("→ {}", winners.join(", ")), inner_w),
                green().add_modifier(Modifier::BOLD),
            ))
        }
    } else if v.pots.len() > 1 {
        let parts: Vec<String> = v
            .pots
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == 0 {
                    format!("main {}", chips_text(*p))
                } else {
                    format!("side {}", chips_text(*p))
                }
            })
            .collect();
        Line::from(Span::styled(
            fit(&parts.join(" · "), inner_w),
            Style::new().fg(main),
        ))
    } else if amount > 0 {
        Line::from(Span::styled(
            fit(&big_blinds_text(amount, v.bb), inner_w),
            Style::new().fg(main),
        ))
    } else {
        Line::from("")
    };
    lines.push(extra);
    let lines = centre_vertically(lines, area);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::super::POT_WIDTH;
    use super::*;

    #[test]
    fn pot_sizes_in_big_blinds() {
        assert_eq!(big_blinds_text(30, 20), "worth 1.5 big blinds");
        assert_eq!(big_blinds_text(20, 20), "worth 1 big blind");
        assert_eq!(big_blinds_text(190, 20), "worth 9.5 big blinds");
        assert_eq!(big_blinds_text(1234, 20), "worth 62 big blinds");
        assert!(text_width(&big_blinds_text(u32::MAX, 1)) <= (POT_WIDTH - 2) as usize);
    }

    #[test]
    fn big_numbers_line_up_and_fit_the_pot_panel() {
        for n in ["0", "7", "1,234", "999,999"] {
            let rows = big_number(n);
            let w = text_width(&rows[0]);
            assert!(
                rows.iter().all(|r| text_width(r) == w),
                "{n} has ragged rows"
            );
        }
        assert!(text_width(&big_number("999,999")[0]) <= (POT_WIDTH - 2) as usize);
    }
}
