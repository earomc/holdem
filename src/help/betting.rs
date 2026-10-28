//! Help page: no-limit betting: raises, all-ins, side pots.

use holdem::config::Config;

use super::{H, P, Page, T};

pub(super) fn page(d: &Config) -> Page {
    let steps = d.game.rules.level_steps();
    Page {
        title: "Betting rules",
        items: vec![
            H("YOUR OPTIONS"),
            P(
                "Check",
                "pass without betting. Only possible when nobody has bet in this round.",
            ),
            P("Bet", "put chips in when nobody has bet yet in this round."),
            P(
                "Call",
                "put in just enough to match the biggest bet in this round.",
            ),
            P(
                "Raise",
                "make the bet bigger. Everybody else must then call the new amount, \
                    raise again, or fold.",
            ),
            P(
                "Fold",
                "give up the hand and every chip you already put in the pot.",
            ),
            P("All-in", "bet everything you have left."),
            T(
                "A betting round ends when every player still in has put in the same amount \
           (or is all-in) and everyone has had a chance to act.",
            ),
            H("NO-LIMIT"),
            T(
                "You may bet or raise any amount up to your whole stack. The smallest opening \
           bet is one big blind. A raise must be at least as big as the last bet or \
           raise in the same round: if the big blind is 20 and someone raises to 60 (an \
           increase of 40), the next raise must go to at least 100.",
            ),
            T(
                "In this game you always type the total you want to have bet in the round \
           (\"raise to\"), not the amount added on top.",
            ),
            T(
                "Players who still have to act must call the full big blind even when the \
           big blind is all-in for less.",
            ),
            H("ALL-IN FOR LESS"),
            T(
                "If you cannot cover a bet you may call with all your chips. You can then only \
           win the part of the pot you matched: the rest becomes a side pot that only \
           the players who put in more can win. There can be several side pots.",
            ),
            T(
                "An all-in raise that is smaller than a full raise does not reopen the betting: \
           players who already acted in this round may then only call or fold. If \
           several such short all-ins add up to a full raise, the betting is reopened.",
            ),
            H("UNCALLED BETS"),
            T(
                "If a bet or part of a bet is not matched by anyone, those chips go back to the \
           player who bet them before the pot is awarded.",
            ),
            H("SHOWDOWN AND SPLIT POTS"),
            T(
                "The best five-card hand wins each pot. Equal hands split it evenly. A chip that \
           cannot be split (the odd chip) goes to the first tied winner clockwise from \
           the left of the button. Suits never decide a winner.",
            ),
            H("THE GAME"),
            T(format!(
                "Everyone starts with the same number of chips. If the blinds go up (a setup \
           option), they rise after a fixed number of hands, by {}% to {}% per level, so \
           the game cannot go on forever. A player with no chips left is eliminated; \
           players knocked out in the same hand are ranked by the chips they started \
           that hand with. The last player with chips wins, or the biggest stack when \
           a hand limit is reached.",
                steps.0, steps.1
            )),
        ],
    }
}
