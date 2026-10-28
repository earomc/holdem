//! What every setting means: the comments of a written settings file.

/// What every setting means, by its dotted path: the single source of the
/// comments in the settings file. Boolean settings say what `true` and `false`
/// do, each on its own line. (Tests check that every setting is described and
/// that nothing described is missing from the settings.)
pub(super) const DOCS: &[(&str, &str)] = &[
    ("client", "Only your own screen and keys."),
    (
        "client.name",
        "Your name at the table. Empty means your computer's login name.",
    ),
    (
        "client.speed",
        "The game speed to start with: one of the names in [[game.speeds]].",
    ),
    (
        "client.win_chance_hint",
        "The win-chance hint, where the table allows it.\n\
         true: shown while you are in a hand\n\
         false: hidden (switch it on in the settings, key S)",
    ),
    (
        "client.hint_samples",
        "Simulated deals behind the win-chance hint.",
    ),
    (
        "client.message_ms",
        "How long status messages stay on screen, in milliseconds.",
    ),
    (
        "client.pot_flash_ms",
        "How long the pot flashes when it grows, in milliseconds.",
    ),
    (
        "client.log_lines",
        "Hand log lines kept for the log window.",
    ),
    (
        "client.style_min_hands",
        "Counted hands before your playing style is estimated.",
    ),
    ("client.sound", "Sound effects."),
    ("client.sound.volume", "0 (off) to 100."),
    (
        "client.sound.volume_step",
        "How much one press changes the volume in the settings.",
    ),
    (
        "client.sound.gap_ms",
        "Pause between sounds that happen together, in milliseconds.",
    ),
    (
        "client.setup",
        "The values offered on the setup screen. 0 means \"never\", \"no limit\" or \"off\".",
    ),
    ("client.setup.starting_chips", "Starting stacks."),
    (
        "client.setup.small_blinds",
        "Starting small blinds (the big blind is twice as much).",
    ),
    ("client.setup.hands_per_level", "Hands per blind level."),
    ("client.setup.hand_limits", "Hand limits."),
    (
        "client.setup.rebuy_levels",
        "How many blind levels rebuys stay open.",
    ),
    ("client.join", "Joining other people's tables."),
    (
        "client.join.connect_seconds",
        "Give up connecting to a table after this many seconds.",
    ),
    (
        "client.join.discovery_seconds",
        "How often the join screen looks for tables on your network, in seconds.",
    ),
    (
        "client.join.discovery_wait_ms",
        "How long it waits for answers, in milliseconds.",
    ),
    (
        "game",
        "How tables run by this program play: solo games, games you host, and\n\
         `holdem serve`. At someone else's table, that table's settings apply.",
    ),
    ("game.seats", "Seats at the table, 2 to 9."),
    (
        "game.fill_with_bots",
        "Seats that no person takes.\n\
         true: given to bots\n\
         false: left empty (in a network game only people play)",
    ),
    (
        "game.allow_win_chance",
        "Whether players may use the win-chance hint.\n\
         true: allowed\n\
         false: not allowed; it cannot be switched on",
    ),
    (
        "game.runout_pace_percent",
        "Pause between the streets of an all-in run-out, in percent of the game speed.",
    ),
    (
        "game.style_min_stack_bb",
        "Hands started with fewer big blinds than this do not count for the style\n\
         estimate (short stacks must play push-or-fold whatever their style).",
    ),
    (
        "game.speeds",
        "Game speeds, slowest first: as many as you like, each with a name and the\n\
         milliseconds between bot moves (20 to 10000).",
    ),
    ("game.bots", "The computer players."),
    (
        "game.bots.names",
        "Names given to the bots; numbers are added if there are too few.",
    ),
    (
        "game.bots.thinking_samples",
        "Simulated deals per bot decision: more plays slightly better but slower.",
    ),
    ("game.rules", "The rules."),
    (
        "game.rules.starting_chips",
        "Every player starts with this many chips.",
    ),
    (
        "game.rules.small_blind",
        "The small blind at the start; the big blind is twice as much.",
    ),
    (
        "game.rules.hands_per_level",
        "Hands per blind level; 0 means the blinds never go up.",
    ),
    (
        "game.rules.blind_schedule",
        "The small blind at each level, in percent of the starting small blind.",
    ),
    (
        "game.rules.blind_growth_percent",
        "After the schedule's last level, each level is this many percent of the one before.",
    ),
    (
        "game.rules.hand_limit",
        "The game ends after this many hands; 0 means no limit.",
    ),
    (
        "game.rules.antes",
        "Antes.\n\
         true: every player pays an ante each hand\n\
         false: no antes",
    ),
    (
        "game.rules.ante_divisor",
        "The ante is the big blind divided by this (at least 1 chip).",
    ),
    (
        "game.rules.straddle",
        "Straddle.\n\
         true: the player left of the big blind posts a straddle and acts last before the flop\n\
         false: no straddle",
    ),
    (
        "game.rules.straddle_multiplier",
        "The straddle, in big blinds (2 or more).",
    ),
    (
        "game.rules.straddle_min_players",
        "Fewest players at the table for a straddle (3 to 9).",
    ),
    (
        "game.rules.rebuy_levels",
        "Busted players may buy back in while the blind level is at most this; 0 means no rebuys.",
    ),
    (
        "game.rules.rebuy_chips",
        "Chips a rebuy gives; 0 means the starting stack.",
    ),
    (
        "game.rules.dead_button",
        "Button rule.\n\
         true: dead button: the big blind moves exactly one player each hand, and the \
         small blind and button follow it, even onto empty seats\n\
         false: moving button: the button simply moves to the next player",
    ),
    (
        "game.rules.muck_losers",
        "Showdown.\n\
         true: players who cannot win may throw their cards away unseen\n\
         false: everyone still in shows their cards",
    ),
    ("game.fluctuation", "Bots coming and going between hands."),
    (
        "game.fluctuation.enabled",
        "Players come and go.\n\
         true: bots sometimes leave and new bots sit down in empty seats\n\
         false: the bots stay until they are knocked out",
    ),
    (
        "game.fluctuation.leave_chance",
        "Chance per hand that a given bot leaves (0 to 1).",
    ),
    (
        "game.fluctuation.join_chance",
        "Chance per hand that an empty seat gets a new bot (0 to 1).",
    ),
    (
        "server",
        "Network play: hosting from the menu, and `holdem serve`.",
    ),
    (
        "server.name",
        "The table's name, shown to people looking for a game.",
    ),
    (
        "server.port",
        "Port to listen on. The join screen also assumes and searches this port.",
    ),
    (
        "server.speed",
        "The game speed of `holdem serve`: one of the names in [[game.speeds]].\n\
         (When you host from the menu, your own speed setting is used.)",
    ),
    (
        "server.turn_seconds",
        "Seconds a person has to act when several people play; 0 means no limit.",
    ),
    (
        "server.next_hand_seconds",
        "Seconds after a hand before the next one starts by itself; 0 means wait for everyone.",
    ),
    (
        "server.away_seconds",
        "Seconds before a disconnected player's turn is folded for them.",
    ),
    (
        "server.tables",
        "Several tables for `holdem serve`, all on the one port: players choose one when\n\
         they join. One [[server.tables]] entry per table, each with a name (different for\n\
         every table) and, if you like, its own seats, speed, fill_with_bots,\n\
         starting_chips and small_blind; everything else comes from [game] and [server].\n\
         Without entries, `holdem serve` runs one table called server.name. For example:\n\
         \n\
         [[server.tables]]\n\
         name = \"Low stakes\"\n\
         small_blind = 5\n\
         \n\
         [[server.tables]]\n\
         name = \"Turbo\"\n\
         speed = \"Fast\"",
    ),
];

pub(super) fn doc(path: &str) -> &'static str {
    DOCS.iter()
        .find(|(p, _)| *p == path)
        .map(|(_, d)| *d)
        .unwrap_or("")
}
