//! Help page: the rules of a hand, from the blinds to the showdown.

use super::{H, P, Page, T};

pub(super) fn page() -> Page {
    Page {
        title: "How a hand is played",
        items: vec![
            T("Texas Hold'em is played with one 52-card deck. Every hand runs like this:"),
            P(
                "1. Blinds",
                "The two players to the left of the dealer button post the small blind and the \
             big blind: forced bets that give everyone something to play for. Some tables \
             add antes or a straddle (see Table options).",
            ),
            P(
                "2. Hole cards",
                "Every player gets two private cards, dealt one at a time starting to the left \
             of the button.",
            ),
            P(
                "3. Pre-flop",
                "The first betting round. It starts with the player to the left of the big \
             blind. The big blind acts last and may still raise if everyone only called.",
            ),
            P(
                "4. Flop",
                "One card is burned (put aside unseen) and three community cards are turned \
             face up. Second betting round.",
            ),
            P(
                "5. Turn",
                "Burn, then a fourth community card. Third betting round.",
            ),
            P(
                "6. River",
                "Burn, then the fifth and last community card. Final betting round.",
            ),
            P(
                "7. Showdown",
                "Everyone still in shows their cards. Each player makes the best five-card \
             hand out of their two hole cards and the five community cards, in any \
             combination (using one, both or neither hole card). The best hand wins.",
            ),
            T(
                "The hand ends at once when everyone but one player has folded: that player \
           wins the pot without showing their cards.",
            ),
            H("ORDER OF PLAY"),
            T(
                "Play goes clockwise. From the flop on, the first player still in to the left \
           of the button acts first and the button acts last. Acting last is an \
           advantage, because you have seen what everyone else did.",
            ),
            T(
                "The button moves one seat clockwise after every hand, so everybody takes a \
           turn at every position and pays the blinds equally often.",
            ),
            T(
                "Heads-up (only two players left) the button posts the small blind and acts \
           first before the flop, but last on the flop, turn and river.",
            ),
            H("WHEN EVERYONE IS ALL-IN"),
            T(
                "When no more betting is possible because all players but at most one have \
           bet all their chips, every hand still in is turned face up and the remaining \
           community cards are dealt one street at a time.",
            ),
        ],
    }
}
