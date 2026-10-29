//! The other players' seats around the table.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use holdem::game::{State, chips_text, ordinal};
use holdem::protocol::{SeatView, TableView};

use crate::ui::avatar::avatar;
use crate::ui::boxed;
use crate::ui::cards::{cards_line, hidden_line};
use crate::ui::style::{bold, dim, framed, green};

pub(super) fn tags(v: &TableView, seat: usize) -> String {
    let mut tags: Vec<&str> = Vec::new();
    if seat == v.button {
        tags.push("D");
    }
    if seat == v.sb_seat && !v.sb_dead {
        tags.push("SB");
    }
    if seat == v.bb_seat {
        tags.push("BB");
    }
    if v.straddle_seat == Some(seat) {
        tags.push("STR");
    }
    tags.join(" ")
}

pub(super) fn seat_title(v: &TableView, seat: usize) -> Line<'static> {
    let s = &v.seats[seat];
    if s.vacant {
        return Line::from(Span::styled(" empty ", dim()));
    }
    let mut spans = vec![
        Span::raw(" "),
        avatar(&s.name, s.symbol),
        Span::raw(" "),
        Span::styled(s.name.clone(), bold()),
    ];
    if !s.connected {
        spans.push(Span::styled(" (away)", Style::new().fg(Color::LightRed)));
    }
    let t = tags(v, seat);
    if !t.is_empty() && !s.out {
        spans.push(Span::styled(
            format!(" [{t}]"),
            Style::new().fg(Color::Yellow),
        ));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

pub(super) fn seat_border(v: &TableView, seat: usize) -> Color {
    let s = &v.seats[seat];
    if s.out || s.folded || !s.in_hand {
        Color::DarkGray
    } else if v.state == State::HandOver && s.payout > 0 {
        Color::Green
    } else if v.to_act == Some(seat) {
        Color::Yellow
    } else {
        Color::Gray
    }
}

pub(super) fn status_span(v: &TableView, seat: usize) -> Span<'static> {
    let s = &v.seats[seat];
    if v.state == State::HandOver && s.payout > 0 {
        Span::styled(
            format!("Wins +{}", chips_text(s.payout)),
            green().add_modifier(Modifier::BOLD),
        )
    } else if s.folded {
        Span::styled("Folded", dim())
    } else if v.to_act == Some(seat) {
        Span::styled("thinking…", Style::new().fg(Color::Yellow))
    } else if !s.last_action.is_empty() {
        Span::raw(s.last_action.clone())
    } else if s.all_in {
        Span::styled("All-in", Style::new().fg(Color::Red))
    } else {
        Span::raw("")
    }
}

pub(super) fn draw_seat(frame: &mut Frame, v: &TableView, seat: usize, area: Rect) {
    let s: &SeatView = &v.seats[seat];
    let block = framed(seat_title(v, seat), seat_border(v, seat));
    let mut lines: Vec<Line<'static>> = Vec::new();
    if s.vacant {
        lines.push(Line::from(Span::styled("Empty seat", dim())));
    } else if s.rebuy_pending {
        lines.push(Line::from(Span::styled(
            "Busted",
            Style::new().fg(Color::Red),
        )));
        lines.push(Line::from(Span::styled("may rebuy…", dim())));
    } else if s.out {
        lines.push(Line::from(Span::styled("Eliminated", dim())));
        if let Some(p) = s.place {
            lines.push(Line::from(Span::styled(
                format!("Finished {}", ordinal(p)),
                dim(),
            )));
        }
    } else {
        lines.push(Line::from(format!("Stack {}", chips_text(s.chips))));
        lines.push(Line::from(status_span(v, seat)));
        if !s.cards.is_empty() {
            lines.push(cards_line(&s.cards));
        } else if s.hidden_cards {
            lines.push(hidden_line());
        }
    }
    frame.render_widget(boxed(lines, block, area), area);
}
