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
83% of the time and is among the top two 98% of the time. Neighbours such as Rock and Nit are
genuinely similar.

## Releasing

Releases are built by [dist](https://axodotdev.github.io/cargo-dist/) (formerly cargo-dist)
(`dist-workspace.toml`, `.github/workflows/release.yml`). Raise `version` in
`[workspace.package]` of `Cargo.toml`, move the `Unreleased` entries in
`CHANGELOG.md` under a heading for the new version (dist uses that section as the release
notes), commit, and push a tag with that version:
`git tag v0.6.2 && git push origin v0.6.2`. GitHub Actions then builds every archive and
installer and publishes the release. `dist plan` shows what a release will contain.

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

## Network protocol versions

Every holdem speaks a range of network protocol versions. When you connect, the two programs
agree on the newest version both speak. If there is none, you are told in plain words whether
you or the host needs to update, instead of the game failing in odd ways. The join screen marks
tables found on your network that need another version. Within a protocol version, messages
tolerate missing and extra fields, and a message of an unknown kind is skipped or answered with
a notice rather than ending the connection. Versions 0.4 to 0.6 speak protocol 3; holdem 0.3 spoke
protocol 2 and is told to update.
