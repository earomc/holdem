//! Help page: the optional table rules of the setup screen.

use holdem::config::Config;

use super::{P, Page, T, number_word};
use crate::keys::{self, Cmd, Place};

pub(super) fn page(d: &Config) -> Page {
    let rules = &d.game.rules;
    let settings = keys::key(Place::Table, Cmd::Settings);
    Page {
        title: "Table options",
        items: vec![
            T(
                "These are chosen on the game setup screen. Their starting values, the \
           values you can pick from, and details such as the blind schedule, the size \
           of the ante and the straddle, and the rebuy stack are set in holdem.toml \
           (holdem config write writes one with every setting explained). The \
           optional rules are off unless you switch them on.",
            ),
            P(
                "Hand limit",
                "The game ends after this many hands. Players still in are \
                         ranked by their chips, the biggest stack wins.",
            ),
            P(
                "Antes",
                format!(
                    "Every player also pays a small forced bet each hand, by default 1/{} of \
                 the big blind. Antes go into the pot but do not count as a bet: you \
                 still owe the full big blind to play. More money in the pot rewards \
                 playing more hands.",
                    rules.ante_divisor
                ),
            ),
            P(
                "Straddle",
                format!(
                    "With enough players ({} by default), the player to the left of the \
                 big blind posts a third blind ({} big blinds by default). Play before the \
                 flop starts after the straddler, who acts last and may raise. The \
                 smallest raise is then to twice the straddle.",
                    number_word(rules.straddle_min_players),
                    number_word(rules.straddle_multiplier as usize)
                ),
            ),
            P(
                "Rebuys",
                "A player who loses all their chips while rebuys are open (the first \
                     N blind levels) may buy back in (for the starting stack by default). You are \
                     asked after the hand: Y rebuys, Enter gives up. Bots decide by \
                     themselves; loose bots rebuy more often.",
            ),
            P(
                "Button rule",
                "With a moving button, the button simply moves to the next \
                          player. With a dead button, the big blind moves exactly one \
                          player every hand and the small blind and button follow it, \
                          even onto an empty seat. That way nobody skips paying a big \
                          blind when players leave. A dead small blind is not posted.",
            ),
            P(
                "Showdown",
                "With \"losers may muck\", hands are shown in order (the last \
                       player who bet on the river first, otherwise the first player \
                       left of the button) and anyone already beaten throws their cards \
                       away unseen. All-in hands are always shown.",
            ),
            P(
                "Players come and go",
                "Bots occasionally leave between hands, taking their \
                                  chips, and new bots sit down in empty seats with the \
                                  starting stack. The chances are set in holdem.toml.",
            ),
            P(
                "Win-chance hint",
                format!(
                    "Whether players at this table may turn on the win-chance hint (in the \
                 settings, key {settings}). When the table does not allow it, it cannot be \
                 switched on."
                ),
            ),
        ],
    }
}
