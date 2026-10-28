# Configuration

The settings file `holdem.toml` and the command line. Back to the [README](../README.md).

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
| `[game]` | any table this program runs: solo, hosted, or `holdem serve` | seats, bots in free seats, whether the win-chance hint is allowed, the all-in run-out pace, which hands count for the style estimate |
| `[[game.speeds]]` | | the game speeds: as many as you like, each with a name and the milliseconds between bot moves |
| `[game.bots]` | | bot names, and how hard bots think (simulated deals per decision) |
| `[game.rules]` | | the rules, including the blind schedule, how fast blinds grow after it, the ante size, the straddle size and minimum players, and the rebuy stack |
| `[game.fluctuation]` | | whether bots come and go, and how often |
| `[server]` | network play only: hosting and `holdem serve` | table name, port (also the port the join screen assumes), the speed of `holdem serve`, time to act, pause between hands, how long a dropped player has |

The `[game]` settings are not split into "client" and "server" parts, because the program
that runs a table reads them: your own program in solo and hosted games, the server in
`holdem serve` games. Guests at someone else's table use that table's settings.

Not configurable: the internals of algorithms, such as how bots weigh their decisions, the
hand evaluator, the measured style reference values and the sound synthesis. There, a number
only means something together with the code around it.

Mistakes are reported, not ignored. A misspelled setting, a value out of range, a speed name
that is not in the list, or an empty list of choices all give an error with the line number.
A file from an older holdem is refused with a hint to write a fresh one.

## Command line

```
holdem [OPTIONS]                 play
holdem serve [OPTIONS]           run a table that anyone can join, without playing yourself
holdem config write [FILE]       write a settings file with every setting explained
                                 (default: the --config file, or holdem.toml; --force replaces it)
holdem config show               print the settings in effect and where they come from
holdem config check [FILE]       check a settings file (default: the one holdem would use)

  -c, --config <FILE>    settings file to use
      --seed <NUMBER>    fixed random seed: the same deals every game
      --mute             do not open the audio device (not in the holdem-server build)
serve:
  -p, --port <NUMBER>    instead of server.port
  -n, --name <TEXT>      instead of server.name
      --speed <NAME>     instead of server.speed
```

On start-up, `holdem serve` prints every setting it runs with: table, rules and network play.
It also shows which settings file they came from (or that it uses the built-in defaults) and
which values the command line changed.
