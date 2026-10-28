# Playing holdem

How to play against bots and with other people, the keys, the table options and the
playing-style estimate. Back to the [README](../README.md).

## Ways to play

### Against bots

*Play against bots* opens the setup screen: seats, chips, blinds and the
table options below. Enter deals.

### Hosting

*Host a network game* opens your table to others. The lobby shows the address to
give them (for example `192.168.1.20:7777`). People on the same network also find your table
automatically on their join screen. The game starts when everyone in the lobby is ready, and
bots take the free seats unless you switch that off. For players on the internet, forward TCP
port 7777 (or the port in `holdem.toml`) to your computer in your router.

### A server anyone can join

`holdem serve` runs a table with no player of its own (for example on a computer that stays
on), using the `[game]` and `[server]` settings of `holdem.toml`
([Configuration](configuration.md)). `--port`, `--name` and `--speed` override single values,
and on start-up it prints every setting it uses and the file they came from. After each game
the table goes back to its lobby.

### Joining

*Join a network game*: type your name, then pick a table found on your network or
type an address (`host` or `host:port`). You can join a game that is already running: you get
the next free seat with the starting stack (a bot makes room if the table is full).

While several people play, each has a time limit to act (30 s by default), and the next hand
starts when everyone has pressed Enter or after a few seconds. If your connection drops, your
hand is folded and you leave the table when the hand ends. Join again with the same name
before then to keep your seat. The server only ever sends you what you could see at a real
table: nobody receives another player's hole cards before they are shown.

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

## Table options

These can be changed on the setup screen. Their starting values come from `[game]` and
`[game.rules]` in `holdem.toml` ([Configuration](configuration.md)). The optional rules are off by default; the win-chance hint is
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

How the style reference values are measured is described in [CONTRIBUTING](../CONTRIBUTING.md#tests).
