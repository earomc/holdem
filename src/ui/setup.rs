//! The setup screen of a game against bots or a hosted game.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use holdem::game::{MAX_PLAYERS, MIN_PLAYERS, chips_text};

use super::style::{centered, dim, key_for, reminder, reminder_spans, yellow};
use super::text::wrap_text;
use super::{boxed, screen_box, title_block};
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
        SetupRow::Table => (
            "Table",
            app.table().name.clone(),
            "Start from one of the tables in holdem.toml; the rows below can still be changed."
                .to_string(),
        ),
        SetupRow::Seats => (
            "Seats",
            if s.host {
                s.seats.to_string()
            } else {
                format!("{}  (you + {} bots)", s.seats, s.seats - 1)
            },
            format!("How many seats the table has ({MIN_PLAYERS} to {MAX_PLAYERS})."),
        ),
        SetupRow::FillWithBots => (
            "Free seats",
            if s.fill_with_bots {
                "Filled with bots".to_string()
            } else {
                "Left empty (people only)".to_string()
            },
            if s.fill_with_bots {
                "Seats that no person takes get computer players."
            } else {
                "Only people play: the table has as many players as there are people."
            }
            .to_string(),
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
                if r.hands_per_level == 0 {
                    "The blinds stay at their starting size the whole game.".to_string()
                } else {
                    format!(
                        "Rising blinds push the game towards an end (each level is {low}% to \
                         {high}% higher)."
                    )
                },
            )
        }
        SetupRow::HandLimit => (
            "Hand limit",
            r.hand_limit_text(),
            if r.hand_limit == 0 {
                "The game goes on until one player has all the chips."
            } else {
                "The game ends after this many hands: the biggest stack wins."
            }
            .to_string(),
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
            if r.rebuy_levels == 0 {
                "A player who loses all their chips is out."
            } else {
                "A player who loses all their chips may buy back in; you are asked after the hand."
            }
            .to_string(),
        ),
        SetupRow::Button => (
            "Button rule",
            capitalized(r.button_text()),
            if r.dead_button {
                "The big blind moves exactly one seat each hand, so nobody skips it when players \
                 leave; the button or small blind may then sit on an empty seat."
            } else {
                "The button goes to the next player each hand."
            }
            .to_string(),
        ),
        SetupRow::Showdown => (
            "Showdown",
            capitalized(r.showdown_text()),
            if r.muck_losers {
                "Hands are shown in turn, and a player already beaten throws their cards away \
                 unseen (all-in hands always show)."
            } else {
                "Every hand still in at the end is turned up."
            }
            .to_string(),
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
            if s.allow_win_chance {
                format!(
                    "Players may turn on the win-chance hint (in the settings, key {settings})."
                )
            } else {
                "Nobody at this table can turn on the win-chance hint.".to_string()
            },
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
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(Line::from(""));
    let (_, _, about) = setup_row_text(app, rows[selected]);
    for l in wrap_text(&about, inner_w.saturating_sub(2), 0) {
        lines.push(Line::from(Span::styled(
            format!(" {l}"),
            Style::new().fg(Color::Cyan),
        )));
    }
    let body = std::mem::take(&mut lines);
    if host {
        lines.push(Line::from(Span::styled(
            format!(
                " Hosting as \"{}\" on port {} (change both in holdem.toml).",
                app.table().name,
                app.config.network.port
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
    let (rect, lines) = screen_box(area, 72, height, body, lines);
    frame.render_widget(boxed(lines, title_block(title, rect), rect), rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sound::Sound;
    use holdem::config::Config;

    #[test]
    fn every_explanation_reads_cleanly_for_every_value() {
        let mut app = App::new(Config::default(), Some(1), Sound::new(true));
        for flip in [false, true] {
            let r = &mut app.setup.rules;
            r.dead_button = flip;
            r.muck_losers = flip;
            r.antes = flip;
            r.straddle = flip;
            r.hands_per_level = if flip { 0 } else { 8 };
            r.hand_limit = if flip { 0 } else { 50 };
            r.rebuy_levels = if flip { 0 } else { 2 };
            app.setup.fill_with_bots = flip;
            app.setup.fluctuation = flip;
            app.setup.allow_win_chance = flip;
            for row in app.setup.rows() {
                let (label, _, about) = setup_row_text(&app, row);
                assert!(!about.contains("  "), "{label}: {about:?}");
            }
        }
    }
}
