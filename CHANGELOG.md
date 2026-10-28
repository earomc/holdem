# Changelog

All notable changes to holdem. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- One server, several tables: `holdem serve` runs every `[[server.tables]]` entry of
  `holdem.toml` on the one port, each with its own name and, if you like, seats, speed,
  starting chips, small blind and free seats. Players see each table on their join screen, or
  choose from the server's list after typing its address. The log names the table.

### Changed
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

[Unreleased]: https://github.com/earomc/holdem/compare/v0.7.0...HEAD
[0.7.0]: https://github.com/earomc/holdem/releases/tag/v0.7.0
[0.6.1]: https://github.com/earomc/holdem/releases/tag/v0.6.1
[0.6.0]: https://github.com/earomc/holdem/releases/tag/v0.6.0
