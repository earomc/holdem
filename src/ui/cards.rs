//! Cards as coloured text.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use holdem::card::Card;

use super::style::dim;

fn card_span(c: Card) -> Span<'static> {
    let fg = if c.suit.is_red() {
        Color::Red
    } else {
        Color::Black
    };
    Span::styled(
        format!("{:>3}{} ", c.rank_symbol(), c.suit.symbol()),
        Style::new()
            .fg(fg)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD),
    )
}

fn back_span() -> Span<'static> {
    Span::styled(" ▒▒▒ ", Style::new().fg(Color::LightBlue).bg(Color::Blue))
}

pub(super) fn cards_line(cards: &[Card]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, c) in cards.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(card_span(*c));
    }
    Line::from(spans)
}

pub(super) fn hidden_line() -> Line<'static> {
    Line::from(vec![back_span(), Span::raw(" "), back_span()])
}

pub(super) fn board_line(board: &[Card]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for i in 0..5 {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        match board.get(i) {
            Some(c) => spans.push(card_span(*c)),
            None => spans.push(Span::styled("  ·  ", dim())),
        }
    }
    Line::from(spans)
}
