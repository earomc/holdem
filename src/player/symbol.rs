//! Player symbols: a shape and a colour shown before every name, so that
//! players are easy to tell apart. People may choose theirs; everyone else
//! (bots, and people who did not choose) gets the one their name gives.
//!
//! This is the plain data; the terminal colours are the UI's business.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Every shape: the first [`BASE_GLYPHS`] are open to everyone, the rest are
/// unlocked by achievements. Each is one column wide and none is an emoji or
/// a card suit.
pub const GLYPHS: [&str; 20] = [
    "●", "▲", "◆", "■", "★", "♪", "✚", "◉", "✿", "☾", "⚑", "▼", // open to everyone
    "♛", "✦", "✠", "↯", "‽", "♞", "↺", "⌘", // unlocked by achievements
];

/// How many of [`GLYPHS`] everyone may use.
pub const BASE_GLYPHS: usize = 12;

/// The colours, by name; they stand out from each other and from the grey of
/// the seat borders.
pub const COLORS: [&str; 8] = [
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "light red",
    "light blue",
];

/// A shape and a colour, as indices into [`GLYPHS`] and [`COLORS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol {
    pub glyph: u8,
    pub color: u8,
}

/// FNV-1a: small and fixed for good (the standard hasher may change between
/// Rust versions, and with it everyone's symbol).
fn hash(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |h, b| {
        (h ^ u32::from(b)).wrapping_mul(0x0100_0193)
    })
}

impl Symbol {
    /// The symbol a name gives: always the same for the same name, whatever
    /// its case and surrounding spaces. Only shapes open to everyone.
    pub fn for_name(name: &str) -> Symbol {
        let h = hash(&name.trim().to_lowercase()) as usize;
        Symbol {
            glyph: (h % BASE_GLYPHS) as u8,
            color: ((h / BASE_GLYPHS) % COLORS.len()) as u8,
        }
    }

    /// Its shape ("★"). Out-of-range indices (from a newer program) wrap.
    pub fn glyph_text(self) -> &'static str {
        GLYPHS[usize::from(self.glyph) % GLYPHS.len()]
    }

    /// Its colour's name ("cyan").
    pub fn color_name(self) -> &'static str {
        COLORS[usize::from(self.color) % COLORS.len()]
    }

    /// One of the shapes open to everyone.
    pub fn is_base(self) -> bool {
        usize::from(self.glyph) < BASE_GLYPHS
    }

    /// Reads "★ cyan": a shape from [`GLYPHS`], a space, a colour from [`COLORS`].
    pub fn parse(text: &str) -> Option<Symbol> {
        let text = text.trim();
        let (glyph, color) = text.split_once(' ')?;
        let glyph = GLYPHS.iter().position(|g| *g == glyph)?;
        let color = COLORS
            .iter()
            .position(|c| c.eq_ignore_ascii_case(color.trim()))?;
        Some(Symbol {
            glyph: glyph as u8,
            color: color as u8,
        })
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.glyph_text(), self.color_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn a_name_always_gets_the_same_symbol() {
        assert_eq!(Symbol::for_name("Mo"), Symbol::for_name(" mo "));
        assert_eq!(Symbol::for_name("Ava"), Symbol::for_name("AVA"));
        assert!(Symbol::for_name("Ava").is_base());
        let names = ["Ava", "Ben", "Chloe", "Dmitri", "Elena", "Frank"];
        let mut symbols: Vec<Symbol> = names.iter().map(|n| Symbol::for_name(n)).collect();
        symbols.dedup();
        assert!(symbols.len() > 1, "every name got the same symbol");
    }

    #[test]
    fn names_keep_the_symbols_they_had() {
        // The mapping of holdem 0.9, where the symbol came from the name alone.
        assert_eq!(Symbol::for_name("Ava").to_string(), "☾ light blue");
        assert_eq!(Symbol::for_name("Ben").to_string(), "⚑ red");
    }

    #[test]
    fn every_shape_is_one_column_wide_and_unique() {
        for g in GLYPHS {
            assert_eq!(g.width(), 1, "{g}");
        }
        let mut all = GLYPHS.to_vec();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), GLYPHS.len());
    }

    #[test]
    fn symbols_read_back_as_written() {
        let s = Symbol { glyph: 4, color: 5 };
        assert_eq!(s.to_string(), "★ cyan");
        assert_eq!(Symbol::parse("★ cyan"), Some(s));
        assert_eq!(
            Symbol::parse("  ★ Light Blue "),
            Some(Symbol { glyph: 4, color: 7 })
        );
        assert_eq!(Symbol::parse("★"), None);
        assert_eq!(Symbol::parse("x cyan"), None);
        assert_eq!(Symbol::parse("★ purple"), None);
    }
}
