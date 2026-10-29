//! Every key binding, in one table. Input handling (`app`) asks this table
//! what a key means, the key reminders on screen are built from it, and so is
//! the Keys page of the help: a binding changed here changes everywhere.

use ratatui::crossterm::event::KeyCode;

/// Where a key is pressed. Each place has its own section of bindings.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Menu,
    Setup,
    Join,
    /// Choosing one of the tables at a server with several.
    Choose,
    Lobby,
    /// At the table, when it is your turn.
    Turn,
    /// At the table, between hands.
    HandOver,
    /// At the table, at any time.
    Table,
    Raise,
    Over,
    HelpWindow,
    /// The game info window: the table log, your hands and your style.
    InfoWindow,
    SettingsWindow,
    LeaveWindow,
    /// Keys that work everywhere; handled before anything else.
    Everywhere,
}

/// What a key asks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmd {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    First,
    Last,
    Select,
    Back,
    Erase,
    Save,
    Close,
    Help,
    Log,
    Profile,
    Settings,
    Leave,
    Quit,
    Ready,
    Rebuy,
    Fold,
    Call,
    Raise,
    AllIn,
    MinRaise,
    HalfPot,
    Pot,
    MaxRaise,
    MainMenu,
    Stay,
    /// The next colour for your symbol.
    Color,
}

/// One line of the Keys help page: one or more bindings and what they do.
pub struct Row {
    /// Each binding: the keys (any of them works) and the command.
    pub keys: &'static [(&'static [KeyCode], Cmd)],
    /// Shown instead of the key names, for keys this table cannot list
    /// (typing, Ctrl-C).
    pub label: Option<&'static str>,
    /// What the keys do, for the help.
    pub what: &'static str,
    /// A word or two for the key reminder on screen; empty if the screen
    /// words it itself (or does not show it).
    pub hint: &'static str,
}

pub struct Section {
    pub place: Place,
    pub title: &'static str,
    pub rows: &'static [Row],
}

use KeyCode::{
    Backspace, Char, Down, End, Enter, Esc, Home, Left, PageDown, PageUp, Right, Tab, Up,
};

const HELP_KEYS: &[KeyCode] = &[Char('?'), Char('h'), KeyCode::F(1)];

const fn row(
    keys: &'static [(&'static [KeyCode], Cmd)],
    what: &'static str,
    hint: &'static str,
) -> Row {
    Row {
        keys,
        label: None,
        what,
        hint,
    }
}

const fn note(
    label: &'static str,
    keys: &'static [(&'static [KeyCode], Cmd)],
    what: &'static str,
) -> Row {
    Row {
        keys,
        label: Some(label),
        what,
        hint: "",
    }
}

const UP_DOWN: &[(&[KeyCode], Cmd)] = &[(&[Up], Cmd::Up), (&[Down], Cmd::Down)];
const LEFT_RIGHT: &[(&[KeyCode], Cmd)] = &[(&[Left], Cmd::Left), (&[Right], Cmd::Right)];
const OPEN_HELP: Row = row(&[(HELP_KEYS, Cmd::Help)], "open this help", "help");
const LEAVE: Row = row(
    &[(&[Char('q'), Esc], Cmd::Leave)],
    "leave: back to the main menu, quit, or keep playing",
    "leave",
);

/// The sections in the order the help lists them.
pub const SECTIONS: &[Section] = &[
    Section {
        place: Place::Menu,
        title: "MAIN MENU",
        rows: &[
            row(UP_DOWN, "choose", "choose"),
            row(&[(&[Enter], Cmd::Select)], "select", "select"),
            OPEN_HELP,
            row(
                &[(&[Char('q'), Esc], Cmd::Quit)],
                "quit the application",
                "quit",
            ),
        ],
    },
    Section {
        place: Place::Setup,
        title: "GAME SETUP",
        rows: &[
            row(UP_DOWN, "choose a setting", "choose"),
            row(
                LEFT_RIGHT,
                "change it (the line below the list explains it)",
                "change",
            ),
            note(
                "typing, Backspace",
                &[(&[Backspace], Cmd::Erase)],
                "edit your name (on that row, Q, ? and H are letters: F1 opens the help)",
            ),
            row(
                &[(&[Char('c')], Cmd::Color)],
                "on Your symbol: the next colour (← → change the shape)",
                "",
            ),
            row(
                &[(&[Enter], Cmd::Select)],
                "deal (or open your table to others when hosting)",
                "",
            ),
            row(
                &[(&[Char('w')], Cmd::Save)],
                "save your name, settings and these table options as your defaults",
                "save",
            ),
            row(
                &[(&[Esc, Char('q')], Cmd::Back)],
                "back to the main menu",
                "back",
            ),
            OPEN_HELP,
        ],
    },
    Section {
        place: Place::Join,
        title: "JOIN SCREEN",
        rows: &[
            row(
                &[(&[Up], Cmd::Up), (&[Down, Tab], Cmd::Down)],
                "move between your name, the address and the tables found",
                "choose",
            ),
            note(
                "typing, Backspace",
                &[(&[Backspace], Cmd::Erase)],
                "edit your name or the address",
            ),
            note(
                "← → and C",
                &[],
                "on Your symbol: change its shape and its colour",
            ),
            row(
                &[(&[Enter], Cmd::Select)],
                "join the highlighted table, or the address you typed",
                "join",
            ),
            row(
                &[(&[Esc], Cmd::Back)],
                "back to the main menu (or cancel a connection attempt)",
                "back",
            ),
            row(
                &[(&[KeyCode::F(1)], Cmd::Help)],
                "open this help (? and H are letters you can type here)",
                "help",
            ),
        ],
    },
    Section {
        place: Place::Choose,
        title: "CHOOSING A TABLE",
        rows: &[
            row(UP_DOWN, "move between the server's tables", "choose"),
            row(
                &[(&[Enter], Cmd::Select)],
                "join the highlighted table",
                "join",
            ),
            row(
                &[(&[Esc, Char('q')], Cmd::Back)],
                "hang up and go back to the join screen",
                "back",
            ),
            OPEN_HELP,
        ],
    },
    Section {
        place: Place::Lobby,
        title: "LOBBY",
        rows: &[
            row(
                &[(&[Enter], Cmd::Ready)],
                "ready / not ready: the game starts when everyone is ready",
                "ready / not ready",
            ),
            row(&[(&[Char('s')], Cmd::Settings)], "settings", "settings"),
            OPEN_HELP,
            LEAVE,
        ],
    },
    Section {
        place: Place::Turn,
        title: "AT THE TABLE, WHEN IT IS YOUR TURN",
        rows: &[
            row(
                &[(&[Char('f')], Cmd::Fold)],
                "fold (not offered while checking is free)",
                "",
            ),
            row(
                &[(&[Char('c')], Cmd::Call)],
                "check, or call the current bet",
                "",
            ),
            row(
                &[(&[Char('r'), Char('b')], Cmd::Raise)],
                "open the raise box to bet or raise",
                "",
            ),
            row(
                &[(&[Char('a')], Cmd::AllIn)],
                "go all-in: press it a second time to confirm, any other key cancels",
                "",
            ),
        ],
    },
    Section {
        place: Place::HandOver,
        title: "AT THE TABLE, BETWEEN HANDS",
        rows: &[
            row(
                &[(&[Enter, Char('n')], Cmd::Ready)],
                "deal the next hand (with others: say you are ready); \
                 while a rebuy is offered, give up instead",
                "",
            ),
            row(
                &[(&[Char('y')], Cmd::Rebuy)],
                "buy back in, when a rebuy is offered",
                "",
            ),
        ],
    },
    Section {
        place: Place::Table,
        title: "AT THE TABLE, AT ANY TIME",
        rows: &[
            OPEN_HELP,
            row(
                &[(&[Char('l')], Cmd::Log)],
                "game info: the table log, your hands and your style",
                "game info",
            ),
            row(
                &[(&[Char('p')], Cmd::Profile)],
                "your playing style and the bot style it is closest to",
                "your style",
            ),
            row(
                &[(&[Char('s')], Cmd::Settings)],
                "settings: game speed, sound volume, win-chance hint (if the table allows it)",
                "settings",
            ),
            LEAVE,
        ],
    },
    Section {
        place: Place::Raise,
        title: "RAISE BOX",
        rows: &[
            note(
                "0-9, Backspace",
                &[(&[Backspace], Cmd::Erase)],
                "type the total you want to have bet in this round",
            ),
            row(&[(&[Char('m')], Cmd::MinRaise)], "the minimum raise", "min"),
            row(
                &[(&[Char('h')], Cmd::HalfPot)],
                "raise by half the pot",
                "half pot",
            ),
            row(&[(&[Char('p')], Cmd::Pot)], "raise by the whole pot", "pot"),
            row(
                &[(&[Char('a')], Cmd::MaxRaise)],
                "everything you have (all-in); Enter still confirms",
                "all-in",
            ),
            row(UP_DOWN, "one big blind more / less", "± big blind"),
            row(
                &[(&[Enter], Cmd::Select)],
                "confirm the bet or raise",
                "confirm",
            ),
            row(
                &[(&[Esc], Cmd::Back)],
                "cancel and go back to fold / call / raise",
                "cancel",
            ),
        ],
    },
    Section {
        place: Place::Over,
        title: "FINAL STANDINGS",
        rows: &[
            row(
                &[(&[Enter], Cmd::Select)],
                "main menu (solo) or play again (network game: back to the lobby)",
                "",
            ),
            row(
                &[(&[Char('p')], Cmd::Profile)],
                "your playing style",
                "your style",
            ),
            row(
                &[(&[Char('l')], Cmd::Log)],
                "game info: the table log, your hands and your style",
                "game info",
            ),
            OPEN_HELP,
            LEAVE,
        ],
    },
    Section {
        place: Place::HelpWindow,
        title: "HELP WINDOW",
        rows: &[
            row(LEFT_RIGHT, "turn the page", "page"),
            row(UP_DOWN, "scroll", "scroll"),
            row(
                &[(&[PageUp], Cmd::PageUp), (&[PageDown], Cmd::PageDown)],
                "scroll faster",
                "",
            ),
            row(
                &[(&[Esc, Char('?'), Char('h'), KeyCode::F(1)], Cmd::Close)],
                "close",
                "close",
            ),
        ],
    },
    Section {
        place: Place::InfoWindow,
        title: "GAME INFO WINDOW",
        rows: &[
            row(
                &[(&[Left], Cmd::Left), (&[Right, Tab], Cmd::Right)],
                "switch between the table log, your hands and your style",
                "tab",
            ),
            row(UP_DOWN, "scroll", "scroll"),
            row(
                &[(&[PageUp], Cmd::PageUp), (&[PageDown], Cmd::PageDown)],
                "scroll faster",
                "",
            ),
            row(
                &[(&[Home], Cmd::First), (&[End], Cmd::Last)],
                "jump to the oldest / newest line",
                "",
            ),
            row(&[(&[Char('p')], Cmd::Profile)], "your style", ""),
            row(&[(&[Esc, Enter, Char('l')], Cmd::Close)], "close", "close"),
        ],
    },
    Section {
        place: Place::SettingsWindow,
        title: "SETTINGS WINDOW",
        rows: &[
            row(UP_DOWN, "choose", "choose"),
            row(LEFT_RIGHT, "change", "change"),
            row(
                &[(&[Char('w')], Cmd::Save)],
                "save your settings as your defaults",
                "save",
            ),
            row(&[(&[Esc, Enter, Char('s')], Cmd::Close)], "close", "close"),
        ],
    },
    Section {
        place: Place::LeaveWindow,
        title: "LEAVE WINDOW",
        rows: &[
            row(UP_DOWN, "choose", "choose"),
            row(&[(&[Enter], Cmd::Select)], "confirm", "confirm"),
            row(
                &[(&[Char('m')], Cmd::MainMenu)],
                "back to the main menu",
                "menu",
            ),
            row(&[(&[Char('q')], Cmd::Quit)], "quit the application", "quit"),
            row(&[(&[Esc], Cmd::Stay)], "keep playing", "stay"),
        ],
    },
    Section {
        place: Place::Everywhere,
        title: "EVERYWHERE",
        rows: &[note(
            "Ctrl-C",
            &[],
            "quit the application immediately, from any screen",
        )],
    },
];

fn section(place: Place) -> &'static Section {
    SECTIONS
        .iter()
        .find(|s| s.place == place)
        .expect("every place has a section")
}

/// Letters match in either case.
fn same(bound: KeyCode, pressed: KeyCode) -> bool {
    match (bound, pressed) {
        (Char(a), Char(b)) => a.eq_ignore_ascii_case(&b),
        (a, b) => a == b,
    }
}

/// What `pressed` means in `place`, if anything.
pub fn command(place: Place, pressed: KeyCode) -> Option<Cmd> {
    section(place)
        .rows
        .iter()
        .flat_map(|r| r.keys.iter())
        .find(|(keys, _)| keys.iter().any(|&k| same(k, pressed)))
        .map(|&(_, cmd)| cmd)
}

/// A key's name as the screen shows it.
pub fn name(key: KeyCode) -> String {
    match key {
        Char(c) => c.to_ascii_uppercase().to_string(),
        Up => "↑".to_string(),
        Down => "↓".to_string(),
        Left => "←".to_string(),
        Right => "→".to_string(),
        PageUp => "PgUp".to_string(),
        PageDown => "PgDn".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        other => other.to_string(),
    }
}

fn is_arrow(key: KeyCode) -> bool {
    matches!(key, Up | Down | Left | Right)
}

/// The keys of a help row, e.g. "? or H or F1", "↑ ↓ or Tab", "Home / End".
pub fn label(row: &Row) -> String {
    if let Some(label) = row.label {
        return label.to_string();
    }
    let mut out = String::new();
    let mut previous: Option<KeyCode> = None;
    for &(keys, _) in row.keys {
        if let Some(p) = previous {
            // Arrows read as a pair ("↑ ↓"); other bindings are set apart.
            out.push_str(if is_arrow(p) && is_arrow(keys[0]) {
                " "
            } else {
                " / "
            });
        }
        let names: Vec<String> = keys.iter().map(|&k| name(k)).collect();
        out.push_str(&names.join(" or "));
        previous = keys.last().copied();
    }
    out
}

/// The key for `cmd` in `place` as the on-screen reminder shows it: the first
/// key of each binding of its row, e.g. "Enter", "↑↓".
pub fn key(place: Place, cmd: Cmd) -> String {
    let row = section(place)
        .rows
        .iter()
        .find(|r| r.keys.iter().any(|&(_, c)| c == cmd))
        .unwrap_or_else(|| panic!("{cmd:?} has no key in {place:?}"));
    short(row)
}

fn short(row: &Row) -> String {
    // Arrows run together ("↑↓"); other keys are spaced ("PgUp PgDn").
    let mut out = String::new();
    let mut previous: Option<KeyCode> = None;
    for &(keys, _) in row.keys {
        let k = keys[0];
        if let Some(p) = previous
            && !(is_arrow(p) && is_arrow(k))
        {
            out.push(' ');
        }
        out.push_str(&name(k));
        previous = Some(k);
    }
    out
}

/// The key reminder of a place: every row that has a hint, as (key, hint).
pub fn reminder(place: Place) -> Vec<(String, &'static str)> {
    reminder_where(place, |_| true)
}

/// Like [`reminder`], but only rows whose first command passes `keep`.
pub fn reminder_where(place: Place, keep: impl Fn(Cmd) -> bool) -> Vec<(String, &'static str)> {
    section(place)
        .rows
        .iter()
        .filter(|r| !r.hint.is_empty() && r.keys.first().is_some_and(|&(_, c)| keep(c)))
        .map(|r| (short(r), r.hint))
        .collect()
}

/// The reminder as plain text, e.g. "? help   L log".
pub fn reminder_text(place: Place) -> String {
    reminder(place)
        .iter()
        .map(|(k, h)| format!("{k} {h}"))
        .collect::<Vec<_>>()
        .join("   ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Places whose keys are all active at the same time.
    const TOGETHER: &[&[Place]] = &[
        &[Place::Menu],
        &[Place::Setup],
        &[Place::Join],
        &[Place::Choose],
        &[Place::Lobby],
        &[Place::Table, Place::Turn],
        &[Place::Table, Place::HandOver],
        &[Place::Raise],
        &[Place::Over],
        &[Place::HelpWindow],
        &[Place::InfoWindow],
        &[Place::SettingsWindow],
        &[Place::LeaveWindow],
    ];

    #[test]
    fn no_key_means_two_things_at_once() {
        for places in TOGETHER {
            let mut seen: Vec<(KeyCode, Cmd)> = Vec::new();
            for &p in *places {
                for r in section(p).rows {
                    for &(keys, cmd) in r.keys {
                        for &k in keys {
                            if let Some((_, other)) = seen.iter().find(|(s, _)| same(*s, k)) {
                                panic!("{k:?} means {other:?} and {cmd:?} in {places:?}");
                            }
                            seen.push((k, cmd));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_place_has_one_section() {
        for s in SECTIONS {
            assert_eq!(SECTIONS.iter().filter(|o| o.place == s.place).count(), 1);
        }
    }

    #[test]
    fn letters_match_in_either_case() {
        assert_eq!(command(Place::Table, Char('L')), Some(Cmd::Log));
        assert_eq!(command(Place::Table, Char('l')), Some(Cmd::Log));
        assert_eq!(command(Place::Raise, Char('P')), Some(Cmd::Pot));
        assert_eq!(command(Place::Table, Char('x')), None);
    }

    #[test]
    fn labels_and_reminders() {
        let s = section(Place::Menu);
        assert_eq!(label(&s.rows[0]), "↑ ↓");
        assert_eq!(label(&s.rows[2]), "? or H or F1");
        assert_eq!(key(Place::Menu, Cmd::Up), "↑↓");
        assert_eq!(key(Place::InfoWindow, Cmd::First), "Home End");
        assert_eq!(key(Place::Join, Cmd::Down), "↑↓");
        assert_eq!(key(Place::InfoWindow, Cmd::Up), "↑↓");
        assert_eq!(label(&section(Place::Join).rows[0]), "↑ ↓ or Tab");
        assert_eq!(label(&section(Place::InfoWindow).rows[3]), "Home / End");
        assert_eq!(
            reminder_text(Place::Table),
            "? help   L game info   P your style   S settings   Q leave"
        );
    }
}
