# Contributing to holdem

How to build, test and release holdem, and how the code is organized. For playing, see the
[README](README.md).

## Building

```
cargo run --release                         # play
cargo run --release -- serve                # a table anyone can join, without playing yourself
cargo run --release -- config write        # write holdem.toml with every setting explained
cargo build --release -p holdem-server      # holdem without sound and audio dependencies
```

Requirements:
- Rust 1.88 or newer.
- On Linux, the sound feature needs the ALSA development files:
  `sudo apt install libasound2-dev pkg-config` (Debian/Ubuntu) or
  `sudo dnf install alsa-lib-devel` (Fedora). macOS and Windows need nothing extra.

The workspace has two packages that build the same program, called `holdem` in both:
- `holdem` (the root package): with the default `sound` feature.
- `holdem-server` (`server/`): without it, so it needs no audio libraries. The same as
  `holdem` with `--no-default-features`.

Build one at a time with `-p`. A `--workspace` build writes one `holdem` over the other, and
Cargo would also give `holdem-server` the sound feature.

## Tests

```
cargo test --workspace                      # rules, server, bots, interface and sound tests
cargo test --release -- --ignored           # also check all 133,784,560 seven-card hands (slow)
```

The reference values for each style are measured, not guessed. `cargo run --release --example
calibrate` lets the bots play about 110,000 hands and prints them. A test checks that every
style is recognized from its own play: on samples of 200 hands, the right style comes first
83% of the time and is among the top two 98% of the time. Neighbors such as Rock and Nit are
genuinely similar.

## Before you send a change

CI checks every push and pull request; locally, the same checks are:

```
cargo fmt --check
cargo clippy -p holdem --all-targets -- -D warnings
cargo clippy -p holdem-server --all-targets -- -D warnings
cargo test -p holdem
cargo test -p holdem-server
```

- Anything people will notice gets a line under *Unreleased* in `CHANGELOG.md`.
- A change to the glossary or the bot styles: run
  `HOLDEM_UPDATE_DOCS=1 cargo test -p holdem docs` to update `docs/poker.md`.
- A change to how a screen looks: redraw the screenshots (below).

## Screenshots

The pictures in `docs/screenshots/` are drawn by the game itself, so they always show the
current screens. After changing what a screen looks like, redraw them:

```
cargo test -p holdem screenshots -- --ignored
```

## Releasing

Releases are built by [dist](https://axodotdev.github.io/cargo-dist/) (formerly cargo-dist)
(`dist-workspace.toml`, `.github/workflows/release.yml`). Raise `version` in
`[workspace.package]` of `Cargo.toml`, move the `Unreleased` entries in
`CHANGELOG.md` under a heading for the new version (dist uses that section as the release
notes), commit, and push a tag with that version:
`git tag v0.9.1 && git push origin v0.9.1`. GitHub Actions then builds every archive and
installer and publishes the release. `dist plan` shows what a release will contain.

## How it is built

Every game runs on the same small **table server**, whether you play alone, host, or join.
Your screen is a client of it: through memory when the table runs in your own program, and
through TCP (one JSON message per line) for everyone else. One code path drives the game for
all three cases. The server is authoritative, so players cannot cheat by changing their
client.

The library (`src/lib.rs`) is the engine and the server; the program (`src/main.rs`) is the
terminal front end. A module that grew large is a folder whose `mod.rs` says how it is split.

| Module | Purpose |
| --- | --- |
| `card`, `eval` | cards, deck, shuffling; the 5-to-7-card hand evaluator |
| `game/` | the rules as a state machine: `rules`, `pots`, and `Game`'s methods by stage (`seats`, `hand`, `betting`, `showdown`, `queries`) |
| `bot/` | bot styles (each with its description and measured statistics); `decide`: equity simulation and decisions |
| `stats` | playing statistics and the style estimate |
| `table/` | the table server: `connections`, `messages`, `flow` (games, seats, bots, time limits), `views` (what each player may see) |
| `protocol` | the messages between table and clients |
| `net/` | running a table on a thread (`server`), connecting and finding tables (`client`) |
| `config/` | `holdem.toml`: the settings and defaults, `load`, `render` (with `docs`), `save` |
| `player/` | you beyond one game, next to `holdem.toml`: `history` (your statistics over all games, `stats.toml`) |
| `install` | the release installer's receipts, and what `holdem uninstall` removes |
| `main`, `cli/` | the program: `main` and the play loop; the command line in `cli`, one file per command (`serve`, `config`, `uninstall`) |
| `app/` | screen state and input: `input`, `setup`, `table`, `settings`, `lifecycle` |
| `session` | the client's side of a game |
| `keys` | every key binding in one table: input, the key reminders on screen and the Keys help page all come from it |
| `ui/` | drawing: one module per screen, `popups` for the windows on top, shared `style`, `text`, `cards`, `log` |
| `help/` | the help pages, one module per page; `docs` keeps `docs/poker.md` in step with them |
| `sound` | sound synthesis and playback |
| `tests/` | rule scenarios, a side-pot oracle, random-play fuzzing across all options, style recognition |
| `examples/calibrate.rs`, `examples/strength.rs` | the measurements behind the bot tuning and the style estimate |

## Network protocol versions

Every holdem speaks a range of network protocol versions. When you connect, the two programs
agree on the newest version both speak. If there is none, you are told in plain words whether
you or the host needs to update, instead of the game failing in odd ways. The join screen marks
tables found on your network that need another version. Within a protocol version, messages
tolerate missing and extra fields, and a message of an unknown kind is skipped or answered with
a notice rather than ending the connection. Versions 0.4 to 0.7 speak protocol 3; from 0.8 on,
holdem speaks protocols 3 and 4. Protocol 4 added several tables per server: `Hello` may name
a table, and a server with several answers one that names none with the list of tables.
Programs speaking protocol 3 are seated at the server's first table. holdem 0.3 spoke protocol
2 and is told to update.
