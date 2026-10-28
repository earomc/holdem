//! Help page: every poker term the game uses.

use holdem::config::Config;

use super::{P, Page, number_word};
use crate::keys::{self, Cmd, Place};

pub(super) fn page(d: &Config) -> Page {
    let rules = &d.tables[0].rules;
    let settings = keys::key(Place::Table, Cmd::Settings);
    Page {
        title: "Glossary",
        items: vec![
            P(
                "Aggressive",
                "Playing by betting and raising rather than checking and calling.",
            ),
            P(
                "All-in",
                "Having bet all your remaining chips. You stay in the hand to the end \
                     but can only win what you matched.",
            ),
            P(
                "Ante",
                "A small forced bet every player pays each hand (an optional rule).",
            ),
            P(
                "Bet",
                "Putting chips in when nobody else has in the current betting round.",
            ),
            P(
                "Bet to match",
                "On the table: the biggest bet in this round, which everyone must \
                           call to stay in.",
            ),
            P(
                "Betting round",
                "One pass of decisions around the table. A hand has up to four: \
                            pre-flop, flop, turn and river.",
            ),
            P(
                "Big blind (BB)",
                "The larger forced bet, posted by the second player to the left \
                             of the button. It is also the smallest bet allowed.",
            ),
            P(
                "Blinds",
                "The forced bets posted before the cards are dealt.",
            ),
            P(
                "Blind level",
                "The current size of the blinds. Levels go up during the \
                          game if that option is chosen.",
            ),
            P(
                "Bluff",
                "Betting with a weak hand to make better hands fold.",
            ),
            P("Board", "The community cards in the middle of the table."),
            P(
                "Burn card",
                "A card dealt face down off the deck before the flop, the turn and \
                        the river, and never used.",
            ),
            P(
                "Button (D)",
                "The dealer position. It acts last after the flop and moves one \
                         seat clockwise every hand.",
            ),
            P("Call", "Matching the current bet."),
            P(
                "Check",
                "Passing without betting; allowed only when nobody has bet this \
                    round.",
            ),
            P(
                "Chips",
                "The tokens used for betting. Your chips in front of you are your \
                    stack.",
            ),
            P(
                "Community cards",
                "The five shared cards (flop, turn, river) every player can \
                              use.",
            ),
            P(
                "Dead button",
                "A rule that moves the big blind exactly one player per hand, \
                          so the button may sit on an empty seat.",
            ),
            P(
                "Dealer",
                "See button. Here nobody actually deals: the button just marks the \
                     position.",
            ),
            P("Eliminated", "Out of the game after losing all chips."),
            P(
                "Flop",
                "The first three community cards, dealt together; also the second \
                   betting round.",
            ),
            P("Flush", "Five cards of the same suit."),
            P("Fold", "Giving up the hand and the chips already bet."),
            P("Four of a kind (quads)", "Four cards of the same rank."),
            P(
                "Full house",
                "Three of a kind plus a pair. \"Kings full of Fours\" is K K K 4 4.",
            ),
            P(
                "Hand limit",
                "A maximum number of hands, after which the biggest stack wins.",
            ),
            P(
                "Heads-up",
                "Only two players in the hand or left in the game.",
            ),
            P(
                "High card",
                "A hand with no pair or better; named after its highest card.",
            ),
            P("Hole cards", "Your two private cards."),
            P("Host", "The person whose computer runs a network table."),
            P(
                "Kicker",
                "A card that is not part of the pair, trips, etc. but still breaks \
                     ties.",
            ),
            P(
                "Limp",
                "Just calling the big blind before the flop instead of raising.",
            ),
            P(
                "Lobby",
                "The waiting room of a network table before a game starts.",
            ),
            P("Loose", "Playing many hands. The opposite of tight."),
            P(
                "Main pot",
                "The pot every player still in can win. See side pot.",
            ),
            P(
                "Minimum raise",
                "The smallest legal raise: the size of the last bet or raise \
                            in this round, at least one big blind.",
            ),
            P(
                "Muck",
                "To throw your cards away face down, unseen (when folding, or at a \
                   showdown you cannot win if mucking is allowed).",
            ),
            P(
                "Nuts",
                "The best possible hand with the cards on the board.",
            ),
            P(
                "Odd chip",
                "A chip left over when a pot is split; it goes to the first winner \
                       to the left of the button.",
            ),
            P(
                "Offsuit",
                "Two hole cards of different suits, e.g. A♠ K♦ (\"Ace-King \
                      offsuit\").",
            ),
            P(
                "Passive",
                "Checking and calling rather than betting and raising.",
            ),
            P(
                "Pocket pair",
                "Two hole cards of the same rank, e.g. \"Pocket Kings\".",
            ),
            P(
                "Position",
                "Where you sit relative to the button. Late position (near the \
                       button) acts later and is an advantage.",
            ),
            P("Post", "Putting a blind, ante or straddle into the pot."),
            P("Post-flop", "The flop, turn and river betting rounds."),
            P("Pot", "All the chips bet so far in the current hand."),
            P(
                "Pot odds",
                "The share of the final pot that a call costs: call / (pot + call). \
                       A call pays off in the long run if your hand wins more often than \
                       that.",
            ),
            P(
                "Pre-flop",
                "The first betting round, after the hole cards and before the \
                       flop.",
            ),
            P(
                "Push-or-fold",
                "With a short stack, the only sensible plays are going all-in \
                           or folding.",
            ),
            P(
                "Raise",
                "Making the current bet bigger. \"Raise to 120\" means your total bet \
                    in this round becomes 120.",
            ),
            P(
                "Rebuy",
                "Buying back in for the starting stack after losing all your chips \
                    (an optional rule).",
            ),
            P(
                "Reopen",
                "A full raise gives players who already acted the right to raise \
                     again; a short all-in does not.",
            ),
            P(
                "River",
                "The fifth and last community card; also the last betting round.",
            ),
            P(
                "Royal flush",
                "A K Q J 10 of one suit: the best hand there is.",
            ),
            P(
                "Short stack",
                "A stack that is small compared to the blinds.",
            ),
            P(
                "Showdown",
                "Showing the hands after the last betting round to decide who \
                       wins.",
            ),
            P(
                "Side pot",
                "An extra pot for chips that an all-in player could not match. \
                       Only players who put chips into it can win it.",
            ),
            P(
                "Slow play",
                "Checking or just calling with a strong hand to hide its \
                        strength.",
            ),
            P(
                "Small blind (SB)",
                "The smaller forced bet (half the big blind), posted by the \
                               player to the left of the button.",
            ),
            P("Split pot", "A pot shared by players with equal hands."),
            P("Stack", "The chips a player has in front of them."),
            P(
                "Straddle (STR)",
                format!(
                    "An optional third blind ({} big blinds by default), posted by the player \
                 left of the big blind, who then acts last pre-flop.",
                    number_word(rules.straddle_multiplier as usize)
                ),
            ),
            P(
                "Straight",
                "Five cards of consecutive ranks in mixed suits.",
            ),
            P(
                "Straight flush",
                "Five cards of consecutive ranks, all of one suit.",
            ),
            P(
                "Street",
                "A stage of the hand: pre-flop, flop, turn or river.",
            ),
            P(
                "Suited",
                "Two hole cards of the same suit, e.g. J♥ 10♥ (\"Jack-Ten \
                     suited\").",
            ),
            P(
                "Tight",
                "Playing few hands, only good ones. The opposite of loose.",
            ),
            P("Trips / set", "Three of a kind."),
            P(
                "Turn",
                "The fourth community card; also the third betting round.",
            ),
            P("Two pair", "Two different pairs."),
            P(
                "Uncalled bet",
                "Chips nobody matched; they are returned to the bettor.",
            ),
            P(
                "Under the gun",
                "The first player to act before the flop, left of the big \
                            blind (or of the straddle).",
            ),
            P("Wheel", "5 4 3 2 A, the lowest straight."),
            P(
                "Win chance",
                format!(
                    "The optional hint shown while you are in a hand: how often your hand \
                     would win against the same number of opponents holding random cards. \
                     Turn it on in the settings (key {settings}), where the table allows it."
                ),
            ),
            P(
                "Worth N big blinds",
                "Shown under the pot: its size divided by the big blind, i.e. how big \
             the pot is compared with what it costs to play a hand. Players think in big \
             blinds because chip amounts mean little once the blinds rise.",
            ),
        ],
    }
}
