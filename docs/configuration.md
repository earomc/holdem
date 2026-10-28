# Configuration

The settings file `holdem.toml` and the command line. Back to the [README](../README.md).

## Settings file

`holdem.toml` is read from the file given with `--config`, or else from the first of these
that exists:

1. the current folder;
2. the folder of the program;
3. your own settings folder: `%APPDATA%\holdem` on Windows,
   `~/Library/Application Support/holdem` on macOS, `$XDG_CONFIG_HOME/holdem` or
   `~/.config/holdem` on Linux.

Every line is optional: missing settings keep their defaults.
`holdem config write` writes a file with every setting. Every setting is explained; for the
on/off ones the comment says what `true` and what `false` does, and each ends with its default
spelled out. The file is generated from the program's own defaults, so it cannot disagree
with them.

### Saving from the game

Press `W` on the setup screen or in the settings window (`S`) to save what you changed as
your defaults: your name, the speed, the volume, the win-chance hint and the setup screen's
table options. holdem updates the file it loaded, or creates `holdem.toml` in your settings
folder if there was none, and says where it saved.

Saving changes only those settings, in place: your comments and the rest of the file stay as
they are. A setting that is not in the file yet is only added when it differs from its
default. That matters, because a value written down is frozen: settings you never touched
would no longer improve with a newer holdem.

Otherwise holdem does not write a settings file by itself, only when you press `W` or run
`holdem config write`. `holdem config write --force` replaces an existing file with the fully
explained format and keeps its values.

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
| `[[server.tables]]` | | several tables for `holdem serve`, each with a name and its own seats, speed, starting chips, small blind or free seats (see [Running a server](server.md#several-tables)) |

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
