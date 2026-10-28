//! Windows drawn on top of the screen: help, the raise box, game info (the
//! table log, your hands and your style), settings, and the leave dialog.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear};

use holdem::game::chips_text;
use holdem::protocol::TableView;
use holdem::stats;

use super::hands::{self, hands_view};
use super::log::log_view;
use super::over::{rough_note, style_line};
use super::style::{bold, centered, cyan, dim, key_for, reminder, reminder_spans, yellow};
use super::text::{fit, text_width, wrap_text};
use super::{boxed, fitted};
use crate::app::{App, InfoTab, LeaveChoice, RaisePrompt, SettingsRow, pot_raise_to};
use crate::help::{self, Item};
use crate::keys::{self, Cmd, Place};
use crate::session::Kind;

fn window(title: String, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(title)
}

// ------------------------------------------------------------------ leave

pub(super) fn leave(frame: &mut Frame, app: &App, selected: usize, area: Rect) {
    let warning = match app.session.as_ref() {
        Some(s) if s.kind == Kind::Host => {
            "You are hosting: everyone at your table will be disconnected."
        }
        Some(s) if s.kind == Kind::Guest => "You give up your seat at this table.",
        Some(_) => "This game ends.",
        None => "",
    };
    let mut lines = vec![Line::from(Span::styled(format!(" {warning}"), dim()))];
    lines.push(Line::from(""));
    for (i, option) in LeaveChoice::ALL.iter().enumerate() {
        let style = if i == selected {
            yellow()
        } else {
            Style::new()
        };
        let marker = if i == selected { " ▶ " } else { "   " };
        lines.push(Line::from(Span::styled(
            format!("{}{}", marker, option.label()),
            style,
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::LeaveWindow)));
    let (rect, lines) = fitted(area, 60, lines);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        boxed(lines, window(" Leave? ".to_string(), Color::Red), rect),
        rect,
    );
}

// ------------------------------------------------------------------ settings

pub(super) fn settings(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![Line::from("")];
    for (i, row) in SettingsRow::ALL.iter().enumerate() {
        let label = row.label();
        let value = match row {
            SettingsRow::Speed => format!("{} ({} ms per move)", app.speed().name, app.speed().ms),
            SettingsRow::Volume => app.volume_label(),
            SettingsRow::WinChance if !app.hints_allowed() => {
                "Not allowed at this table".to_string()
            }
            SettingsRow::WinChance => if app.show_odds { "On" } else { "Off" }.to_string(),
        };
        let text = if i == app.settings_row {
            format!(" ▶ {label:<17}◀ {value} ▶")
        } else {
            format!("   {label:<17}  {value}")
        };
        let style = if i == app.settings_row {
            yellow()
        } else {
            Style::new()
        };
        lines.push(Line::from(Span::styled(text, style)));
    }
    lines.push(Line::from(""));
    let guest = app.session.as_ref().is_some_and(|s| s.kind == Kind::Guest);
    let note = if guest {
        " The table's owner decides the game speed."
    } else {
        " Speeds and their names are set in holdem.toml."
    };
    lines.push(Line::from(Span::styled(note, dim())));
    lines.push(Line::from(reminder(&[], Place::SettingsWindow)));
    let (rect, lines) = fitted(area, 58, lines);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        boxed(lines, window(" Settings ".to_string(), Color::Cyan), rect),
        rect,
    );
}

// ------------------------------------------------------------------ game info

/// The row of tabs at the top of the game info window, the open one marked.
fn tab_row(open: InfoTab) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, tab) in InfoTab::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" │ ", dim()));
        }
        if *tab == open {
            spans.push(Span::styled(
                format!("[{}]", tab.title()),
                yellow().add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(format!(" {} ", tab.title()), dim()));
        }
    }
    Line::from(spans)
}

/// The game info window: the table log, your hands and your style, as tabs.
pub(super) fn info(frame: &mut Frame, app: &App, area: Rect) {
    let (Some(tab), Some(session)) = (app.info, app.session.as_ref()) else {
        return;
    };
    let rect = centered(
        area,
        area.width.saturating_sub(6).min(100),
        area.height.saturating_sub(2),
    );
    frame.render_widget(Clear, rect);
    let inner_h = rect.height.saturating_sub(2) as usize;
    let inner_w = rect.width.saturating_sub(2) as usize;
    let mut lines = vec![tab_row(tab), Line::from("")];
    let body_h = inner_h.saturating_sub(lines.len());
    let max = match tab {
        InfoTab::TableLog => {
            let (body, max) = log_view(&session.log, body_h, inner_w, app.log_scroll);
            lines.extend(body);
            max
        }
        InfoTab::YourHands => {
            if !session.my_hands.is_empty() {
                lines.push(Line::from(Span::styled(
                    fit(&format!(" {}", hands::summary(&session.my_hands)), inner_w),
                    bold(),
                )));
                lines.push(Line::from(""));
            }
            let room = inner_h.saturating_sub(lines.len());
            let (body, max) = hands_view(&session.my_hands, room, inner_w, app.log_scroll);
            lines.extend(body);
            max
        }
        InfoTab::Style => {
            lines.extend(style_lines(app, inner_w));
            0
        }
    };
    app.log_max.set(max);
    let scroll = app.log_scroll.min(max);
    let keys = keys::reminder_text(Place::InfoWindow);
    let title = if scroll > 0 {
        format!(" Game info, {scroll} lines up  ({keys}) ")
    } else {
        format!(" Game info  ({keys}) ")
    };
    frame.render_widget(
        boxed(lines, window(fit(&title, inner_w), Color::Cyan), rect),
        rect,
    );
}

fn bar(share: f32, width: usize) -> String {
    let filled = (share.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

/// The Your style tab: this game's numbers, and the estimate over all games.
fn style_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let stats = app
        .session
        .as_ref()
        .map(|s| s.game_stats)
        .unwrap_or_default();
    let facts = app.help_facts();
    let (min_hands, min_stack) = (facts.style_min_hands, facts.style_min_stack_bb);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let counted = format!(
        " Hands counted this game: {}  (only hands you started with at least {} big blinds:",
        stats.hands, min_stack
    );
    lines.push(Line::from(counted));
    lines.push(Line::from(Span::styled(
        "  with fewer, everybody has to play push-or-fold whatever their style)",
        dim(),
    )));
    lines.push(Line::from(""));
    match stats::profile(&stats, min_hands) {
        None => {
            lines.push(Line::from(format!(
                " Play at least {} counted hands for a first guess.",
                stats::ROUGH_HANDS.min(min_hands)
            )));
        }
        Some(p) => {
            let x = p.features;
            let row = |label: &str, value: f32, about: &str| {
                Line::from(vec![
                    Span::raw(format!(" {label:<12}")),
                    Span::styled(bar(value, 16), cyan()),
                    Span::raw(format!(" {:>3.0}%  ", value * 100.0)),
                    Span::styled(about.to_string(), dim()),
                ])
            };
            lines.push(row(
                "Hands played",
                x.vpip,
                "share of hands you put chips in",
            ));
            lines.push(row(
                "Raised first",
                x.pfr,
                "share of hands you raised pre-flop",
            ));
            lines.push(row("Aggression", x.aggression, "bets and raises vs calls"));
            lines.push(row("Bluffing", x.bluff, "post-flop bets without a pair"));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::raw(" In words: "),
                Span::styled(p.summary.clone(), yellow()),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(" Closest bot styles:"));
            for (name, score) in p.ranking.iter().take(3) {
                lines.push(Line::from(vec![
                    Span::raw(format!("   {name:<18}")),
                    Span::styled(bar(*score as f32 / 100.0, 20), yellow()),
                    Span::raw(format!(" {score}% match")),
                ]));
            }
            if p.rough {
                lines.push(Line::from(""));
                for l in wrap_text(
                    &format!(" {}", rough_note(stats.hands, min_hands)),
                    width.saturating_sub(1),
                    1,
                ) {
                    lines.push(Line::from(Span::styled(l, dim())));
                }
            }
        }
    }
    lines.push(Line::from(""));
    let all_games = match app.stats_file {
        Some(_) => {
            let saved = app.saved_stats();
            let mut all = saved.stats;
            let mut games = saved.games;
            let saved_already = app.session.as_ref().is_some_and(|s| s.stats_saved);
            if !saved_already && stats.hands > 0 {
                all.add(&stats);
                games += 1;
            }
            format!(
                " All games ({games} game{}, {} counted hands): {}",
                if games == 1 { "" } else { "s" },
                all.hands,
                style_line(&all, min_hands)
            )
        }
        None => " All games: not kept (there is no settings folder to keep them in).".to_string(),
    };
    for l in wrap_text(&all_games, width.saturating_sub(1), 3) {
        lines.push(Line::from(Span::styled(l, Style::new().fg(Color::Cyan))));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " The styles are explained on the last help page.",
        dim(),
    )));
    lines
}

// ------------------------------------------------------------------ raise box

pub(super) fn raise(frame: &mut Frame, v: &TableView, prompt: &RaisePrompt, area: Rect) {
    let Some(legal) = v.legal else {
        return;
    };
    let word = if v.current_bet == 0 {
        "Bet"
    } else {
        "Raise to"
    };
    let error = match &prompt.error {
        Some(e) => Line::from(Span::styled(e.clone(), Style::new().fg(Color::LightRed))),
        None => Line::from(""),
    };
    let lines = vec![
        Line::from(vec![
            Span::raw(format!("{word}: ")),
            Span::styled(format!("{}▏", prompt.text), yellow()),
        ]),
        Line::from(Span::styled(
            format!(
                "smallest {}   largest {} (all-in)",
                chips_text(legal.min_to),
                chips_text(legal.max_to)
            ),
            dim(),
        )),
        Line::from(Span::styled(
            format!(
                "half pot {}   pot {}",
                chips_text(pot_raise_to(v, 0.5)),
                chips_text(pot_raise_to(v, 1.0))
            ),
            dim(),
        )),
        Line::from(""),
        Line::from(reminder_spans(
            keys::reminder_where(Place::Raise, |c| !matches!(c, Cmd::Select | Cmd::Back))
                .into_iter(),
        )),
        error,
        Line::from(reminder_spans(
            keys::reminder_where(Place::Raise, |c| matches!(c, Cmd::Select | Cmd::Back))
                .into_iter(),
        )),
    ];
    let (rect, lines) = fitted(area, 60, lines);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        boxed(lines, window(format!(" {word} "), Color::Yellow), rect),
        rect,
    );
}

// ------------------------------------------------------------------ help

/// Width of the left column (label plus padding) for two-column help items.
const LABEL_COL: usize = 20;

fn help_lines(items: &[Item], width: usize) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    let desc_w = width.saturating_sub(LABEL_COL).max(12);
    for item in items {
        match item {
            Item::Heading(text) => {
                if !out.is_empty() {
                    out.push(Line::from(""));
                }
                out.push(Line::from(Span::styled(text.to_string(), yellow())));
            }
            Item::Text(text) => {
                for piece in wrap_text(text, width.saturating_sub(1), 0) {
                    out.push(Line::from(format!(" {piece}")));
                }
            }
            Item::Pair(label, desc) => {
                let pieces = wrap_text(desc, desc_w, 0);
                let label_w = text_width(label);
                let pad = " ".repeat(LABEL_COL);
                if label_w + 3 > LABEL_COL {
                    out.push(Line::from(Span::styled(format!("  {label}"), cyan())));
                    for piece in pieces {
                        out.push(Line::from(format!("{pad}{piece}")));
                    }
                } else {
                    let label_text = format!("  {}{}", label, " ".repeat(LABEL_COL - 2 - label_w));
                    for (i, piece) in pieces.into_iter().enumerate() {
                        if i == 0 {
                            out.push(Line::from(vec![
                                Span::styled(label_text.clone(), cyan()),
                                Span::raw(piece),
                            ]));
                        } else {
                            out.push(Line::from(format!("{pad}{piece}")));
                        }
                    }
                }
            }
        }
    }
    out
}

pub(super) fn help(frame: &mut Frame, app: &App, area: Rect) {
    let width = area.width.min(84);
    let text_w = (width.saturating_sub(3) as usize).max(30);
    let pages = help::pages(&app.help_facts());
    let page_index = app.help_page % pages.len();
    let page = &pages[page_index];
    let lines = help_lines(&page.items, text_w);
    let total = lines.len();

    let height = ((lines.len() + 2) as u16).min(area.height);
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);

    let visible = rect.height.saturating_sub(2) as usize;
    let max_scroll = lines.len().saturating_sub(visible);
    app.help_max.set(max_scroll);
    let scroll = app.help_scroll.min(max_scroll);
    let shown: Vec<Line<'static>> = lines.into_iter().skip(scroll).take(visible).collect();

    let title = format!(
        " Help {}/{}: {}  ({} page, {} close) ",
        page_index + 1,
        pages.len(),
        page.title,
        key_for(Place::HelpWindow, Cmd::Left),
        key_for(Place::HelpWindow, Cmd::Close),
    );
    let mut block = window(
        fit(&title, rect.width.saturating_sub(2) as usize),
        Color::Cyan,
    );
    if max_scroll > 0 {
        let hint = if scroll < max_scroll {
            format!(" ↓ more below (line {} of {}) ", scroll + visible, total)
        } else {
            " end of page  (↑ scrolls back) ".to_string()
        };
        block = block.title_bottom(Line::from(hint).alignment(Alignment::Right));
    }
    frame.render_widget(boxed(shown, block, rect), rect);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_pages_fit_the_narrowest_window() {
        // An 80 column terminal gives the help window 77 columns of text.
        for width in [77usize, 81] {
            for page in help::pages(&help::Facts {
                style_min_hands: 15,
                style_min_stack_bb: 15,
            }) {
                let lines = help_lines(&page.items, width);
                assert!(!lines.is_empty(), "{} is empty", page.title);
                for line in &lines {
                    assert!(
                        line.width() <= width,
                        "a line on the {:?} page is {} wide (limit {})",
                        page.title,
                        line.width(),
                        width
                    );
                }
            }
        }
    }
}
