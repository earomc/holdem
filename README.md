# holdem

No-limit Texas Hold'em in the terminal, written in Rust with [ratatui](https://ratatui.rs).
Play against up to eight computer opponents, each with its own style, or with other people
over your home network or the internet. Optional table rules (antes, straddle, rebuys, dead
button, mucking, a hand limit, players coming and going), synthesized sound effects, and an
estimate of your own playing style.

```
cargo run --release                         # play
cargo run --release -- server               # a table anyone can join, without playing yourself
cargo run --release -- config write        # write holdem.toml with every setting explained
cargo test                                  # rules, server, bots, interface and sound tests
cargo test --release -- --ignored           # also check all 133,784,560 seven-card hands (slow)
cargo build --release --no-default-features # a silent build without audio dependencies
```

Requirements: Rust 1.88 or newer, and a terminal of at least 80×23 (80×28 with six or more
players). On Linux the sound feature needs the ALSA development files:
`sudo apt install libasound2-dev pkg-config` (Debian/Ubuntu) or
`sudo dnf install alsa-lib-devel` (Fedora). macOS and Windows need nothing extra. Without them,
build with `--no-default-features`.

## Ways to play

**Against bots.** *Play against bots* opens the setup screen: seats, chips, blinds and the
table options below. Enter deals.

**Hosting.** *Host a network game* opens your table to others. The lobby shows the address to
give them (for example `192.168.1.20:7777`). People on the same network also find your table
automatically on their join screen. The game starts when everyone in the lobby is ready, and
bots take the free seats unless you switch that off. For players on the internet, forward TCP
port 7777 (or the port in `holdem.toml`) to your computer in your router.

**A server anyone can join.** `holdem server` runs a table with no player of its own (for
example on a computer that stays on), using the `[game]` and `[server]` settings
of `holdem.toml`. `--port`, `--name` and `--speed` override single values, and on start-up it prints every
setting it uses and the file they came from. After each game the table goes back
to its lobby.

**Joining.** *Join a network game*: type your name, then pick a table found on your network or
type an address (`host` or `host:port`). You can join a game that is already running: you get
the next free seat with the starting stack (a bot makes room if the table is full).

While several people play, each has a time limit to act (30 s by default), and the next hand
starts when everyone has pressed Enter or after a few seconds. If your connection drops, your
hand is folded and you leave the table when the hand ends. Join again with the same name
before then to keep your seat. The server only ever sends you what you could see at a real
table: nobody receives another player's hole cards before they are shown.

## Table options

These can be changed on the setup screen. Their starting values come from `[game]` and
`[game.rules]` in `holdem.toml`. The optional rules are off by default; the win-chance hint is
allowed by default.

| Option | What it does |
| --- | --- |
| Hand limit | The game ends after this many hands. Players still in are ranked by chips. |
| Antes | Everyone also pays an ante every hand (1/8 of the big blind by default). Antes are dead money: you still owe the full big blind to play. |
| Straddle | With enough players (4 by default), the player left of the big blind posts a straddle (two big blinds by default) and acts last before the flop. The minimum raise is then to twice the straddle. |
| Rebuys | A busted player may buy back in (for the starting stack by default) during the first N blind levels. You choose after the hand (Y / Enter); bots decide by their style. |
| Button rule | *Dead button*: the big blind moves exactly one player each hand, and the small blind and button follow it, even onto empty seats. Nobody skips a big blind when players leave. |
| Showdown | *Losers may muck*: hands are shown in order (the last river bettor first) and players already beaten throw their cards away unseen. All-in hands are always shown. |
| Win-chance hint | Whether players at the table may turn on the win-chance hint. When it is not allowed, the settings window shows that and it cannot be switched on. |
| Players come and go | Bots leave now and then with their chips; new bots sit down in empty seats. The chances are in `[game.fluctuation]`. |

## Your playing style

Press **P** at the table, or look at the final standings. The game counts four standard
statistics:
- how often you play a hand (voluntarily put chips in before the flop);
- how often you raise first;
- how often you bet or raise rather than call;
- how often your post-flop bets come without even a pair.

It then names the closest of the eight bot styles. Only hands you start with at least 15 big
blinds count, because with a short stack everyone has to play push-or-fold. An estimate
appears after 15 such hands.

The reference values for each style are measured, not guessed. `cargo run --release --example
calibrate` lets the bots play about 110,000 hands and prints them. A test checks that every
style is recognized from its own play: on samples of 200 hands, the right style comes first
83% of the time and is among the top two 98% of the time. Neighbours such as Rock and Nit are
genuinely similar.

## Settings file

`holdem.toml` is read from the current folder, from next to the program, or from the file given
with `--config`. Every line is optional: missing settings keep their defaults.
`holdem config write` writes a file with every setting. Every setting is explained; for the
on/off ones the comment says what `true` and what `false` does, and each ends with its default
spelled out. The file is generated from the program's own defaults, so it cannot disagree
with them.

holdem does not write a settings file by itself. A file written automatically would freeze
today's defaults: settings you never touched would no longer improve with a newer holdem, and
the program would leave files in whatever folder it was started from (or fail where it may
not write, as on a server). Writing one is a deliberate step, and the file can be trimmed to
just the settings you change.

`holdem config show` prints the settings in effect and which file they came from, and
`holdem config check [FILE]` checks a file without starting anything and lists the settings
it changes.

The file has three parts, by who reads them:

| Section | Read by | What it sets |
| --- | --- | --- |
| `[client]` | only your own screen | your name, the speed to start with, the win-chance hint, how long messages and the pot flash last, log length, when the style estimate starts |
| `[client.sound]` | | volume, the volume step, the pause between sounds |
| `[client.setup]` | | the values offered on the setup screen (chips, blinds, blind levels, hand limits, rebuy periods) |
| `[client.join]` | | connection timeout and how the join screen searches for tables |
| `[game]` | any table this program runs: solo, hosted, or `holdem server` | seats, bots in free seats, whether the win-chance hint is allowed, the all-in run-out pace, which hands count for the style estimate |
| `[[game.speeds]]` | | the game speeds: as many as you like, each with a name and the milliseconds between bot moves |
| `[game.bots]` | | bot names, and how hard bots think (simulated deals per decision) |
| `[game.rules]` | | the rules, including the blind schedule, how fast blinds grow after it, the ante size, the straddle size and minimum players, and the rebuy stack |
| `[game.fluctuation]` | | whether bots come and go, and how often |
| `[server]` | network play only: hosting and `holdem server` | table name, port (also the port the join screen assumes), the speed of `holdem server`, time to act, pause between hands, how long a dropped player has |

The `[game]` settings are not split into "client" and "server" parts, because the program
that runs a table reads them: your own program in solo and hosted games, the server in
`holdem server` games. Guests at someone else's table use that table's settings.

Not configurable: the internals of algorithms, such as how bots weigh their decisions, the
hand evaluator, the measured style reference values and the sound synthesis. There, a number
only means something together with the code around it.

Mistakes are reported, not ignored. A misspelled setting, a value out of range, a speed name
that is not in the list, or an empty list of choices all give an error with the line number.
A file from an older holdem is refused with a hint to write a fresh one.

## Command line

```
holdem [OPTIONS]                 play
holdem server [OPTIONS]          run a table that anyone can join, without playing yourself
holdem config write [FILE]       write a settings file with every setting explained
                                 (default: the --config file, or holdem.toml; --force replaces it)
holdem config show               print the settings in effect and where they come from
holdem config check [FILE]       check a settings file (default: the one holdem would use)

  -c, --config <FILE>    settings file to use
      --seed <NUMBER>    fixed random seed: the same deals every game
      --mute             do not open the audio device
server:
  -p, --port <NUMBER>    instead of server.port
  -n, --name <TEXT>      instead of server.name
      --speed <NAME>     instead of server.speed
```

On start-up, `holdem server` prints every setting it runs with: table, rules and network play.
It also shows which settings file they came from (or that it uses the built-in defaults) and
which values the command line changed.

## Versions

Every holdem speaks a range of network protocol versions. When you connect, the two programs
agree on the newest version both speak. If there is none, you are told in plain words whether
you or the host needs to update, instead of the game failing in odd ways. The join screen marks
tables found on your network that need another version. Within a protocol version, messages
tolerate missing and extra fields, and a message of an unknown kind is skipped or answered with
a notice rather than ending the connection. Versions 0.4 and 0.5 speak protocol 3; holdem 0.3 spoke
protocol 2 and is told to update.

## Controls

Every key is also listed on the first page of the in-game help (`?`).

| Where | Keys |
| --- | --- |
| Main menu | `↑` `↓` choose, `Enter` select, `?`/`H`/`F1` help, `Q`/`Esc` quit |
| Setup | `↑` `↓` choose, `←` `→` change, `Enter` deal / open the table, `Esc`/`Q` back |
| Join screen | `↑` `↓`/`Tab` move between name, address and found tables, type to edit, `Enter` join, `Esc` back or cancel, `F1` help |
| Lobby | `Enter` ready / not ready, `S` settings, `?`/`H` help, `Q`/`Esc` leave |
| Your turn | `F` fold, `C` check/call, `R`/`B` raise box, `A` all-in (press twice) |
| At the table | `Enter`/`N` next hand (ready), `Y` rebuy when offered, `L` log, `P` your style, `S` settings (speed, volume, win-chance hint), `?`/`H` help, `Q`/`Esc` leave |
| Raise box | digits and `Backspace` type the total, `M` min, `H` half pot, `P` pot, `A` all-in, `↑` `↓` ± one big blind, `Enter` confirm, `Esc` cancel |
| Final standings | `Enter` main menu (solo) or play again (network), `P` your style, `L` log, `?`/`H` help, `Q`/`Esc` leave |
| Anywhere | `Ctrl-C` quits immediately |

Leaving offers *back to the main menu*, *quit* or *keep playing*. In a solo game, the game pauses
while any window (help, log, settings, style, leave) is open.

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

| Term | Meaning |
| --- | --- |
| Aggressive | Playing by betting and raising rather than checking and calling. |
| Ante | A small forced bet every player pays each hand (optional rule). |
| All-in | Having bet all your remaining chips. You stay in to the end but can only win what you matched. |
| Bet | Putting chips in when nobody else has in the current betting round. |
| Bet to match | On the table: the biggest bet in this round, which everyone must call to stay in. |
| Betting round | One pass of decisions around the table; a hand has up to four (pre-flop, flop, turn, river). |
| Big blind (BB) | The larger forced bet, posted by the second player left of the button. Also the smallest bet allowed. |
| Blinds | The forced bets posted before the cards are dealt. |
| Blind level | The current size of the blinds; levels go up during the game if that option is chosen. |
| Bluff | Betting with a weak hand to make better hands fold. |
| Board | The community cards in the middle of the table. |
| Burn card | A card put aside face down before the flop, the turn and the river; never used. |
| Button (D) / dealer | The dealer position. It acts last after the flop and moves one seat clockwise every hand. |
| Call | Matching the current bet. |
| Check | Passing without betting; only allowed when nobody has bet this round. |
| Chips / stack | The tokens used for betting; a player's chips in front of them are their stack. |
| Community cards | The five shared cards (flop, turn, river) that every player can use. |
| Dead button | A rule that moves the big blind exactly one player per hand, so the button may sit on an empty seat. |
| Eliminated | Out of the game after losing all chips. |
| Flop | The first three community cards, dealt together; also the second betting round. |
| Flush | Five cards of the same suit. |
| Fold | Giving up the hand and the chips already bet. |
| Four of a kind (quads) | Four cards of the same rank. |
| Full house | Three of a kind plus a pair. "Kings full of Fours" is K K K 4 4. |
| Hand limit | A maximum number of hands, after which the biggest stack wins. |
| Heads-up | Only two players in the hand or left in the game. |
| High card | A hand with no pair or better, named after its highest card. |
| Hole cards | Your two private cards. |
| Host | The person whose computer runs a network table. |
| Kicker | A card that is not part of the pair, trips etc. but still breaks ties. |
| Limp | Just calling the big blind before the flop instead of raising. |
| Lobby | The waiting room of a network table before a game starts. |
| Loose / tight | Playing many hands / playing only a few good ones. |
| Main pot / side pot | The pot everyone still in can win / an extra pot for chips an all-in player could not match, which only the players who put chips into it can win. |
| Minimum raise | The smallest legal raise: the size of the last bet or raise in the round, at least one big blind. |
| Muck | Throwing your cards away face down, unseen (when folding, or at a showdown you cannot win if mucking is allowed). |
| Nuts | The best possible hand with the cards on the board. |
| Odd chip | A chip left over when a pot is split; it goes to the first winner left of the button. |
| Offsuit / suited | Two hole cards of different suits (A♠ K♦, "Ace-King offsuit") / of the same suit (J♥ 10♥, "Jack-Ten suited"). |
| Passive | Checking and calling rather than betting and raising. |
| Pocket pair | Two hole cards of the same rank, e.g. "Pocket Kings". |
| Position | Where you sit relative to the button; acting later (near the button) is an advantage. |
| Post | Putting a blind, ante or straddle into the pot. |
| Pot | All the chips bet so far in the current hand. |
| Pot odds | The share of the final pot that a call costs: call / (pot + call). A call pays off in the long run if you win more often than that. |
| Pre-flop / post-flop | The first betting round, before any community card / the flop, turn and river rounds. |
| Push-or-fold | With a short stack, the only sensible plays are going all-in or folding. |
| Raise | Making the current bet bigger. "Raise to 120" means your total bet in this round becomes 120. |
| Rebuy | Buying back in for the starting stack after losing all your chips (optional rule). |
| Reopen the betting | A full raise lets players who already acted raise again; a short all-in does not. |
| River | The fifth and last community card; also the last betting round. |
| Royal flush | A K Q J 10 of one suit, the best hand there is. |
| Short stack | A stack that is small compared to the blinds. |
| Showdown | Showing the hands after the last betting round to decide the winner. |
| Slow play | Checking or just calling with a strong hand to hide its strength. |
| Small blind (SB) | The smaller forced bet (half the big blind), posted by the player left of the button. |
| Split pot | A pot shared by players with equal hands. |
| Straddle (STR) | An optional third blind of two big blinds, posted by the player left of the big blind, who then acts last before the flop. |
| Straight / straight flush | Five consecutive ranks in mixed suits / all of one suit. |
| Street | A stage of the hand: pre-flop, flop, turn or river. |
| Three of a kind (trips, set) | Three cards of the same rank. |
| Turn | The fourth community card; also the third betting round. |
| Two pair | Two different pairs. |
| Uncalled bet | Chips nobody matched; they are returned to the bettor. |
| Under the gun | The first player to act before the flop, left of the big blind (or of the straddle). |
| Wheel | 5 4 3 2 A, the lowest straight. |
| Win chance | The optional hint (in the settings, key `S`; a table may not allow it), shown while you are in a hand: how often your hand would win against the same number of opponents holding random cards, estimated by dealing thousands of random boards. A rough guide only. |
| Worth N big blinds | Shown under the pot: its size divided by the big blind, i.e. how big the pot is compared with what it costs to play a hand. Players think in big blinds because chip amounts mean little once the blinds rise. |

**Opponent styles** (revealed in the final standings):
- *Rock*: tight and cautious, calls more than it raises;
- *Nit*: plays almost only premium hands;
- *Tight-aggressive*: few hands, played forcefully;
- *Shark*: solid and balanced;
- *Loose-aggressive*: many hands, lots of raising;
- *Gambler*: many hands and frequent bluffs;
- *Calling station*: calls a lot, rarely raises;
- *Maniac*: raises and bluffs constantly.

## How it is built

Every game runs on the same small **table server**, whether you play alone, host, or join.
Your screen is a client of it: through memory when the table runs in your own program, and
through TCP (one JSON message per line) for everyone else. One code path drives the game for
all three cases. The server is authoritative, so players cannot cheat by changing their
client.

| File | Purpose |
| --- | --- |
| `src/card.rs`, `src/eval.rs` | cards, deck, shuffling; the 5-to-7-card hand evaluator |
| `src/game.rs` | the rules as a state machine: betting, pots, showdown, options, seats, standings |
| `src/bot.rs` | bot styles (each with its description and measured statistics), equity simulation, decisions |
| `src/stats.rs` | playing statistics and the style estimate |
| `src/table.rs` | the table server's logic: seating, bots' timing, time limits, rebuys, players coming and going, per-player views |
| `src/protocol.rs` | the messages between table and clients, and what each player may see |
| `src/net.rs` | running a table on a thread, TCP connections, LAN discovery |
| `src/config.rs` | `holdem.toml`: reading, checking, and writing it with every setting explained |
| `src/main.rs`, `src/app.rs`, `src/session.rs` | command line, screen state and input, the client's side of a game |
| `src/keys.rs` | every key binding in one table: input, the key reminders on screen and the Keys help page all come from it |
| `src/ui.rs`, `src/popups.rs`, `src/help.rs` | drawing, windows, help text (generated where it quotes keys, styles or defaults) |
| `src/sound.rs` | sound synthesis and playback |
| `tests/` | rule scenarios, a side-pot oracle, random-play fuzzing across all options, style recognition |
| `examples/calibrate.rs`, `examples/strength.rs` | the measurements behind the bot tuning and the style estimate |
