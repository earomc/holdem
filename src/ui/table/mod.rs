//! The table: seats, board, pot, your hand and the footer with the actions.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Wrap};

use holdem::protocol::TableView;

use super::log::log_view;
use super::popups;
use super::style::{centered, framed, key_for};
use super::text::{fit, text_width};
use crate::app::App;
use crate::keys::{Cmd, Place};
use crate::session::Kind;
use footer::footer_lines;
use middle::{draw_board, draw_pot};
use seats::draw_seat;
use you::draw_you;

mod footer;
mod middle;
mod seats;
mod you;

fn draw_header(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let style = Style::new().fg(Color::Black).bg(Color::Green);
    let alive = v.seats.iter().filter(|s| !s.out).count();
    let seated = v.seats.iter().filter(|s| !s.vacant).count();
    let hand = if v.rules.hand_limit > 0 {
        format!("Hand {} of {}", v.hand_no, v.rules.hand_limit)
    } else {
        format!("Hand #{}", v.hand_no)
    };
    let left = format!(" ♠ {hand}  ·  {alive} of {seated} players in");
    let mode = match app.session.as_ref().map(|s| s.kind) {
        Some(Kind::Host) => "HOSTING  ·  ",
        Some(Kind::Guest) => "ONLINE  ·  ",
        _ => "",
    };
    let sound = match app.sound.unavailable() {
        Some(_) => "no sound".to_string(),
        None if app.sound.volume() == 0 => "sound off".to_string(),
        None => format!("sound {}%", app.sound.volume()),
    };
    let paused = if v.paused { "PAUSED  ·  " } else { "" };
    let right = format!(
        "{}{}speed {}  ·  {}  ·  {} settings ",
        mode,
        paused,
        app.speed().name.to_lowercase(),
        sound,
        key_for(Place::Table, Cmd::Settings)
    );
    let right_w = (text_width(&right) as u16).min(area.width);
    let [l, r] = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_w)]).areas(area);
    frame.render_widget(Paragraph::new(fit(&left, l.width as usize)).style(style), l);
    frame.render_widget(
        Paragraph::new(right)
            .style(style)
            .alignment(Alignment::Right),
        r,
    );
}

/// Width of the pot panel next to the board.
const POT_WIDTH: u16 = 30;

// The table screen, top to bottom: header, rows of opponents, board and pot,
// your seat, and the panel with your options.
const HEADER_H: u16 = 1;

const SEAT_H: u16 = 5;

const MIDDLE_MIN_H: u16 = 6;

const YOU_H: u16 = 6;

const FOOTER_H: u16 = 5;

/// Narrowest terminal the table is drawn in.
const MIN_WIDTH: u16 = 80;

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let Some(v) = app.view() else {
        frame.render_widget(
            Paragraph::new("Shuffling…").alignment(Alignment::Center),
            centered(area, 30, 1),
        );
        return;
    };
    let others: Vec<usize> = (0..v.seats.len()).filter(|&i| Some(i) != v.you).collect();
    let count = others.len().max(1);
    let cols = if count <= 4 { count } else { count.div_ceil(2) };
    let rows = count.div_ceil(cols);
    let need_h = HEADER_H + rows as u16 * SEAT_H + MIDDLE_MIN_H + YOU_H + FOOTER_H;
    if area.width < MIN_WIDTH || area.height < need_h {
        let msg = format!(
            "Terminal too small: this table needs at least {}x{} (now {}x{}). Enlarge the window, or press {} to leave.",
            MIN_WIDTH,
            need_h,
            area.width,
            area.height,
            key_for(Place::Table, Cmd::Leave)
        );
        frame.render_widget(
            Paragraph::new(msg)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center),
            centered(area, area.width.min(60), 6),
        );
        return;
    }

    let [header, main, footer] = Layout::vertical([
        Constraint::Length(HEADER_H),
        Constraint::Min(0),
        Constraint::Length(FOOTER_H),
    ])
    .areas(area);
    draw_header(frame, app, v, header);

    let (table_area, log_area) = if area.width >= 110 {
        let [t, l] = Layout::horizontal([Constraint::Min(0), Constraint::Length(38)]).areas(main);
        (t, Some(l))
    } else {
        (main, None)
    };

    let [opp_area, middle, you_area] = Layout::vertical([
        Constraint::Length(rows as u16 * SEAT_H),
        Constraint::Min(MIDDLE_MIN_H),
        Constraint::Length(YOU_H),
    ])
    .areas(table_area);

    let row_areas = Layout::vertical(vec![Constraint::Length(SEAT_H); rows]).split(opp_area);
    let mut k = 0;
    for r in 0..rows {
        let in_row = cols.min(count - r * cols);
        let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, in_row as u32); in_row])
            .split(row_areas[r]);
        for c in 0..in_row {
            if let Some(&seat) = others.get(k) {
                draw_seat(frame, v, seat, col_areas[c]);
            }
            k += 1;
        }
    }

    let [board_area, pot_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(POT_WIDTH)]).areas(middle);
    draw_board(frame, v, board_area);
    draw_pot(frame, app, v, pot_area);
    draw_you(frame, app, v, you_area);

    if let Some(log_area) = log_area
        && let Some(session) = app.session.as_ref()
    {
        let inner_h = log_area.height.saturating_sub(2) as usize;
        let inner_w = log_area.width.saturating_sub(2) as usize;
        let (lines, _) = log_view(&session.log, inner_h, inner_w, 0);
        let block = framed(Line::from(" Log (L: full log) "), Color::DarkGray);
        frame.render_widget(Paragraph::new(lines).block(block), log_area);
    }

    let block = framed(Line::from(" Your move "), Color::Gray);
    let inner_w = footer.width.saturating_sub(2) as usize;
    frame.render_widget(
        Paragraph::new(footer_lines(app, v, inner_w)).block(block),
        footer,
    );

    if let Some(prompt) = &app.raise {
        popups::raise(frame, v, prompt, table_area);
    }
}
