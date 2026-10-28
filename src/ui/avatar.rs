//! A colored symbol for every player name, so players with similar names are
//! easy to tell apart. It follows from the name alone: the same name gets the
//! same symbol on every screen and every computer.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// One column wide everywhere, and none of them a card suit or an emoji.
const GLYPHS: [&str; 12] = ["●", "▲", "◆", "■", "★", "♪", "✚", "◉", "✿", "☾", "⚑", "▼"];

/// Colors that stand out from each other and from the gray of the seat borders.
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

/// FNV-1a: small and fixed for good (the standard hasher may change between
/// Rust versions, and with it everyone's symbol).
fn hash(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

/// The symbol of `name`. Case and surrounding spaces do not count.
pub(super) fn avatar(name: &str) -> Span<'static> {
    let h = hash(&name.trim().to_lowercase());
    let glyph = GLYPHS[h as usize % GLYPHS.len()];
    let color = COLORS[(h as usize / GLYPHS.len()) % COLORS.len()];
    Span::styled(glyph, Style::new().fg(color))
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn a_name_always_gets_the_same_symbol() {
        assert_eq!(avatar("Mo"), avatar(" mo "));
        assert_eq!(avatar("Ava"), avatar("AVA"));
        let names = [
            "Ava", "Ben", "Chloe", "Dmitri", "Elena", "Frank", "Gina", "Hiro",
        ];
        let mut symbols: Vec<_> = names.iter().map(|n| avatar(n)).collect();
        symbols.dedup();
        assert!(symbols.len() > 1, "every name got the same symbol");
    }

    #[test]
    fn every_symbol_is_one_column_wide() {
        for g in GLYPHS {
            assert_eq!(g.width(), 1, "{g}");
        }
    }
}
