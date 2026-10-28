//! The glossary and the bot styles in `docs/poker.md` are generated from the
//! game's own help, so the two cannot disagree. The test fails when they do;
//! `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` rewrites the file.

use std::path::PathBuf;

use holdem::bot::ARCHETYPES;
use holdem::config::Config;

use super::{Item, glossary};

const END: &str = "<!-- end of generated text -->";

/// The marker before a generated block.
fn start(name: &str) -> String {
    format!(
        "<!-- generated {name}: edit src/help, then run \
         `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` -->"
    )
}

fn glossary_table() -> String {
    let mut out = String::from("| Term | Meaning |\n| --- | --- |\n");
    for item in glossary::page(&Config::default()).items {
        if let Item::Pair(term, meaning) = item {
            out.push_str(&format!("| {term} | {meaning} |\n"));
        }
    }
    out
}

fn styles_list() -> String {
    let mut out = String::new();
    for (i, a) in ARCHETYPES.iter().enumerate() {
        let end = if i + 1 == ARCHETYPES.len() { "." } else { ";" };
        out.push_str(&format!(
            "- *{}*: {}{end}\n",
            a.personality.style, a.description
        ));
    }
    out
}

/// `text` with the block called `name` replaced by `content`.
fn replace_block(text: &str, name: &str, content: &str) -> String {
    let start = start(name);
    let (before, rest) = text
        .split_once(&start)
        .unwrap_or_else(|| panic!("docs/poker.md has no marker for {name}"));
    let (_, after) = rest
        .split_once(END)
        .unwrap_or_else(|| panic!("the {name} block in docs/poker.md has no end"));
    format!("{before}{start}\n{content}{END}{after}")
}

#[test]
fn docs_match_the_game() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join("docs").join("poker.md"))
        .find(|p| p.exists())
        .expect("docs/poker.md is in the repository");
    // Compare without carriage returns, which git may add on Windows.
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n");
    let generated = replace_block(&text, "glossary", &glossary_table());
    let generated = replace_block(&generated, "styles", &styles_list());
    if generated == text {
        return;
    }
    if std::env::var_os("HOLDEM_UPDATE_DOCS").is_some() {
        std::fs::write(&path, generated).unwrap();
    } else {
        panic!(
            "docs/poker.md differs from the in-game help; \
             run `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` to update it"
        );
    }
}
