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

The file has these parts:

| Section | What it sets |
| --- | --- |
| `[client]` | only your own screen: your name, the speed to start with, the win-chance hint, how long messages and the pot flash last, log length, when the style estimate starts |
| `[client.sound]` | volume, the volume step, the pause between sounds |
| `[client.setup]` | the values offered on the setup screen (chips, blinds, blind levels, hand limits, rebuy periods) |
| `[client.join]` | connection timeout and how the join screen searches for tables |
| `[network]` | the port for hosting and `holdem serve` (also the port the join screen assumes) |
| `[bots]` | bot names, and how hard bots think (simulated deals per decision) |
| `[[speeds]]` | the game speeds: as many as you like, each with a name and the milliseconds between bot moves |
| `[[tables]]` | one or more tables, each with a name, seats, bots in free seats, its speed under `holdem serve`, whether the win-chance hint is allowed, the all-in run-out pace, which hands count for the style estimate, and the time limits of network play |
| `[tables.rules]` | a table's rules, including the blind schedule, how fast blinds grow after it, the ante size, the straddle size and minimum players, and the rebuy stack |
| `[tables.fluctuation]` | whether bots come and go at a table, and how often |

### Tables

A table's settings are in its `[[tables]]` entry; anything left out keeps its default. With
several tables, the setup screen gets a *Table* row: ←→ picks one, and its settings fill the
rows below, which you can still change before you deal. `W` saves your changes into that
table's entry. `holdem serve` runs every table at once on one port (see
[Running a server](server.md#several-tables)). Guests at someone else's table use that table's
settings.

```toml
[[tables]]
name = "Hold'em table"

[[tables]]
name = "Turbo"
speed = "Fast"
seats = 4
[tables.rules]        # the rules of the entry above
small_blind = 25
```

A settings file from holdem 0.7 or older uses `[game]` and `[server]` instead; holdem says so
and `holdem config write new.toml` writes the current format to copy your values into.

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
  -p, --port <NUMBER>    instead of network.port
  -n, --name <TEXT>      the table's name (with one table)
      --speed <NAME>     the speed of every table
```

On start-up, `holdem serve` prints every setting it runs with: table, rules and network play.
It also shows which settings file they came from (or that it uses the built-in defaults) and
which values the command line changed.
