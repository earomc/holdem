//! Help page: the poker hands, best first.

use super::{H, P, Page, T};

pub(super) fn page() -> Page {
    Page {
        title: "Hand rankings",
        items: vec![
            H("FROM BEST TO WORST"),
            P(
                "Royal flush",
                "A K Q J 10, all of one suit. The best possible hand.",
            ),
            P(
                "Straight flush",
                "Five cards in a row, all of one suit, e.g. 9♥ 8♥ 7♥ 6♥ 5♥.",
            ),
            P(
                "Four of a kind",
                "Four cards of the same rank, e.g. 9 9 9 9 (also called quads).",
            ),
            P(
                "Full house",
                "Three of one rank plus a pair of another, e.g. K K K 4 4, read \"Kings full \
             of Fours\".",
            ),
            P(
                "Flush",
                "Any five cards of the same suit, not in a row. Compared card by card \
                    from the highest.",
            ),
            P(
                "Straight",
                "Five ranks in a row in mixed suits, e.g. 10 9 8 7 6. The ace counts high (A K \
             Q J 10) or low (5 4 3 2 A, the \"wheel\", the lowest straight). Straights do \
             not wrap around: Q K A 2 3 is not a straight.",
            ),
            P(
                "Three of a kind",
                "Three cards of the same rank (also called trips or a set).",
            ),
            P("Two pair", "Two different pairs, e.g. A A 7 7."),
            P("Pair", "Two cards of the same rank."),
            P("High card", "None of the above: the highest card counts."),
            H("BREAKING TIES"),
            T(
                "Two hands of the same kind are compared by the cards that make them first: the \
           higher pair, the higher three of a kind in a full house and then its pair, \
           the higher top pair in two pair, the highest card of a straight. If those are \
           equal, the remaining cards (kickers) are compared one at a time, highest \
           first.",
            ),
            T(
                "Only five cards ever count: a sixth or seventh card never breaks a tie. When \
           the best five cards are the same for several players (for example when the \
           five community cards are everyone's best hand, \"the board plays\") the pot \
           is split.",
            ),
            H("CARDS"),
            T(
                "Ranks from low to high: 2 3 4 5 6 7 8 9 10 J (Jack) Q (Queen) K (King) A \
           (Ace). The four suits are ♣ clubs, ♦ diamonds, ♥ hearts and ♠ spades; all \
           are equal. Two hole cards are described as, for example, \"Ace-King suited\" \
           (same suit), \"Ace-King offsuit\" (different suits) or \"Pocket Kings\" (a \
           pair).",
            ),
        ],
    }
}
