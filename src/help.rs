//! The text of the in-game help: every key, the rules, the optional table
//! rules, playing with others, the hand rankings, and a glossary of every poker
//! term the game uses. Rendering lives in `popups.rs`.
//!
//! The pages are built when shown, so nothing in them can disagree with the
//! code: the Keys page comes from the key table (`keys.rs`), the bot styles
//! from `bot::ARCHETYPES`, and every default value from `Config::default()`.

use holdem::bot::ARCHETYPES;
use holdem::config::Config;

use crate::keys::{self, Cmd, Place};

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

pub fn pages(facts: &Facts) -> Vec<Page> {
    let d = Config::default();
    let rules = &d.game.rules;
    let steps = rules.level_steps();
    let settings = keys::key(Place::Table, Cmd::Settings);
    let profile = keys::key(Place::Table, Cmd::Profile);
    let ready = keys::key(Place::HandOver, Cmd::Ready);
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
    vec![
        keys_page(),
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
        },
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
        },
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
                     settings, key {}). When the table does not allow it, it cannot be \
                     switched on.",
                        settings
                    ),
                ),
            ],
        },
        Page {
            title: "Playing with others",
            items: vec![
                H("HOSTING"),
                T(format!(
                    "Choose \"Host a network game\", pick the rules and press Enter. Your table \
                     is open and the lobby shows the address others type to join, such as \
                     192.168.1.20:{}. On the same network (home Wi-Fi, office LAN) others also \
               find it automatically on their join screen. The game starts when everyone in \
               the lobby is ready; bots take the free seats unless you switched that off.",
                    d.server.port
                )),
                T(format!(
                    "Over the internet, others need your public address, and port {} (or the \
                     port set in holdem.toml) must be forwarded to your computer in your router. \
               Your firewall may ask whether holdem may accept connections.",
                    d.server.port
                )),
                H("A SERVER ANYONE CAN JOIN"),
                T(
                    "Run \"holdem server\" on a computer that stays on: a table with no player of \
               its own, using the [game] and [server] settings of holdem.toml \
               (\"holdem server --port 9000\" picks another port; it prints every setting it \
               uses when it starts). After each game it goes \
               back to the lobby for the next one.",
                ),
                H("JOINING"),
                T(
                    "Choose \"Join a network game\", type your name, then pick a table found on \
               your network or type an address and press Enter. You can join a game that is \
               already running: you get the next free seat with the starting stack (a bot \
               makes room if the table is full).",
                ),
                H("WHILE PLAYING"),
                T(format!(
                    "Each person has a time limit to act ({} seconds by default); when it runs \
                     out they check or fold automatically. After a hand the next one starts \
                     when everyone pressed {}, or after {} seconds. If your connection drops, \
               your hand is folded and you leave the table at the end of the hand; join \
               again with the same name before that to keep your seat.",
                    d.server.turn_seconds, ready, d.server.next_hand_seconds
                )),
                T(
                    "Nobody can see another player's cards before they are turned up: the table \
               only ever sends you what you could see at a real table.",
                ),
                H("VERSIONS"),
                T(
                    "Everyone at a table needs a version of holdem that speaks the same network \
               protocol. Versions are compared as soon as you connect: if yours cannot \
               play with a table, you are told whether you or the host needs to update. \
               Tables found on your network that need a different version are marked on \
               the join screen.",
                ),
            ],
        },
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
        },
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
                P("Win chance", "See the next page."),
                P(
                    "Worth N big blinds",
                    "Shown under the pot: its size divided by the big blind, i.e. how big \
                 the pot is compared with what it costs to play a hand. Players think in big \
                 blinds because chip amounts mean little once the blinds rise.",
                ),
            ],
        },
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
                        "the optional hint (settings, key {}; a table may not allow it), shown \
                 while you are in a hand: how often your hand would win against the same \
                 number of opponents holding random cards, estimated by dealing out \
                 thousands of random boards. Real opponents who bet usually hold better than \
                 random cards, so treat it as a rough guide. Calling tends to pay off when \
                 your win chance is above the pot odds.",
                        settings
                    ),
                ),
                H(format!("YOUR STYLE (KEY {})", profile)),
                T(format!(
                    "The game counts how often you play a hand (put chips in voluntarily before \
               the flop), how often you raise first, how often you bet or raise rather than \
               call, and how often your bets after the flop are made without even a pair. \
               Only hands you start with at least {} big blinds count: with fewer, everybody \
               must play push-or-fold. After {} such hands it compares your numbers with \
               what each bot style really does (measured over thousands of bot hands) and \
               names the closest. Neighbouring styles are similar, so look at the top two.",
                    facts.style_min_stack_bb, facts.style_min_hands
                )),
            ]
            .into_iter()
            .chain(styles)
            .collect(),
        },
    ]
}

/// Small numbers as words, as the text reads better that way.
fn number_word(n: usize) -> String {
    const WORDS: [&str; 11] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), |w| w.to_string())
}
