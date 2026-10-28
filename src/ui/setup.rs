//! The setup screen of a game against bots or a hosted game.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use holdem::game::{MAX_PLAYERS, MIN_PLAYERS, chips_text};

use super::style::{centered, dim, key_for, reminder, reminder_spans, yellow};
use super::text::{fit, wrap_text};
use super::title_block;
use crate::app::{App, SetupRow};
use crate::keys::{self, Cmd, Place};

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

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
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
