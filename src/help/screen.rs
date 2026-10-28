//! Help page: the table screen, the style estimate and the bot styles.

use holdem::bot::ARCHETYPES;

use super::{Facts, H, P, Page, T, number_word};
use crate::keys::{self, Cmd, Place};

pub(super) fn page(facts: &Facts) -> Page {
    let settings = keys::key(Place::Table, Cmd::Settings);
    let profile = keys::key(Place::Table, Cmd::Profile);
    let log = keys::key(Place::Table, Cmd::Log);
    let mut styles = vec![
        H("YOUR OPPONENTS"),
        T(format!(
            "Every bot plays one of {} styles; you see which one in the final \
             standings. All of them only use what a real player could see.",
            number_word(ARCHETYPES.len())
        )),
    ];
    styles.extend(
        ARCHETYPES
            .iter()
            .map(|a| P(a.personality.style, a.description)),
    );
    Page {
        title: "The screen, your style, your opponents",
        items: vec![
            H("ON THE TABLE"),
            P(
                "[D] [SB] [BB] [STR]",
                "the button, the small blind, the big blind and the \
                                  straddle this hand",
            ),
            P("Stack", "the chips a player has not put in the pot yet"),
            P(
                "In this round",
                "the chips you have put in during the current betting round",
            ),
            P(
                "Pot",
                "the gold box: the chips in the middle, and what they are \"worth\" in big \
             blinds (how big the pot is compared with the cost of playing a hand). It \
             flashes when it grows and lists main and side pots once there are several. \
             After a hand it turns green and shows who won it.",
            ),
            P("Bet to match", "the biggest bet in this round"),
            P(
                "Level",
                "the blind level, and how many hands are left before it goes up",
            ),
            P(
                "▒▒▒",
                "a face-down card; an empty slot on the board is shown as a dot",
            ),
            P("thinking…", "the player whose turn it is"),
            P("(away)", "a person whose connection dropped"),
            P(
                "Wins +N",
                "chips the player won in the hand that just ended",
            ),
            P(
                "Borders",
                "yellow: whose turn it is; green: won the last hand; grey: folded \
                      or out",
            ),
            P(
                "To call",
                "the chips you need to put in to stay in the hand",
            ),
            P(
                "Pot odds",
                "how often a call has to win to break even (see the glossary)",
            ),
            P(
                "Win chance",
                format!(
                    "the optional hint (settings, key {settings}; a table may not allow it), shown \
             while you are in a hand: how often your hand would win against the same \
             number of opponents holding random cards, estimated by dealing out \
             thousands of random boards. Real opponents who bet usually hold better than \
             random cards, so treat it as a rough guide. Calling tends to pay off when \
             your win chance is above the pot odds."
                ),
            ),
            H(format!("YOUR STYLE (KEY {profile})")),
            T(format!(
                "The game counts how often you play a hand (put chips in voluntarily before \
           the flop), how often you raise first, how often you bet or raise rather than \
           call, and how often your bets after the flop are made without even a pair. \
           Only hands you start with at least {} big blinds count: with fewer, everybody \
           must play push-or-fold. It compares your numbers with what each bot style \
           really does (measured over thousands of bot hands) and names the closest: a \
           rough guess after {} such hands, a reliable estimate after {}. Neighbouring \
           styles are similar, so look at the top two. Your counts are also added up over \
           all your games (in stats.toml, next to holdem.toml), and the Your style tab of \
           game info (key {log}) shows that estimate too.",
                facts.style_min_stack_bb,
                holdem::stats::ROUGH_HANDS.min(facts.style_min_hands),
                facts.style_min_hands
            )),
        ]
        .into_iter()
        .chain(styles)
        .collect(),
    }
}
