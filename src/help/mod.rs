//! The text of the in-game help: every key, the rules, the optional table
//! rules, playing with others, the hand rankings, and a glossary of every poker
//! term the game uses. Rendering lives in `popups.rs`.
//!
//! The pages are built when shown, so nothing in them can disagree with the
//! code: the Keys page comes from the key table (`keys.rs`), the bot styles
//! from `bot::ARCHETYPES`, and every default value from `Config::default()`.

use holdem::config::Config;

use crate::keys;

mod betting;
#[cfg(test)]
mod docs;
mod glossary;
mod hand;
mod multiplayer;
mod options;
mod rankings;
mod screen;

/// One element of a help page.
pub enum Item {
    Heading(String),
    /// A paragraph, word-wrapped to the window.
    Text(String),
    /// A label in the left column with its explanation on the right.
    Pair(String, String),
}

pub struct Page {
    pub title: &'static str,
    pub items: Vec<Item>,
}

#[allow(non_snake_case)]
fn H(text: impl Into<String>) -> Item {
    Item::Heading(text.into())
}

#[allow(non_snake_case)]
fn T(text: impl Into<String>) -> Item {
    Item::Text(text.into())
}

#[allow(non_snake_case)]
fn P(label: impl Into<String>, text: impl Into<String>) -> Item {
    Item::Pair(label.into(), text.into())
}

/// Values in effect where the help is read (they depend on the settings file
/// and on the table).
pub struct Facts {
    /// Counted hands before your style is estimated.
    pub style_min_hands: u32,
    /// Hands started with fewer big blinds do not count for the style.
    pub style_min_stack_bb: u32,
}

/// How many pages there are.
pub fn page_count() -> usize {
    pages(&Facts {
        style_min_hands: 1,
        style_min_stack_bb: 1,
    })
    .len()
}

/// The Keys page, from the key table.
fn keys_page() -> Page {
    let mut items = Vec::new();
    for section in keys::SECTIONS {
        items.push(H(section.title));
        for row in section.rows {
            items.push(P(keys::label(row), row.what));
        }
    }
    items.push(T(
        "In a game against bots, the game is paused while any window is open.",
    ));
    Page {
        title: "Keys",
        items,
    }
}

/// Every page, in the order the help shows them.
pub fn pages(facts: &Facts) -> Vec<Page> {
    let d = Config::default();
    vec![
        keys_page(),
        hand::page(),
        betting::page(&d),
        options::page(&d),
        multiplayer::page(&d),
        rankings::page(),
        glossary::page(&d),
        screen::page(facts),
    ]
}

/// Small numbers as words, as the text reads better that way.
fn number_word(n: usize) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), std::string::ToString::to_string)
}
