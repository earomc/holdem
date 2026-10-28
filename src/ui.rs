//! Drawing: the menus, the lobby, the table and the final standings. Windows
//! that open on top (help, log, settings...) are in `popups.rs`.

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use holdem::card::Card;
use holdem::game::{MAX_PLAYERS, MIN_PLAYERS, State, chips_text, ordinal};
use holdem::protocol::{SeatView, TableView};
use holdem::stats;

use crate::app::{App, MenuItem, Screen, SetupRow};
use crate::keys::{self, Cmd, Place};
use crate::popups;
use crate::session::Kind;

// ------------------------------------------------------------------ small helpers

pub fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::new().fg(Color::DarkGray)
}

pub fn yellow() -> Style {
    Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

pub fn cyan() -> Style {
    Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

pub fn green() -> Style {
    Style::new().fg(Color::Green)
}

/// A rectangle of at most `width` x `height` in the middle of `area`.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    )
}

/// Display width of a string in terminal columns.
pub fn text_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Splits off the longest prefix of `text` that fits in `width` columns
/// (always at least one character, so wrapping always makes progress).
fn split_at_width(text: &str, width: usize) -> (String, String) {
    let mut used = 0;
    let mut cut = text.len();
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > width && i > 0 {
            cut = i;
            break;
        }
        used += w;
    }
    (text[..cut].to_string(), text[cut..].to_string())
}

/// Truncates `text` to at most `width` columns, ending in an ellipsis when cut.
pub fn fit(text: &str, width: usize) -> String {
    if text_width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let (head, _) = split_at_width(text, width - 1);
    format!("{head}…")
}

/// Word-wraps `text` into lines of at most `width` columns. Continuation lines
/// start with `indent` spaces. Words longer than a line are broken.
pub fn wrap_text(text: &str, width: usize, indent: usize) -> Vec<String> {
    let width = width.max(1);
    let indent = if indent * 2 >= width { 0 } else { indent };
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let limit = if lines.is_empty() {
                width
            } else {
                width - indent
            };
            let ww = text_width(&word);
            let needed = if current.is_empty() {
                ww
            } else {
                current_w + 1 + ww
            };
            if needed <= limit {
                if !current.is_empty() {
                    current.push(' ');
                    current_w += 1;
                }
                current.push_str(&word);
                current_w += ww;
                break;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_w = 0;
                continue;
            }
            // A single word wider than the line: break it.
            let (head, tail) = split_at_width(&word, limit);
            lines.push(head);
            word = tail;
            if word.is_empty() {
                break;
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    let pad = " ".repeat(indent);
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| if i == 0 { l } else { format!("{pad}{l}") })
        .collect()
}

fn card_span(c: Card) -> Span<'static> {
    let fg = if c.suit.is_red() {
        Color::Red
    } else {
        Color::Black
    };
    Span::styled(
        format!("{:>3}{} ", c.rank_symbol(), c.suit.symbol()),
        Style::new()
            .fg(fg)
            .bg(Color::White)
            .add_modifier(Modifier::BOLD),
    )
}

fn back_span() -> Span<'static> {
    Span::styled(" ▒▒▒ ", Style::new().fg(Color::LightBlue).bg(Color::Blue))
}

fn cards_line(cards: &[Card]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, c) in cards.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(card_span(*c));
    }
    Line::from(spans)
}

fn hidden_line() -> Line<'static> {
    Line::from(vec![back_span(), Span::raw(" "), back_span()])
}

fn board_line(board: &[Card]) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for i in 0..5 {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        match board.get(i) {
            Some(c) => spans.push(card_span(*c)),
            None => spans.push(Span::styled("  ·  ", dim())),
        }
    }
    Line::from(spans)
}

fn log_style(text: &str) -> Style {
    if text.starts_with("---") {
        Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if text.contains(" wins ") || text.contains(" split ") {
        Style::new().fg(Color::Green)
    } else if text.contains("eliminated") || text.contains("busted") {
        Style::new().fg(Color::Red)
    } else if text.starts_with("Showdown") || text.contains(" shows ") || text.contains(" mucks") {
        Style::new().fg(Color::Magenta)
    } else if text.starts_with("Flop") || text.starts_with("Turn") || text.starts_with("River") {
        Style::new().fg(Color::Yellow)
    } else if text.contains(" joins ") || text.contains(" leaves ") || text.contains(" rebuys ") {
        Style::new().fg(Color::LightBlue)
    } else {
        Style::new()
    }
}

/// The log wrapped to `width` columns. Returns the `height` lines that end
/// `scroll` lines above the newest one, and the largest useful `scroll`.
pub fn log_view(
    log: &[String],
    height: usize,
    width: usize,
    scroll: usize,
) -> (Vec<Line<'static>>, usize) {
    let mut all: Vec<Line<'static>> = Vec::new();
    for entry in log {
        let style = log_style(entry);
        for piece in wrap_text(entry, width, 2) {
            all.push(Line::from(Span::styled(piece, style)));
        }
    }
    let max = all.len().saturating_sub(height);
    let scroll = scroll.min(max);
    let end = all.len() - scroll;
    let start = end.saturating_sub(height);
    let shown: Vec<Line<'static>> = all.drain(start..end).collect();
    (shown, max)
}

/// Adds blank lines above `lines` so they sit in the middle of a box `area` tall.
fn centre_vertically(mut lines: Vec<Line<'static>>, area: Rect) -> Vec<Line<'static>> {
    let inner = area.height.saturating_sub(2) as usize;
    let pad = inner.saturating_sub(lines.len()) / 2;
    let mut out = vec![Line::from(""); pad];
    out.append(&mut lines);
    out
}

pub fn framed(title: Line<'static>, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(title)
}

pub fn key(k: &str) -> Span<'static> {
    Span::styled(k.to_string(), yellow())
}

/// A key reminder: `lead` entries (key, words) for keys whose words depend on
/// the moment, then the place's reminder from the key table.
pub fn reminder(lead: &[(String, &str)], place: Place) -> Vec<Span<'static>> {
    reminder_spans(lead.iter().cloned().chain(keys::reminder(place)))
}

/// Key reminder spans from (key, words) entries.
pub fn reminder_spans<'a>(entries: impl Iterator<Item = (String, &'a str)>) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw(" ")];
    for (i, (k, words)) in entries.enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(key(&k));
        spans.push(Span::raw(format!(" {words}")));
    }
    spans
}

/// The key that does `cmd` in `place`, as shown on screen.
pub fn key_for(place: Place, cmd: Cmd) -> String {
    keys::key(place, cmd)
}

// ------------------------------------------------------------------ big digits for the pot

/// Three-line block-character digits.
const DIGITS: [[&str; 3]; 10] = [
    ["█▀█", "█ █", "█▄█"],
    ["▀█ ", " █ ", "▄█▄"],
    ["▀▀█", "█▀▀", "█▄▄"],
    ["▀▀█", " ▀█", "▄▄█"],
    ["█ █", "▀▀█", "  █"],
    ["█▀▀", "▀▀█", "▄▄█"],
    ["█▀▀", "█▀█", "█▄█"],
    ["▀▀█", "  █", "  █"],
    ["█▀█", "█▀█", "█▄█"],
    ["█▀█", "▀▀█", "▄▄█"],
];

/// `text` (digits and commas) drawn three lines tall.
pub fn big_number(text: &str) -> [String; 3] {
    let mut rows = [String::new(), String::new(), String::new()];
    for (i, c) in text.chars().enumerate() {
        if i > 0 {
            for r in &mut rows {
                r.push(' ');
            }
        }
        match c.to_digit(10) {
            Some(d) => {
                for (r, part) in rows.iter_mut().zip(DIGITS[d as usize]) {
                    r.push_str(part);
                }
            }
            None => {
                rows[0].push(' ');
                rows[1].push(' ');
                rows[2].push('▄');
            }
        }
    }
    rows
}

// ------------------------------------------------------------------ entry point

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match app.screen {
        Screen::Menu => draw_menu(frame, app, area),
        Screen::Setup => draw_setup(frame, app, area),
        Screen::Join => draw_join(frame, app, area),
        Screen::Lobby => draw_lobby(frame, app, area),
        Screen::Table => draw_table(frame, app, area),
        Screen::Over => draw_over(frame, app, area),
    }
    if app.show_settings {
        popups::settings(frame, app, area);
    }
    if app.show_profile {
        popups::profile(frame, app, area);
    }
    if app.show_log {
        popups::log(frame, app, area);
    }
    if app.show_help {
        popups::help(frame, app, area);
    }
    if let Some(selected) = app.leave {
        popups::leave(frame, app, selected, area);
    }
    if let Some(t) = app.toast_text()
        && !matches!(app.screen, Screen::Table)
    {
        // On the table the footer shows messages; elsewhere, a line at the bottom.
        let rect = Rect::new(
            area.x,
            area.bottom().saturating_sub(1),
            area.width,
            1.min(area.height),
        );
        frame.render_widget(
            Paragraph::new(fit(t, area.width as usize))
                .style(Style::new().fg(Color::Black).bg(Color::LightRed))
                .alignment(Alignment::Center),
            rect,
        );
    }
}

fn title_block(text: &str) -> Block<'static> {
    framed(
        Line::from(Span::styled(format!(" {text} "), yellow())),
        Color::Green,
    )
    .title_alignment(Alignment::Center)
}

// ------------------------------------------------------------------ menus

fn draw_menu(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(Span::styled(
            "No-limit Texas Hold'em in the terminal",
            dim(),
        )),
        Line::from(""),
    ];
    for (i, item) in MenuItem::ALL.iter().enumerate() {
        let (text, style) = if i == app.menu_row {
            (format!("▶ {} ◀", item.label()), yellow())
        } else {
            (format!("  {}  ", item.label()), Style::new())
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::Menu)));
    // Wrapped, not cut: the hint at the end and a long file path both matter.
    for l in wrap_text(&app.config_note, 56, 0) {
        lines.push(Line::from(Span::styled(l, dim())));
    }
    let rect = centered(area, 60, lines.len() as u16 + 2);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(title_block("♠ ♥  TEXAS HOLD'EM  ♦ ♣")),
        rect,
    );
}

fn on_off(b: bool) -> String {
    if b { "On" } else { "Off" }.to_string()
}

/// "moving button" → "Moving button".
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// A setup row: its label, its current value, and what it means.
fn setup_row_text(app: &App, row: SetupRow) -> (&'static str, String, String) {
    let s = &app.setup;
    let r = &s.rules;
    let settings = key_for(Place::Table, Cmd::Settings);
    match row {
        SetupRow::Name => (
            "Your name",
            app.join.name.clone(),
            "Type your name for this table; the default comes from client.name in holdem.toml."
                .to_string(),
        ),
        SetupRow::Seats => (
            "Seats",
            if s.host {
                s.seats.to_string()
            } else {
                format!("{}  (you + {} bots)", s.seats, s.seats - 1)
            },
            format!(
                "How many seats the table has ({MIN_PLAYERS} to {MAX_PLAYERS})."
            ),
        ),
        SetupRow::FillWithBots => (
            "Free seats",
            if s.fill_with_bots {
                "Filled with bots".to_string()
            } else {
                "Left empty (people only)".to_string()
            },
            "Seats that no person takes can be given to computer players.".to_string(),
        ),
        SetupRow::Chips => (
            "Starting chips",
            chips_text(r.starting_chips),
            "Every player starts with this many chips.".to_string(),
        ),
        SetupRow::Blind => (
            "Blinds",
            r.blinds_text(),
            "The forced bets: small blind / big blind at the start.".to_string(),
        ),
        SetupRow::Levels => {
            let (low, high) = r.level_steps();
            (
                "Blinds go up",
                r.levels_text(),
                format!(
                    "Rising blinds push the game towards an end (each level is {low}% to {high}% higher)."
                ),
            )
        }
        SetupRow::HandLimit => (
            "Hand limit",
            r.hand_limit_text(),
            "End the game after this many hands: the biggest stack wins.".to_string(),
        ),
        SetupRow::Antes => (
            "Antes",
            on_off(r.antes),
            format!(
                "Everyone also pays a small forced bet ({}) every hand.",
                r.ante_size_text()
            ),
        ),
        SetupRow::Straddle => (
            "Straddle",
            on_off(r.straddle),
            format!(
                "The player after the big blind posts {}, and acts last pre-flop.",
                r.straddle_size_text()
            ),
        ),
        SetupRow::Rebuys => (
            "Rebuys",
            capitalized(&r.rebuys_text()),
            "Busted players may buy back in while rebuys are open.".to_string(),
        ),
        SetupRow::Button => (
            "Button rule",
            capitalized(r.button_text()),
            "Dead button: the big blind moves exactly one player each hand, even when players leave."
                .to_string(),
        ),
        SetupRow::Showdown => (
            "Showdown",
            capitalized(r.showdown_text()),
            "Losers may muck: players who cannot win throw their cards away unseen.".to_string(),
        ),
        SetupRow::Fluctuation => (
            "Players come and go",
            on_off(s.fluctuation),
            "Bots leave now and then, and new ones sit down in empty seats.".to_string(),
        ),
        SetupRow::WinChance => (
            "Win-chance hint",
            if s.allow_win_chance {
                "Allowed"
            } else {
                "Not allowed"
            }
            .to_string(),
            format!(
                "Whether players may turn on the win-chance hint (in the settings, key {settings})."
            ),
        ),
    }
}

fn draw_setup(frame: &mut Frame, app: &App, area: Rect) {
    let rows = app.setup.rows();
    let host = app.setup.host;
    let fixed = 9u16;
    let height = (rows.len() as u16 + fixed).min(area.height);
    let rect = centered(area, 72, height);
    let inner_w = rect.width.saturating_sub(2) as usize;
    // Scroll the list when the terminal is short.
    let visible = (height.saturating_sub(fixed) as usize).max(1);
    let selected = app.setup.row.min(rows.len() - 1);
    let first = selected.saturating_sub(visible.saturating_sub(1));

    let mut lines: Vec<Line> = vec![Line::from("")];
    for (i, row) in rows.iter().enumerate().skip(first).take(visible) {
        let (label, value, _) = setup_row_text(app, *row);
        let text = if i == selected && *row == SetupRow::Name {
            // Typed in place: a cursor instead of the ◀ ▶ of stepped values.
            format!(" ▶ {label:<21}  {value}▏")
        } else if i == selected {
            format!(" ▶ {label:<21}◀ {value} ▶")
        } else {
            format!("   {label:<21}  {value}")
        };
        let style = if i == selected {
            yellow()
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(fit(&text, inner_w), style)));
    }
    lines.push(Line::from(""));
    let (_, _, about) = setup_row_text(app, rows[selected]);
    for l in wrap_text(&about, inner_w.saturating_sub(2), 0)
        .into_iter()
        .take(2)
    {
        lines.push(Line::from(Span::styled(
            format!(" {l}"),
            Style::new().fg(Color::Cyan),
        )));
    }
    while lines.len() < (height as usize).saturating_sub(5) {
        lines.push(Line::from(""));
    }
    if host {
        lines.push(Line::from(Span::styled(
            fit(
                &format!(
                    " Hosting as \"{}\" on port {} (change both in holdem.toml).",
                    app.config.server.name, app.config.server.port
                ),
                inner_w,
            ),
            dim(),
        )));
    } else {
        lines.push(Line::from(""));
    }
    let start = (
        key_for(Place::Setup, Cmd::Select),
        if host { "open the table" } else { "deal" },
    );
    if rows[selected] == SetupRow::Name {
        // Letters are typed here: no ←→ and no W, and only F1 opens the help.
        let entries = keys::reminder_where(Place::Setup, |c| {
            !matches!(c, Cmd::Left | Cmd::Help | Cmd::Save)
        });
        lines.push(Line::from(reminder_spans(
            std::iter::once(start)
                .chain(entries)
                .chain(std::iter::once((
                    keys::name(ratatui::crossterm::event::KeyCode::F(1)),
                    "help",
                ))),
        )));
    } else {
        lines.push(Line::from(reminder(&[start], Place::Setup)));
    }
    let title = if host {
        "Host a network game"
    } else {
        "New game against bots"
    };
    frame.render_widget(Paragraph::new(lines).block(title_block(title)), rect);
}

fn input_line(label: &str, value: &str, active: bool) -> Line<'static> {
    let cursor = if active { "▏" } else { "" };
    let style = if active { yellow() } else { Style::new() };
    Line::from(vec![
        Span::raw(format!(" {} {:<10}", if active { "▶" } else { " " }, label)),
        Span::styled(format!("{value}{cursor}"), style),
    ])
}

fn draw_join(frame: &mut Frame, app: &App, area: Rect) {
    let j = &app.join;
    let height = (14 + j.found.len().min(6)) as u16;
    let rect = centered(area, 76, height);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = vec![
        Line::from(""),
        input_line("Your name", &j.name, j.row == 0),
        input_line("Address", &j.address, j.row == 1),
        Line::from(Span::styled(
            "              e.g. 192.168.1.20, or myserver.net:7777",
            dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(" Tables on your network:", bold())),
    ];
    if j.found.is_empty() {
        lines.push(Line::from(Span::styled(
            "   none found yet (searching…)",
            dim(),
        )));
    }
    for (k, f) in j.found.iter().enumerate().take(6) {
        let active = j.row == k + 2;
        let state = match f.info.compatibility() {
            Err(m) => m.about_table(),
            Ok(_) if f.info.playing => "playing".to_string(),
            Ok(_) => "waiting".to_string(),
        };
        let text = format!(
            " {} {}  {}  {} connected, {} seats, {}",
            if active { "▶" } else { " " },
            f.info.table,
            f.address,
            f.info.players,
            f.info.seats,
            state
        );
        let style = if active { yellow() } else { Style::new() };
        lines.push(Line::from(Span::styled(fit(&text, inner_w), style)));
    }
    lines.push(Line::from(""));
    let status = if app.connecting() {
        format!("Connecting… ({} cancels)", key_for(Place::Join, Cmd::Back))
    } else {
        j.status.clone()
    };
    for l in wrap_text(&status, inner_w.saturating_sub(2), 0)
        .into_iter()
        .take(2)
    {
        lines.push(Line::from(Span::styled(
            format!(" {l}"),
            Style::new().fg(Color::LightRed),
        )));
    }
    while lines.len() < (height as usize).saturating_sub(3) {
        lines.push(Line::from(""));
    }
    let mut spans = reminder(&[], Place::Join);
    spans.push(Span::raw("   type to edit"));
    lines.push(Line::from(spans));
    frame.render_widget(
        Paragraph::new(lines).block(title_block("Join a network game")),
        rect,
    );
}

/// One line per optional rule that is switched on.
fn rules_summary(rules: &holdem::game::Rules, fluctuation: bool) -> String {
    let mut parts = vec![
        format!("{} chips", chips_text(rules.starting_chips)),
        format!("blinds {}", rules.blinds_text()),
    ];
    if rules.hands_per_level > 0 {
        parts.push(format!("up every {} hands", rules.hands_per_level));
    }
    if rules.hand_limit > 0 {
        parts.push(format!("{} hands", rules.hand_limit));
    }
    if rules.antes {
        parts.push("antes".to_string());
    }
    if rules.straddle {
        parts.push("straddle".to_string());
    }
    if rules.rebuy_levels > 0 {
        parts.push("rebuys".to_string());
    }
    if rules.dead_button {
        parts.push("dead button".to_string());
    }
    if rules.muck_losers {
        parts.push("mucking".to_string());
    }
    if fluctuation {
        parts.push("players come and go".to_string());
    }
    parts.join(" · ")
}

fn draw_lobby(frame: &mut Frame, app: &App, area: Rect) {
    let Some(session) = app.session.as_ref() else {
        return;
    };
    let rect = centered(area, 76, 20);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines: Vec<Line> = vec![Line::from("")];
    match &session.lobby {
        None => lines.push(Line::from(" Connecting to the table…")),
        Some(l) => {
            if let Some(addr) = &session.share {
                lines.push(Line::from(vec![
                    Span::raw(" Others join at  "),
                    Span::styled(addr.clone(), cyan()),
                ]));
                lines.push(Line::from(Span::styled(
                    fit(
                        " (on the same network; for internet games forward this port in your router)",
                        inner_w,
                    ),
                    dim(),
                )));
                lines.push(Line::from(""));
            }
            lines.push(Line::from(Span::styled(
                format!(" Players ({} of {} seats):", l.players.len(), l.seats),
                bold(),
            )));
            for p in &l.players {
                let mark = if p.ready {
                    "✔ ready"
                } else {
                    "… not ready"
                };
                let you = if p.you { "  (you)" } else { "" };
                let style = if p.ready { green() } else { Style::new() };
                lines.push(Line::from(vec![
                    Span::raw(format!("   {:<18}", p.name)),
                    Span::styled(mark, style),
                    Span::styled(you, dim()),
                ]));
            }
            lines.push(Line::from(""));
            let bots = if l.fill_with_bots {
                "Bots take the free seats."
            } else {
                "No bots: only people play."
            };
            lines.push(Line::from(Span::styled(format!(" {bots}"), dim())));
            for w in wrap_text(
                &rules_summary(&l.rules, l.fluctuation),
                inner_w.saturating_sub(2),
                0,
            ) {
                lines.push(Line::from(Span::styled(format!(" {w}"), dim())));
            }
        }
    }
    while lines.len() < 16 {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(" The game starts when everyone is ready."));
    lines.push(Line::from(reminder(&[], Place::Lobby)));
    let title = if session.table.is_empty() {
        "Lobby".to_string()
    } else {
        session.table.clone()
    };
    frame.render_widget(Paragraph::new(lines).block(title_block(&title)), rect);
}

// ------------------------------------------------------------------ table

fn tags(v: &TableView, seat: usize) -> String {
    let mut tags: Vec<&str> = Vec::new();
    if seat == v.button {
        tags.push("D");
    }
    if seat == v.sb_seat && !v.sb_dead {
        tags.push("SB");
    }
    if seat == v.bb_seat {
        tags.push("BB");
    }
    if v.straddle_seat == Some(seat) {
        tags.push("STR");
    }
    tags.join(" ")
}

fn seat_title(v: &TableView, seat: usize) -> Line<'static> {
    let s = &v.seats[seat];
    if s.vacant {
        return Line::from(Span::styled(" empty ", dim()));
    }
    let mut spans = vec![Span::raw(" "), Span::styled(s.name.clone(), bold())];
    if !s.connected {
        spans.push(Span::styled(" (away)", Style::new().fg(Color::LightRed)));
    }
    let t = tags(v, seat);
    if !t.is_empty() && !s.out {
        spans.push(Span::styled(
            format!(" [{t}]"),
            Style::new().fg(Color::Yellow),
        ));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn seat_border(v: &TableView, seat: usize) -> Color {
    let s = &v.seats[seat];
    if s.out || s.folded || !s.in_hand {
        Color::DarkGray
    } else if v.state == State::HandOver && s.payout > 0 {
        Color::Green
    } else if v.to_act == Some(seat) {
        Color::Yellow
    } else {
        Color::Gray
    }
}

fn status_span(v: &TableView, seat: usize) -> Span<'static> {
    let s = &v.seats[seat];
    if v.state == State::HandOver && s.payout > 0 {
        Span::styled(
            format!("Wins +{}", chips_text(s.payout)),
            green().add_modifier(Modifier::BOLD),
        )
    } else if s.folded {
        Span::styled("Folded", dim())
    } else if v.to_act == Some(seat) {
        Span::styled("thinking…", Style::new().fg(Color::Yellow))
    } else if !s.last_action.is_empty() {
        Span::raw(s.last_action.clone())
    } else if s.all_in {
        Span::styled("All-in", Style::new().fg(Color::Red))
    } else {
        Span::raw("")
    }
}

fn draw_seat(frame: &mut Frame, v: &TableView, seat: usize, area: Rect) {
    let s: &SeatView = &v.seats[seat];
    let block = framed(seat_title(v, seat), seat_border(v, seat));
    let mut lines: Vec<Line<'static>> = Vec::new();
    if s.vacant {
        lines.push(Line::from(Span::styled("Empty seat", dim())));
    } else if s.rebuy_pending {
        lines.push(Line::from(Span::styled(
            "Busted",
            Style::new().fg(Color::Red),
        )));
        lines.push(Line::from(Span::styled("may rebuy…", dim())));
    } else if s.out {
        lines.push(Line::from(Span::styled("Eliminated", dim())));
        if let Some(p) = s.place {
            lines.push(Line::from(Span::styled(
                format!("Finished {}", ordinal(p)),
                dim(),
            )));
        }
    } else {
        lines.push(Line::from(format!("Stack {}", chips_text(s.chips))));
        lines.push(Line::from(status_span(v, seat)));
        if !s.cards.is_empty() {
            lines.push(cards_line(&s.cards));
        } else if s.hidden_cards {
            lines.push(hidden_line());
        }
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_board(frame: &mut Frame, v: &TableView, area: Rect) {
    let block = framed(
        Line::from(vec![
            Span::raw(" "),
            Span::styled(v.street.name(), bold()),
            Span::raw(" "),
        ]),
        Color::Green,
    );
    let inner_w = area.width.saturating_sub(2) as usize;
    let hpl = v.rules.hands_per_level;
    let mut level = format!("Blinds {}/{}", chips_text(v.sb), chips_text(v.bb));
    if v.ante > 0 {
        level.push_str(&format!(" ante {}", v.ante));
    }
    level.push_str(&format!("  ·  Level {}", v.level));
    if hpl > 0 && v.hand_no > 0 {
        let left = hpl - (v.hand_no - 1) % hpl;
        level.push_str(&format!(
            ", up in {} hand{}",
            left,
            if left == 1 { "" } else { "s" }
        ));
    }
    let bet = if v.state == State::Betting && v.current_bet > 0 {
        format!("Bet to match {}", chips_text(v.current_bet))
    } else {
        String::new()
    };
    let lines = centre_vertically(
        vec![
            board_line(&v.board),
            Line::from(""),
            Line::from(Span::styled(fit(&level, inner_w), dim())),
            Line::from(Span::styled(bet, dim())),
        ],
        area,
    );
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

/// The pot measured in big blinds, e.g. "worth 9.5 big blinds": how big it is
/// compared with what it costs to play a hand.
pub fn big_blinds_text(amount: u32, bb: u32) -> String {
    let bbs = f64::from(amount) / f64::from(bb.max(1));
    let number = if bbs < 10.0 && bbs.fract() != 0.0 {
        format!("{bbs:.1}")
    } else {
        format!("{}", bbs.round() as u64)
    };
    let unit = if number == "1" {
        "big blind"
    } else {
        "big blinds"
    };
    format!("worth {number} {unit}")
}

fn draw_pot(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let finished = matches!(v.state, State::HandOver | State::GameOver);
    let amount = if finished { v.last_pot } else { v.pot };
    let flashing = app.pot_flash.is_some_and(|t| {
        t.elapsed() < Duration::from_millis(u64::from(app.config.client.pot_flash_ms))
    });
    let (main, border) = if finished {
        (Color::Green, Color::Green)
    } else if flashing {
        (Color::LightYellow, Color::LightYellow)
    } else {
        (Color::Yellow, Color::Yellow)
    };
    let title = if finished {
        " ★ POT WON ★ "
    } else {
        " ◆ POT ◆ "
    };
    let block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(Style::new().fg(border).add_modifier(Modifier::BOLD))
        .title(Line::from(Span::styled(
            title,
            Style::new().fg(main).add_modifier(Modifier::BOLD),
        )))
        .title_alignment(Alignment::Center);
    let inner_w = area.width.saturating_sub(2) as usize;
    let number = chips_text(amount);
    let big = big_number(&number);
    let style = Style::new().fg(main).add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> = if text_width(&big[0]) <= inner_w {
        big.iter()
            .map(|r| Line::from(Span::styled(r.clone(), style)))
            .collect()
    } else {
        vec![
            Line::from(""),
            Line::from(Span::styled(format!("{number} chips"), style)),
            Line::from(""),
        ]
    };
    let extra = if finished {
        let winners: Vec<String> = v
            .seats
            .iter()
            .filter(|s| s.payout > 0)
            .map(|s| s.name.clone())
            .collect();
        if winners.is_empty() {
            Line::from("")
        } else {
            Line::from(Span::styled(
                fit(&format!("→ {}", winners.join(", ")), inner_w),
                green().add_modifier(Modifier::BOLD),
            ))
        }
    } else if v.pots.len() > 1 {
        let parts: Vec<String> = v
            .pots
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i == 0 {
                    format!("main {}", chips_text(*p))
                } else {
                    format!("side {}", chips_text(*p))
                }
            })
            .collect();
        Line::from(Span::styled(
            fit(&parts.join(" · "), inner_w),
            Style::new().fg(main),
        ))
    } else if amount > 0 {
        Line::from(Span::styled(
            fit(&big_blinds_text(amount, v.bb), inner_w),
            Style::new().fg(main),
        ))
    } else {
        Line::from("")
    };
    lines.push(extra);
    let lines = centre_vertically(lines, area);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        area,
    );
}

fn draw_you(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let Some(me) = v.you else {
        let text = if app
            .session
            .as_ref()
            .is_some_and(|s| v.waiting.contains(&s.name))
        {
            "You are waiting for a free seat: you will be dealt in at the next hand."
        } else {
            "You are watching. You will play again in the next game."
        };
        let block = framed(Line::from(" You "), Color::DarkGray);
        frame.render_widget(
            Paragraph::new(text).wrap(Wrap { trim: true }).block(block),
            area,
        );
        return;
    };
    let s = &v.seats[me];
    let acting = v.legal.is_some();
    let color = if s.out || s.folded {
        Color::DarkGray
    } else if v.state == State::HandOver && s.payout > 0 {
        Color::Green
    } else if acting {
        Color::Yellow
    } else {
        Color::White
    };
    let block = framed(seat_title(v, me), color);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if s.out && !s.rebuy_pending {
        lines.push(Line::from(Span::styled("You are out of the game", dim())));
    } else {
        if s.cards.len() == 2 {
            let mut spans = cards_line(&s.cards).spans;
            if let Some(d) = &v.your_hand {
                spans.push(Span::raw("   "));
                spans.push(Span::styled(d.clone(), bold()));
            }
            if s.folded {
                spans.push(Span::styled("   (folded)", dim()));
            }
            lines.push(Line::from(spans));
        }
        let mut info = format!(
            "Stack {}    In this round {}    {}",
            chips_text(s.chips),
            chips_text(s.bet),
            s.last_action
        );
        if s.rebuys > 0 {
            info.push_str(&format!("    Rebuys {}", s.rebuys));
        }
        lines.push(Line::from(info));
        let mut parts: Vec<Span<'static>> = Vec::new();
        if let Some(h) = app.current_hint() {
            parts.push(Span::styled(
                format!(
                    "Win chance vs {} random hand{}: about {:.0}%",
                    h.opponents,
                    if h.opponents == 1 { "" } else { "s" },
                    h.win * 100.0
                ),
                Style::new().fg(Color::Cyan),
            ));
        }
        if acting && let Some(secs) = v.turn_seconds_left {
            if !parts.is_empty() {
                parts.push(Span::raw("    "));
            }
            let style = if secs <= 5 {
                Style::new()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::Yellow)
            };
            parts.push(Span::styled(format!("{secs}s left to act"), style));
        }
        if !parts.is_empty() {
            lines.push(Line::from(parts));
        }
    }
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn action_spans(v: &TableView) -> Vec<Span<'static>> {
    let Some(l) = v.legal else {
        return Vec::new();
    };
    let k = |cmd: Cmd| Span::styled(format!("[{}]", key_for(Place::Turn, cmd)), yellow());
    let mut out: Vec<Span<'static>> = Vec::new();
    if !l.can_check {
        out.push(k(Cmd::Fold));
        out.push(Span::raw(" Fold   "));
    }
    out.push(k(Cmd::Call));
    out.push(Span::raw(if l.can_check {
        " Check   ".to_string()
    } else {
        format!(" Call {}   ", chips_text(l.to_call))
    }));
    if l.can_raise {
        out.push(k(Cmd::Raise));
        let word = if v.current_bet == 0 {
            "Bet"
        } else {
            "Raise to"
        };
        out.push(Span::raw(format!(
            " {} (min {})   ",
            word,
            chips_text(l.min_to)
        )));
    }
    if l.can_go_all_in() {
        out.push(k(Cmd::AllIn));
        out.push(Span::raw(format!(" All-in {}", chips_text(l.stack))));
    }
    out
}

/// The three lines inside the bottom panel, each fitted to `width` columns.
fn footer_lines(app: &App, v: &TableView, width: usize) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    match v.state {
        State::HandOver | State::GameOver => {
            if let Some(first) = v.result_lines.first() {
                lines.push(Line::from(Span::styled(
                    fit(first, width),
                    green().add_modifier(Modifier::BOLD),
                )));
            }
            if v.result_lines.len() > 1 {
                let mut text = v.result_lines[1].clone();
                if v.result_lines.len() > 2 {
                    text = format!("{}  (+{} more in the log)", text, v.result_lines.len() - 2);
                }
                lines.push(Line::from(Span::styled(fit(&text, width), green())));
            }
            let mut spans: Vec<Span<'static>> = Vec::new();
            if let Some(chips) = v.rebuy_offer {
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Rebuy))));
                spans.push(Span::raw(format!(
                    " Rebuy for {} chips   ",
                    chips_text(chips)
                )));
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Ready))));
                spans.push(Span::raw(" Give up"));
            } else if v.you_ready {
                spans.push(Span::styled(
                    format!(
                        "Waiting for the others ({} of {} ready)",
                        v.ready.0, v.ready.1
                    ),
                    Style::new().fg(Color::Yellow),
                ));
            } else {
                spans.push(key(&format!("[{}]", key_for(Place::HandOver, Cmd::Ready))));
                spans.push(Span::raw(if v.last_hand {
                    " Final results"
                } else {
                    " Next hand"
                }));
                if v.ready.1 > 1 {
                    spans.push(Span::styled(
                        format!("   ({} of {} ready)", v.ready.0, v.ready.1),
                        dim(),
                    ));
                }
            }
            if let Some(secs) = v.next_hand_seconds_left {
                spans.push(Span::styled(format!("   · starts in {secs}s"), dim()));
            }
            lines.push(Line::from(spans));
        }
        State::Runout => {
            lines.push(Line::from(Span::styled(
                "Everyone is all-in: the rest of the board is being dealt…",
                Style::new().fg(Color::Yellow),
            )));
        }
        State::Betting => {
            if let Some(l) = v.legal {
                lines.push(Line::from(action_spans(v)));
                let mut s = format!("Pot {}", chips_text(v.pot));
                if l.to_call > 0 {
                    let odds = 100.0 * l.to_call as f32 / (v.pot + l.to_call) as f32;
                    s.push_str(&format!(
                        "   ·   To call {}   ·   Pot odds {:.0}%",
                        chips_text(l.to_call),
                        odds
                    ));
                } else {
                    s.push_str("   ·   Nothing to call");
                }
                lines.push(Line::from(Span::styled(
                    fit(&s, width),
                    Style::new().fg(Color::Cyan),
                )));
            } else if let Some(actor) = v.to_act {
                let who = &v.seats[actor].name;
                let folded = v.you.is_some_and(|s| v.seats[s].folded);
                let prefix = if folded {
                    "You folded this hand.  "
                } else {
                    ""
                };
                let mut text = format!("{prefix}{who} is thinking…");
                if let Some(secs) = v.turn_seconds_left {
                    text.push_str(&format!(" ({secs}s left)"));
                }
                lines.push(Line::from(Span::styled(
                    fit(&text, width),
                    Style::new().fg(Color::Yellow),
                )));
            }
        }
    }

    // The third line: a confirmation request, a status message, or the key reminder.
    let extra = if app.all_in_armed {
        let stack = v.legal.map(|l| l.stack).unwrap_or(0);
        Some(Line::from(Span::styled(
            fit(
                &format!(
                    "Press {} again to put all {} chips in; any other key cancels.",
                    key_for(Place::Turn, Cmd::AllIn),
                    chips_text(stack)
                ),
                width,
            ),
            Style::new()
                .fg(Color::LightRed)
                .add_modifier(Modifier::BOLD),
        )))
    } else {
        app.toast_text().map(|t| {
            Line::from(Span::styled(
                fit(t, width),
                Style::new().fg(Color::LightRed),
            ))
        })
    };
    if let Some(line) = extra {
        while lines.len() >= 3 {
            lines.remove(0);
        }
        lines.push(line);
    } else if lines.len() < 3 {
        lines.push(Line::from(Span::styled(
            fit(&keys::reminder_text(Place::Table), width),
            dim(),
        )));
    }
    lines
}

fn draw_header(frame: &mut Frame, app: &App, v: &TableView, area: Rect) {
    let style = Style::new().fg(Color::Black).bg(Color::Green);
    let alive = v.seats.iter().filter(|s| !s.out).count();
    let seated = v.seats.iter().filter(|s| !s.vacant).count();
    let hand = if v.rules.hand_limit > 0 {
        format!("Hand {} of {}", v.hand_no, v.rules.hand_limit)
    } else {
        format!("Hand #{}", v.hand_no)
    };
    let left = format!(" ♠ {hand}  ·  {alive} of {seated} players in");
    let mode = match app.session.as_ref().map(|s| s.kind) {
        Some(Kind::Host) => "HOSTING  ·  ",
        Some(Kind::Guest) => "ONLINE  ·  ",
        _ => "",
    };
    let sound = match app.sound.unavailable() {
        Some(_) => "no sound".to_string(),
        None if app.sound.volume() == 0 => "sound off".to_string(),
        None => format!("sound {}%", app.sound.volume()),
    };
    let paused = if v.paused { "PAUSED  ·  " } else { "" };
    let right = format!(
        "{}{}speed {}  ·  {}  ·  {} settings ",
        mode,
        paused,
        app.speed().name.to_lowercase(),
        sound,
        key_for(Place::Table, Cmd::Settings)
    );
    let right_w = (text_width(&right) as u16).min(area.width);
    let [l, r] = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_w)]).areas(area);
    frame.render_widget(Paragraph::new(fit(&left, l.width as usize)).style(style), l);
    frame.render_widget(
        Paragraph::new(right)
            .style(style)
            .alignment(Alignment::Right),
        r,
    );
}

/// Width of the pot panel next to the board.
const POT_WIDTH: u16 = 30;

// The table screen, top to bottom: header, rows of opponents, board and pot,
// your seat, and the panel with your options.
const HEADER_H: u16 = 1;
const SEAT_H: u16 = 5;
const MIDDLE_MIN_H: u16 = 6;
const YOU_H: u16 = 6;
const FOOTER_H: u16 = 5;
/// Narrowest terminal the table is drawn in.
const MIN_WIDTH: u16 = 80;

fn draw_table(frame: &mut Frame, app: &App, area: Rect) {
    let Some(v) = app.view() else {
        frame.render_widget(
            Paragraph::new("Shuffling…").alignment(Alignment::Center),
            centered(area, 30, 1),
        );
        return;
    };
    let others: Vec<usize> = (0..v.seats.len()).filter(|&i| Some(i) != v.you).collect();
    let count = others.len().max(1);
    let cols = if count <= 4 { count } else { count.div_ceil(2) };
    let rows = count.div_ceil(cols);
    let need_h = HEADER_H + rows as u16 * SEAT_H + MIDDLE_MIN_H + YOU_H + FOOTER_H;
    if area.width < MIN_WIDTH || area.height < need_h {
        let msg = format!(
            "Terminal too small: this table needs at least {}x{} (now {}x{}). Enlarge the window, or press {} to leave.",
            MIN_WIDTH,
            need_h,
            area.width,
            area.height,
            key_for(Place::Table, Cmd::Leave)
        );
        frame.render_widget(
            Paragraph::new(msg)
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center),
            centered(area, area.width.min(60), 6),
        );
        return;
    }

    let [header, main, footer] = Layout::vertical([
        Constraint::Length(HEADER_H),
        Constraint::Min(0),
        Constraint::Length(FOOTER_H),
    ])
    .areas(area);
    draw_header(frame, app, v, header);

    let (table_area, log_area) = if area.width >= 110 {
        let [t, l] = Layout::horizontal([Constraint::Min(0), Constraint::Length(38)]).areas(main);
        (t, Some(l))
    } else {
        (main, None)
    };

    let [opp_area, middle, you_area] = Layout::vertical([
        Constraint::Length(rows as u16 * SEAT_H),
        Constraint::Min(MIDDLE_MIN_H),
        Constraint::Length(YOU_H),
    ])
    .areas(table_area);

    let row_areas = Layout::vertical(vec![Constraint::Length(SEAT_H); rows]).split(opp_area);
    let mut k = 0;
    for r in 0..rows {
        let in_row = cols.min(count - r * cols);
        let col_areas = Layout::horizontal(vec![Constraint::Ratio(1, in_row as u32); in_row])
            .split(row_areas[r]);
        for c in 0..in_row {
            if let Some(&seat) = others.get(k) {
                draw_seat(frame, v, seat, col_areas[c]);
            }
            k += 1;
        }
    }

    let [board_area, pot_area] =
        Layout::horizontal([Constraint::Min(0), Constraint::Length(POT_WIDTH)]).areas(middle);
    draw_board(frame, v, board_area);
    draw_pot(frame, app, v, pot_area);
    draw_you(frame, app, v, you_area);

    if let Some(log_area) = log_area
        && let Some(session) = app.session.as_ref()
    {
        let inner_h = log_area.height.saturating_sub(2) as usize;
        let inner_w = log_area.width.saturating_sub(2) as usize;
        let (lines, _) = log_view(&session.log, inner_h, inner_w, 0);
        let block = framed(Line::from(" Log (L: full log) "), Color::DarkGray);
        frame.render_widget(Paragraph::new(lines).block(block), log_area);
    }

    let block = framed(Line::from(" Your move "), Color::Gray);
    let inner_w = footer.width.saturating_sub(2) as usize;
    frame.render_widget(
        Paragraph::new(footer_lines(app, v, inner_w)).block(block),
        footer,
    );

    if let Some(prompt) = &app.raise {
        popups::raise(frame, v, prompt, table_area);
    }
}

// ------------------------------------------------------------------ final standings

/// "Tight-aggressive (84% match): tight, aggressive, bluffs sometimes".
pub fn style_line(stats: &holdem::stats::Stats, min_hands: u32) -> String {
    match stats::profile(stats, min_hands) {
        Some(p) => format!(
            "{} ({}% match): {}",
            p.ranking[0].0, p.ranking[0].1, p.summary
        ),
        None => format!(
            "not enough hands yet ({} of {} counted)",
            stats.hands, min_hands
        ),
    }
}

fn draw_over(frame: &mut Frame, app: &App, area: Rect) {
    let Some(v) = app.view() else {
        return;
    };
    let my_name = app
        .session
        .as_ref()
        .map(|s| s.name.clone())
        .unwrap_or_default();
    let mine = v.standings.iter().find(|s| s.is_human && s.name == my_name);
    let headline = match mine.and_then(|s| s.place) {
        Some(1) => "You won!".to_string(),
        Some(p) => format!(
            "You finished {} of {}",
            ordinal(p),
            v.standings.iter().filter(|s| s.place.is_some()).count()
        ),
        None => "Game over".to_string(),
    };
    let width = 72u16.min(area.width);
    let inner_w = width.saturating_sub(2) as usize;
    let mut lines: Vec<Line<'static>> = vec![
        Line::from(""),
        Line::from(Span::styled(headline, yellow())),
        Line::from(""),
    ];
    for s in v.standings.iter().take(12) {
        let place = s.place.map(ordinal).unwrap_or_else(|| "-".to_string());
        let style_name = if s.is_human {
            "(person)".to_string()
        } else {
            s.style.clone()
        };
        let text = format!(
            "{:<5}{:<17}{:<18}{}",
            place,
            fit(&s.name, 16),
            style_name,
            s.status
        );
        let style = if s.is_human && s.name == my_name {
            yellow()
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(fit(&text, inner_w), style)));
    }
    if v.standings.len() > 12 {
        lines.push(Line::from(Span::styled(
            format!("… and {} more", v.standings.len() - 12),
            dim(),
        )));
    }
    lines.push(Line::from(""));
    if let Some(m) = mine {
        let text = format!(
            "Your style: {}",
            style_line(&m.stats, app.config.client.style_min_hands)
        );
        for l in wrap_text(&text, inner_w.saturating_sub(2), 2) {
            lines.push(Line::from(Span::styled(l, Style::new().fg(Color::Cyan))));
        }
        lines.push(Line::from(""));
    }
    let solo = app.session.as_ref().is_some_and(|s| s.kind == Kind::Solo);
    let next = (
        key_for(Place::Over, Cmd::Select),
        if solo { "main menu" } else { "play again" },
    );
    lines.push(Line::from(reminder(&[next], Place::Over)));
    let height = (lines.len() + 2) as u16;
    let block =
        framed(Line::from(" Final standings "), Color::Green).title_alignment(Alignment::Center);
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(block),
        centered(area, width, height),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_lines_never_exceed_the_width() {
        let texts = [
            "Ava raises to 1200 (all-in)",
            "--- Hand #12 (level 3, blinds 25/50) - the button is on Chloe ---",
            "Ben and Dmitri split side pot 2 (1234) with Full House, Kings full of Fours",
            "Showdown (board: A♠ K♦ 10♥ 5♣ 2♠)",
            "Averyveryveryverylongwordthatcannotfitonasinglelineatall and more",
            "",
        ];
        for width in [5usize, 10, 20, 36, 60] {
            for t in &texts {
                let lines = wrap_text(t, width, 2);
                assert!(!lines.is_empty());
                for l in &lines {
                    assert!(text_width(l) <= width, "{l:?} is wider than {width}");
                }
                let joined: Vec<String> = lines.iter().map(|l| l.trim().to_string()).collect();
                assert_eq!(joined.join(" ").replace(' ', ""), t.replace(' ', ""));
            }
        }
    }

    #[test]
    fn fit_truncates_by_display_width() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello…");
        assert_eq!(text_width(&fit("A♠ K♦ Q♥ J♣", 5)), 5);
        assert_eq!(fit("abc", 0), "");
    }

    #[test]
    fn pot_sizes_in_big_blinds() {
        assert_eq!(big_blinds_text(30, 20), "worth 1.5 big blinds");
        assert_eq!(big_blinds_text(20, 20), "worth 1 big blind");
        assert_eq!(big_blinds_text(190, 20), "worth 9.5 big blinds");
        assert_eq!(big_blinds_text(1234, 20), "worth 62 big blinds");
        assert!(text_width(&big_blinds_text(u32::MAX, 1)) <= (POT_WIDTH - 2) as usize);
    }

    #[test]
    fn big_numbers_line_up_and_fit_the_pot_panel() {
        for n in ["0", "7", "1,234", "999,999"] {
            let rows = big_number(n);
            let w = text_width(&rows[0]);
            assert!(
                rows.iter().all(|r| text_width(r) == w),
                "{n} has ragged rows"
            );
        }
        assert!(text_width(&big_number("999,999")[0]) <= (POT_WIDTH - 2) as usize);
    }
}

#[cfg(test)]
mod render_tests {
    use super::draw;
    use crate::app::{App, Screen};
    use crate::sound::Sound;
    use holdem::config::Config;
    use holdem::game::State;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::time::{Duration, Instant};

    const SIZES: [(u16, u16); 5] = [(80, 23), (80, 28), (109, 30), (150, 45), (50, 12)];

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    /// Draws the current screen, alone and under every window, at several sizes.
    fn draw_everything(app: &mut App) {
        for (w, h) in SIZES {
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            for overlay in 0..6 {
                app.show_help = overlay == 1;
                app.show_log = overlay == 2;
                app.show_settings = overlay == 3;
                app.show_profile = overlay == 4;
                app.leave = (overlay == 5).then_some(1);
                let pages = if overlay == 1 {
                    crate::help::page_count()
                } else {
                    1
                };
                for page in 0..pages {
                    app.help_page = page;
                    app.help_scroll = usize::MAX / 2;
                    app.log_scroll = usize::MAX / 2;
                    terminal.draw(|f| draw(f, app)).unwrap();
                }
            }
        }
        app.show_help = false;
        app.show_log = false;
        app.show_settings = false;
        app.show_profile = false;
        app.leave = None;
    }

    fn fast_config() -> Config {
        let mut c = Config::default();
        c.game.speeds.push(holdem::config::SpeedConfig {
            name: "Test".to_string(),
            ms: 20,
        });
        c.client.speed = "Test".to_string();
        c.server.port = 0; // any free port
        c.server.next_hand_seconds = 1;
        c
    }

    /// Plays by pressing keys like a person would, drawing everything after each step.
    fn play(app: &mut App, hands: u32, draw_every: usize) {
        let deadline = Instant::now() + Duration::from_secs(60);
        let mut step = 0;
        loop {
            assert!(Instant::now() < deadline, "the game got stuck");
            app.tick();
            if step % draw_every == 0 {
                draw_everything(app);
            }
            step += 1;
            if app.screen == Screen::Over {
                break;
            }
            let Some(v) = app.view() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            if v.hand_no > hands && v.state == State::Betting {
                break;
            }
            if let Some(l) = v.legal {
                // Raise now and then (through the raise box), call cheap bets, else fold.
                let code = if l.can_raise && step % 9 == 0 {
                    KeyCode::Char('r')
                } else if l.can_check || l.to_call <= l.stack / 10 {
                    KeyCode::Char('c')
                } else {
                    KeyCode::Char('f')
                };
                press(app, code);
                if app.raise.is_some() {
                    draw_everything(app);
                    press(app, KeyCode::Char('m'));
                    press(app, KeyCode::Enter);
                }
            } else if v.state == State::HandOver && !v.you_ready {
                press(app, KeyCode::Enter);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn screen_text(app: &App, w: u16, h: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut out = String::new();
        for y in 0..h {
            for x in 0..w {
                out.push_str(buffer[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// Prints screens for a person to look at: `cargo test show_screens -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn show_screens() {
        let mut app = App::new(fast_config(), Some(1), Sound::new(true));
        println!("{}", screen_text(&app, 80, 24));
        press(&mut app, KeyCode::Enter);
        println!("{}", screen_text(&app, 80, 24));
        press(&mut app, KeyCode::Esc);
        app.menu_row = 2;
        press(&mut app, KeyCode::Enter);
        println!("{}", screen_text(&app, 80, 24));
        press(&mut app, KeyCode::Esc);
        app.menu_row = 0;
        press(&mut app, KeyCode::Enter);
        press(&mut app, KeyCode::Enter);
        let deadline = Instant::now() + Duration::from_secs(20);
        while app
            .view()
            .is_none_or(|v| v.legal.is_none_or(|l| !l.can_raise))
        {
            assert!(Instant::now() < deadline);
            app.tick();
            if app.view().is_some_and(|v| v.state == State::HandOver) {
                press(&mut app, KeyCode::Enter);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        println!("{}", screen_text(&app, 80, 30));
        press(&mut app, KeyCode::Char('r'));
        println!("{}", screen_text(&app, 80, 30));
        press(&mut app, KeyCode::Esc);
        for k in ['s', 'p', 'l', 'q', '?'] {
            press(&mut app, KeyCode::Char(k));
            println!("{}", screen_text(&app, 80, 24));
            press(&mut app, KeyCode::Esc);
        }
    }

    /// The hint is on screen for the whole hand while you are in it: on your
    /// turn, while others act, and during an all-in run-out; never after a fold.
    #[test]
    fn the_win_chance_shows_while_you_are_in_the_hand() {
        let (mut waiting, mut runout, mut folded) = (0, 0, 0);
        for seed in 0..6u64 {
            let mut app = App::new(fast_config(), Some(seed), Sound::new(true));
            press(&mut app, KeyCode::Enter);
            press(&mut app, KeyCode::Enter);
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut turns = 0;
            while turns < 30 && Instant::now() < deadline {
                app.tick();
                let Some(v) = app.view().cloned() else {
                    continue;
                };
                if v.game_over {
                    break;
                }
                let me = &v.seats[v.you.unwrap()];
                let live =
                    matches!(v.state, State::Betting | State::Runout) && me.in_hand && !me.folded;
                let shown = screen_text(&app, 100, 32).contains("Win chance");
                assert_eq!(
                    shown,
                    live,
                    "seed {}: state {:?}, folded {}, your turn {}",
                    seed,
                    v.state,
                    me.folded,
                    v.legal.is_some()
                );
                if live && v.legal.is_none() {
                    waiting += 1;
                }
                if live && v.state == State::Runout {
                    runout += 1;
                }
                if me.folded {
                    folded += 1;
                }
                if let Some(l) = v.legal {
                    turns += 1;
                    // Mix calls, folds and all-ins, to see every situation.
                    let key = match turns % 4 {
                        0 | 2 if !l.can_check => 'f',
                        3 if l.can_go_all_in() => 'a',
                        _ => 'c',
                    };
                    press(&mut app, KeyCode::Char(key));
                    if key == 'a' {
                        press(&mut app, KeyCode::Char('a'));
                    }
                } else if v.state == State::HandOver && !v.you_ready {
                    press(&mut app, KeyCode::Enter);
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        // Every situation was seen: waiting for others, an all-in run-out, folded.
        assert!(
            waiting > 0 && runout > 0 && folded > 0,
            "waiting {waiting} run-out {runout} folded {folded}"
        );
    }

    #[test]
    fn menus_render_at_every_size() {
        let mut app = App::new(fast_config(), Some(1), Sound::new(true));
        draw_everything(&mut app);
        press(&mut app, KeyCode::Enter); // Play against bots
        assert_eq!(app.screen, Screen::Setup);
        for _ in 0..12 {
            draw_everything(&mut app);
            press(&mut app, KeyCode::Right);
            press(&mut app, KeyCode::Down);
        }
        press(&mut app, KeyCode::Esc);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Enter); // Join a network game
        assert_eq!(app.screen, Screen::Join);
        for c in "10.0.0.1".chars() {
            press(&mut app, KeyCode::Char(c));
        }
        draw_everything(&mut app);
        press(&mut app, KeyCode::Esc);
        assert_eq!(app.screen, Screen::Menu);
    }

    #[test]
    fn a_solo_game_renders_from_start_to_finish() {
        let mut config = fast_config();
        config.game.rules.starting_chips = 1000;
        config.game.rules.antes = true;
        config.game.rules.straddle = true;
        config.game.rules.rebuy_levels = 1;
        config.game.rules.dead_button = true;
        config.game.rules.muck_losers = true;
        config.game.rules.hand_limit = 6;
        config.game.seats = 9;
        config.game.fluctuation.enabled = true;
        let mut app = App::new(config, Some(3), Sound::new(true));
        app.setup.fluctuation = true;
        press(&mut app, KeyCode::Enter); // Play against bots
        press(&mut app, KeyCode::Enter); // Deal
        assert_eq!(app.screen, Screen::Table);
        play(&mut app, 100, 3);
        assert_eq!(app.screen, Screen::Over, "the hand limit ends the game");
        let v = app.view().unwrap();
        assert!(v.game_over && !v.standings.is_empty());
        draw_everything(&mut app);
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Menu);
        assert!(app.session.is_none());
    }

    #[test]
    fn a_hosted_game_renders_the_lobby_and_the_table() {
        let mut app = App::new(fast_config(), Some(4), Sound::new(true));
        app.menu_row = 1; // Host a network game
        press(&mut app, KeyCode::Enter);
        assert!(app.setup.host);
        press(&mut app, KeyCode::Enter);
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.session.as_ref().is_none_or(|s| s.lobby.is_none()) {
            assert!(Instant::now() < deadline, "no lobby");
            app.tick();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.screen, Screen::Lobby);
        assert!(app.session.as_ref().unwrap().share.is_some());
        draw_everything(&mut app);
        press(&mut app, KeyCode::Enter); // ready
        play(&mut app, 3, 4);
        let v = app.view().unwrap();
        assert!(v.hand_no >= 3 || v.game_over);
        // Leaving through the dialog goes back to the menu and closes the table.
        press(&mut app, KeyCode::Char('q'));
        assert_eq!(app.leave, Some(0));
        press(&mut app, KeyCode::Enter);
        assert_eq!(app.screen, Screen::Menu);
    }
}
