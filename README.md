# holdem

No-limit Texas Hold'em in the terminal, written in Rust with [ratatui](https://ratatui.rs).
Play against computer opponents, or with other people over your home network or the internet.

![Your turn after the flop: five opponents around the table, the board, the pot, your cards and your choices](docs/screenshots/table.svg)

## Features

- Up to eight bot opponents, each with one of eight playing styles.
- Multiplayer over your home network or the internet; tables on your network are found
  automatically.
- `holdem serve`: a table anyone can join, for a computer that stays on.
- Optional table rules: antes, straddle, rebuys, dead button, mucking, a hand limit, players
  coming and going.
- A win-chance hint and an estimate of your own playing style.
- Achievements across all your games, some unlocking extra shapes for your symbol.
- Synthesized sound effects (or a build without any audio libraries for servers).
- Everything adjustable in `holdem.toml`, with every setting explained.

## Installing

Every release comes in two versions, for Linux, macOS and Windows. Both install a program
called `holdem`:
- **`holdem`**, the game with sound. On Linux it needs the ALSA library (`libasound2`), which
  every desktop has.
- **`holdem-server`**, the same game without sound and without any audio libraries, for
  dedicated servers. For Linux there are also static `musl` builds that run on any distribution.

`holdem --version` shows which one you have: `holdem 0.10.0 (with sound)` or
`holdem 0.10.0 (without sound)`.

The installers put the program in `~/.cargo/bin`. The archives on the
[releases page](https://github.com/earomc/holdem/releases) contain just the program, to run
from anywhere. Every archive is attested by GitHub: with the [GitHub CLI](https://cli.github.com),
`gh attestation verify <archive> --repo earomc/holdem` checks that it was built by this
repository's release workflow and not changed since.

### With sound (`holdem`), for playing

Linux and macOS:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/earomc/holdem/releases/latest/download/holdem-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/earomc/holdem/releases/latest/download/holdem-installer.ps1 -OutFile "$env:TEMP\holdem-installer.ps1"
powershell -ExecutionPolicy Bypass -File "$env:TEMP\holdem-installer.ps1"
```

Use these two lines rather than the one-line `irm … | iex` form shown on the release page:
Microsoft Defender blocks that one, flagging its command line as `Trojan:Win32/Commando.A!ml`.

### Without sound (`holdem-server`), for servers

Linux and macOS:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/earomc/holdem/releases/latest/download/holdem-server-installer.sh | sh
```

Windows (PowerShell):

```powershell
irm https://github.com/earomc/holdem/releases/latest/download/holdem-server-installer.ps1 -OutFile "$env:TEMP\holdem-server-installer.ps1"
powershell -ExecutionPolicy Bypass -File "$env:TEMP\holdem-server-installer.ps1"
```

On a server, start a table with `holdem serve`; [Running a server](docs/server.md) explains
the rest.

## Uninstalling

```
holdem uninstall           # asks first; your settings and statistics stay unless you say so
holdem uninstall --purge   # also deletes your settings and statistics
holdem uninstall --yes     # does not ask (keeps settings and statistics unless --purge)
```

It removes the program and the installer's receipt, and takes `~/.cargo/bin` off your PATH
if nothing else is in that folder (Rust's own tools live there too). It only works for a copy
the installer put there; a program from an archive you simply delete.

By hand, on Windows (PowerShell):

```powershell
Remove-Item "$env:USERPROFILE\.cargo\bin\holdem.exe", "$env:LOCALAPPDATA\holdem\holdem-receipt.json"
Remove-Item -Recurse "$env:APPDATA\holdem"   # your settings and statistics, if you want
```

Take `%USERPROFILE%\.cargo\bin` off your PATH (Settings → System → About → Advanced system
settings → Environment Variables) only if nothing else is in that folder.

On Linux and macOS:

```sh
rm ~/.cargo/bin/holdem ~/.config/holdem/holdem-receipt.json
rm -r ~/.config/holdem                         # Linux: your settings and statistics, if you want
rm -r ~/Library/Application\ Support/holdem    # macOS: the same
```

The installer added a line `. "$HOME/.cargo/env"` to your shell's startup files; remove it
only if nothing else is in `~/.cargo/bin`. The `holdem-server` release is removed the same
way, with `holdem-server` in place of `holdem` in the receipt's folder and name.

## Quick start

```
holdem                  # the main menu: play against bots, host a game, or join one
holdem serve            # a table anyone can join, without playing yourself
holdem config write     # write holdem.toml with every setting explained
```

Press `?` in the game for the rules, the keys and a glossary of poker terms. The terminal
needs to be at least 80×23 (80×28 with six or more players).

## Documentation

- [Playing](docs/playing.md): ways to play, hosting and joining, controls, table options, your
  playing style.
- [Poker rules and terms](docs/poker.md): the rules implemented, a glossary, the bot styles.
- [Configuration](docs/configuration.md): the settings file and the command line.
- [Running a server](docs/server.md): `holdem serve`, the network, its log, running it as a
  service.
- [Contributing](CONTRIBUTING.md): building, tests, releasing, how the code is organized.
- [Changelog](CHANGELOG.md): what changed in each version.

## License

MIT, see [LICENSE](LICENSE).
