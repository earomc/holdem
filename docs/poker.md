# Poker rules and terms

The rules holdem implements, a glossary of poker terms and the bot styles. The in-game help
(`?`) has the same. Back to the [README](../README.md).

## Rules implemented

* Dealer button, small and big blinds, burn cards before the flop, turn and river, and the
  heads-up exception (the button posts the small blind, acts first before the flop and last
  after it).
* No-limit betting: the minimum raise equals the previous bet or raise in the round, the
  opening bet is at least one big blind, any raise up to the whole stack. Players who still
  have to act owe the full big blind even if the big blind is all-in for less.
* Short all-ins: an all-in smaller than a full raise does not change the minimum raise and
  does not reopen the betting to players who already acted, unless several short all-ins add
  up to a full raise.
* Calling for less, main pot and any number of side pots, dead money from folded players and
  antes staying in the pots they were paid into, uncalled bets returned.
* Showdown with all nine hand categories (the wheel A-2-3-4-5 is the lowest straight, two sets
  of trips make a full house), kickers, split pots and the odd-chip rule (first winner
  clockwise from the left of the button). Optional mucking in proper showing order.
* Game structure: rising blind levels, optional hand limit, antes, straddle, rebuys, dead
  button, eliminations with correct places (simultaneous busts ranked by starting stack),
  players joining and leaving between hands, final standings.

## Glossary

The in-game help has the same explanations, plus the hand rankings and the rules.

<!-- generated glossary: edit src/help, then run `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` -->
| Term | Meaning |
| --- | --- |
| Aggressive | Playing by betting and raising rather than checking and calling. |
| All-in | Having bet all your remaining chips. You stay in the hand to the end but can only win what you matched. |
| Ante | A small forced bet every player pays each hand (an optional rule). |
| Bet | Putting chips in when nobody else has in the current betting round. |
| Bet to match | On the table: the biggest bet in this round, which everyone must call to stay in. |
| Betting round | One pass of decisions around the table. A hand has up to four: pre-flop, flop, turn and river. |
| Big blind (BB) | The larger forced bet, posted by the second player to the left of the button. It is also the smallest bet allowed. |
| Blinds | The forced bets posted before the cards are dealt. |
| Blind level | The current size of the blinds. Levels go up during the game if that option is chosen. |
| Bluff | Betting with a weak hand to make better hands fold. |
| Board | The community cards in the middle of the table. |
| Burn card | A card dealt face down off the deck before the flop, the turn and the river, and never used. |
| Button (D) | The dealer position. It acts last after the flop and moves one seat clockwise every hand. |
| Call | Matching the current bet. |
| Check | Passing without betting; allowed only when nobody has bet this round. |
| Chips | The tokens used for betting. Your chips in front of you are your stack. |
| Community cards | The five shared cards (flop, turn, river) every player can use. |
| Dead button | A rule that moves the big blind exactly one player per hand, so the button may sit on an empty seat. |
| Dealer | See button. Here nobody actually deals: the button just marks the position. |
| Eliminated | Out of the game after losing all chips. |
| Flop | The first three community cards, dealt together; also the second betting round. |
| Flush | Five cards of the same suit. |
| Fold | Giving up the hand and the chips already bet. |
| Four of a kind (quads) | Four cards of the same rank. |
| Full house | Three of a kind plus a pair. "Kings full of Fours" is K K K 4 4. |
| Hand limit | A maximum number of hands, after which the biggest stack wins. |
| Heads-up | Only two players in the hand or left in the game. |
| High card | A hand with no pair or better; named after its highest card. |
| Hole cards | Your two private cards. |
| Host | The person whose computer runs a network table. |
| Kicker | A card that is not part of the pair, trips, etc. but still breaks ties. |
| Limp | Just calling the big blind before the flop instead of raising. |
| Lobby | The waiting room of a network table before a game starts. |
| Loose | Playing many hands. The opposite of tight. |
| Main pot | The pot every player still in can win. See side pot. |
| Minimum raise | The smallest legal raise: the size of the last bet or raise in this round, at least one big blind. |
| Muck | To throw your cards away face down, unseen (when folding, or at a showdown you cannot win if mucking is allowed). |
| Nuts | The best possible hand with the cards on the board. |
| Odd chip | A chip left over when a pot is split; it goes to the first winner to the left of the button. |
| Offsuit | Two hole cards of different suits, e.g. A♠ K♦ ("Ace-King offsuit"). |
| Passive | Checking and calling rather than betting and raising. |
| Pocket pair | Two hole cards of the same rank, e.g. "Pocket Kings". |
| Position | Where you sit relative to the button. Late position (near the button) acts later and is an advantage. |
| Post | Putting a blind, ante or straddle into the pot. |
| Post-flop | The flop, turn and river betting rounds. |
| Pot | All the chips bet so far in the current hand. |
| Pot odds | The share of the final pot that a call costs: call / (pot + call). A call pays off in the long run if your hand wins more often than that. |
| Pre-flop | The first betting round, after the hole cards and before the flop. |
| Push-or-fold | With a short stack, the only sensible plays are going all-in or folding. |
| Raise | Making the current bet bigger. "Raise to 120" means your total bet in this round becomes 120. |
| Rebuy | Buying back in for the starting stack after losing all your chips (an optional rule). |
| Reopen | A full raise gives players who already acted the right to raise again; a short all-in does not. |
| River | The fifth and last community card; also the last betting round. |
| Royal flush | A K Q J 10 of one suit: the best hand there is. |
| Short stack | A stack that is small compared to the blinds. |
| Showdown | Showing the hands after the last betting round to decide who wins. |
| Side pot | An extra pot for chips that an all-in player could not match. Only players who put chips into it can win it. |
| Slow play | Checking or just calling with a strong hand to hide its strength. |
| Small blind (SB) | The smaller forced bet (half the big blind), posted by the player to the left of the button. |
| Split pot | A pot shared by players with equal hands. |
| Stack | The chips a player has in front of them. |
| Straddle (STR) | An optional third blind (two big blinds by default), posted by the player left of the big blind, who then acts last pre-flop. |
| Straight | Five cards of consecutive ranks in mixed suits. |
| Straight flush | Five cards of consecutive ranks, all of one suit. |
| Street | A stage of the hand: pre-flop, flop, turn or river. |
| Suited | Two hole cards of the same suit, e.g. J♥ 10♥ ("Jack-Ten suited"). |
| Tight | Playing few hands, only good ones. The opposite of loose. |
| Trips / set | Three of a kind. |
| Turn | The fourth community card; also the third betting round. |
| Two pair | Two different pairs. |
| Uncalled bet | Chips nobody matched; they are returned to the bettor. |
| Under the gun | The first player to act before the flop, left of the big blind (or of the straddle). |
| Wheel | 5 4 3 2 A, the lowest straight. |
| Win chance | The optional hint shown while you are in a hand: how often your hand would win against the same number of opponents holding random cards. Turn it on in the settings (key S), where the table allows it. |
| Worth N big blinds | Shown under the pot: its size divided by the big blind, i.e. how big the pot is compared with what it costs to play a hand. Players think in big blinds because chip amounts mean little once the blinds rise. |
<!-- end of generated text -->

## Opponent styles

Revealed in the final standings:

<!-- generated styles: edit src/help, then run `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` -->
- *Rock*: tight and cautious: few hands, mostly called rather than raised;
- *Nit*: plays almost nothing but premium hands;
- *Tight-aggressive*: few hands, played forcefully;
- *Shark*: solid and balanced, the hardest to read;
- *Loose-aggressive*: many hands, lots of raising;
- *Gambler*: plays many hands and bluffs often;
- *Calling station*: calls a lot, rarely raises, hard to bluff;
- *Maniac*: raises and bluffs constantly.
<!-- end of generated text -->
