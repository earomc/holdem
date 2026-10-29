# Changelog

All notable changes to holdem. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- Choose your own symbol: `Enter` on the *Your symbol* row of the setup or join screen opens
  a window with every shape and colour; shapes not unlocked yet stay hidden behind `▒`.
  Everyone at the table sees your symbol, and `W` saves it as `client.symbol`. Without a
  choice you keep the one your name gives.
- Achievements: 21 of them, many in stages (Winner I, II, III), from winning hands and games
  to bluffs, big pots, made hands up to a royal flush, comebacks and games with other people.
  *Achievements* in the main menu shows each with its progress; a notice says when you earn
  one. Eight stages unlock extra shapes for your symbol. Progress is kept with your statistics
  in `stats.toml`.
- `holdem uninstall` removes a copy the release installer put in place: the program, the
  installer's receipt, and `~/.cargo/bin` from your PATH if nothing else is in it. It asks
  first, and keeps your settings and statistics unless you choose otherwise (`--purge`
  deletes them, `--yes` does not ask). The README also lists the steps by hand.

### Fixed
- At someone else's table the settings window shows the table's own speed, and changing it
  says that the table sets the speed. Before, the speed seemed to change but did nothing.
- The Windows install commands in the README download the installer first and then run it:
  Microsoft Defender blocks the one-line `irm … | iex` form (as `Trojan:Win32/Commando.A!ml`).

## [0.9.0] - 2026-10-28

### Added
- Every player has a colored symbol next to their name, on the seats and in the lobby. It
  follows from the name, so the same name always gets the same symbol.
- When a kicker decides a pot, the result says so: "wins the pot (840) with Pair of Kings,
  Ace kicker".
- **Game info** (`L`): one window with three tabs, *Table log*, *Your hands* and *Your style*
  (`Tab` or `←` `→` switch). `L` opens the tab you looked at last, `P` opens *Your style*.
- *Your hands*: every hand you were dealt, with the chips it won or cost, your cards, your hand
  at a showdown and how it ended ("folded on the turn", "bluff: everyone folded"), and a line
  for the whole game. The panel beside the table shows whichever log tab you looked at last.
- Your playing style is counted over all your games too, in `stats.toml` in your settings
  folder; the style tab shows the estimate for this game and for all games.

### Changed
- Slower game speeds: Slow 2000 ms, Normal 1200 ms, Fast 600 ms, and a new Turbo at 200 ms.
- The explanation under a setup row follows the value shown: switch between moving and dead
  button, or between the showdown rules, and it says what that choice does. *Never*, *no limit*
  and *off* are explained too.
- Names that differ only in case count as the same: no bot is called "Ava" next to "ava", and
  a second "ANN" joining after "Ann" becomes "ANN 2".
- A rough style estimate shows after 5 counted hands, saying it gets more accurate with more;
  from `client.style_min_hands` (15) on it counts as reliable.
- The final standings say how long each player was at the table: "out after 20 hands
  (12–31)", "left after 3 hands (1–3), 900", "1,250 chips (from hand 12)". People's rows show
  the style they are closest to ("≈ Shark", with a `?` while rough) instead of "(person)".
- The log window and the style window are now tabs of game info.

### Fixed
- After you act, the next bot waits a full step, as it does after another bot. Before, it moved
  at once.

## [0.8.0] - 2026-10-28

### Added
- One server, several tables: `holdem serve` runs every `[[tables]]` entry of `holdem.toml`
  on the one port. Players see each table on their join screen, or choose from the server's
  list after typing its address. The log names the table.
- At home the tables are presets: with several, the setup screen gets a *Table* row that
  fills in that table's settings, and `W` saves into the chosen table.

### Changed
- The settings file is built around tables: `[client]`, `[network]` (the port), `[bots]`,
  `[[speeds]]`, and one or more `[[tables]]` with `[tables.rules]` and `[tables.fluctuation]`.
  Files from 0.7 or older (`[game]`, `[server]`) are refused with a hint:
  `holdem config write new.toml` writes the new format.
- Text in every box wraps instead of being cut off at the edge.
- Network protocol 4 (holdem still speaks 3): players with holdem 0.7 or older can still join a
  server with several tables, and are seated at its first table.

## [0.7.0] - 2026-10-28

### Added
- Save your settings from the game: `W` on the setup screen or in the settings window writes
  your name, speed, volume, win-chance hint and table options to `holdem.toml`. Only those
  settings change; comments and the rest of the file stay.
- holdem also looks for `holdem.toml` in your own settings folder (`%APPDATA%\holdem`,
  `~/Library/Application Support/holdem` or `~/.config/holdem`); saving creates it there.
- `holdem serve` prints what happens at the table, with the time: people joining, leaving and
  coming back, games starting and ending, programs turned away for their version.
- [Running a server](docs/server.md): installing, the network, the log, a systemd service.
- Release archives are attested by GitHub: `gh attestation verify <archive> --repo earomc/holdem`
  shows that a download was built by this repository's release workflow.

### Changed
- `holdem config write --force` keeps the values of the file it replaces.
- The in-game glossary explains "Two pair" and "Four of a kind", and "Win chance" in its own
  words. The glossary in `docs/poker.md` is now generated from it, so the two always agree.

## [0.6.1] - 2026-10-28

### Added
- MIT `LICENSE` file, also included in the release archives.
- `CHANGELOG.md`; its section for a version becomes the release notes.
- A *Your name* row on the setup screen, for games against bots and hosted games. It starts
  with `client.name` from `holdem.toml` (or your login name) and is shared with the join screen.

### Changed
- The README is shorter; the details moved to `docs/` and `CONTRIBUTING.md`.

### Fixed
- The main menu shows the whole note about the settings file, wrapped over several lines,
  instead of cutting it off.

## [0.6.0] - 2026-10-28

### Added
- Prebuilt releases for Linux, macOS and Windows, with one-line installers.
- A second release, `holdem-server`: the same program without sound and without any audio
  libraries, for dedicated servers. It also comes as static musl builds for Linux.
- `holdem --version` and `holdem --help` say whether the build has sound.

### Changed
- The `server` command is now `holdem serve`.
- `--mute` only exists in the build with sound.

Versions before 0.6.0 are not listed here.

[Unreleased]: https://github.com/earomc/holdem/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/earomc/holdem/releases/tag/v0.9.0
[0.8.0]: https://github.com/earomc/holdem/releases/tag/v0.8.0
[0.7.0]: https://github.com/earomc/holdem/releases/tag/v0.7.0
[0.6.1]: https://github.com/earomc/holdem/releases/tag/v0.6.1
[0.6.0]: https://github.com/earomc/holdem/releases/tag/v0.6.0
