# Changelog

All notable changes to holdem. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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

[Unreleased]: https://github.com/earomc/holdem/compare/v0.6.1...HEAD
[0.6.1]: https://github.com/earomc/holdem/releases/tag/v0.6.1
[0.6.0]: https://github.com/earomc/holdem/releases/tag/v0.6.0
