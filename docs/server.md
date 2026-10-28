# Running a server

How to run a holdem table that anyone can join, on a computer that stays on. Back to the
[README](../README.md).

## Installing

On a server, install the `holdem-server` release: the same program as `holdem`, without sound
and without any audio libraries. On Linux there are also static `musl` builds that run on any
distribution.

```
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/earomc/holdem/releases/latest/download/holdem-server-installer.sh | sh
```

The installer puts `holdem` in `~/.cargo/bin`. You can also download an archive from the
[releases page](https://github.com/earomc/holdem/releases) and put the program anywhere.
`holdem --version` says `(without sound)` for this build. To check a downloaded archive, run
`gh attestation verify <archive> --repo earomc/holdem` (with the [GitHub CLI](https://cli.github.com)).

## Starting the table

```
holdem serve                         # with the settings of holdem.toml, or the defaults
holdem serve --port 9000 --name "Friday night" --speed fast
```

On start-up it prints every setting it uses and the file they came from. The table has no
player of its own; people join from their *Join a network game* screen. After each game the
table goes back to its lobby.

The tables are the `[[tables]]` entries of `holdem.toml`, and the port is `network.port` (see
[Configuration](configuration.md)). Without a settings file it runs one table with the
defaults. For a server, write a settings file at a fixed place and
point holdem at it:

```
holdem config write ~/holdem.toml    # every setting, explained
holdem serve --config ~/holdem.toml
```

## Several tables

One `holdem serve` runs every table of `holdem.toml` on the one port: one `[[tables]]` entry
per table. Each needs a name (different for every table); everything left out keeps its
default, and `[tables.rules]` right after an entry sets that table's rules:

```toml
[[tables]]
name = "Low stakes"
[tables.rules]
small_blind = 5

[[tables]]
name = "Turbo"
speed = "Fast"
seats = 6

[[tables]]
name = "Heads-up"
seats = 2
```

Players on your network see every table on their join screen. Someone who types the server's
address gets the list of tables and picks one. Players with holdem 0.7 or older cannot choose:
they are seated at the first table. On start-up, `holdem serve` lists the tables, and each line
of its log names the table.

## Network

- Players connect over **TCP** to the port (7777 by default, `network.port`).
- On the same network, the join screen finds the table through **UDP** on the same port. This
  is only a convenience; joining by address works without it.
- For players on the internet, open the TCP port in the firewall
  (`sudo ufw allow 7777/tcp` with ufw) and, at home, forward it to the computer in your router.

## What it prints

While it runs, `holdem serve` prints one line per event, with the time in UTC:

```
18:02:11  Ann joined (1 person connected)
18:02:40  Ben joined (2 people connected)
18:03:05  Game started: 2 people and 4 bots
18:41:52  Ben left (1 person connected)
18:41:58  Ben is back
19:20:33  Game over: Ann wins
```

A program with an incompatible version is turned away with a line saying why.

## Running it as a service (Linux, systemd)

So that the table starts with the computer and comes back after a crash, create
`/etc/systemd/system/holdem.service` (adjust the user and the paths):

```ini
[Unit]
Description=holdem poker table
After=network-online.target
Wants=network-online.target

[Service]
User=holdem
ExecStart=/home/holdem/.cargo/bin/holdem serve --config /home/holdem/holdem.toml
Restart=on-failure

[Install]
WantedBy=multi-user.target
```

Then:

```
sudo systemctl daemon-reload
sudo systemctl enable --now holdem
journalctl -u holdem -f              # the table's log
```

## Updating

Run the installer again (or replace the program), then restart the table
(`sudo systemctl restart holdem`). Players with an older or newer holdem are told in plain
words whether they or the table need to update; see
[network protocol versions](../CONTRIBUTING.md#network-protocol-versions).
