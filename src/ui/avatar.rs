//! Drawing a player's symbol (see [`holdem::player::symbol`]).

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use holdem::player::symbol::Symbol;

/// The terminal colors, in the order of [`holdem::player::symbol::COLORS`].
const COLORS: [Color; 8] = [
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::LightRed,
    Color::LightBlue,
];

/// The symbol of a player: the one they chose, or else the one their name gives.
pub(super) fn avatar(name: &str, chosen: Option<Symbol>) -> Span<'static> {
    let s = chosen.unwrap_or_else(|| Symbol::for_name(name));
    let color = COLORS[usize::from(s.color) % COLORS.len()];
    Span::styled(s.glyph_text(), Style::new().fg(color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use holdem::player::symbol::COLORS as NAMES;

    #[test]
    fn a_chosen_symbol_wins_over_the_name() {
        let chosen = Symbol {
            glyph: 12,
            color: 2,
        };
        assert_eq!(avatar("Mo", Some(chosen)).content, "♛");
        assert_eq!(avatar("Mo", None), avatar("mo", None));
        assert_eq!(COLORS.len(), NAMES.len());
    }
}
