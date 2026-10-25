//! Windows drawn on top of the screen: help, the raise box, the hand log,
//! settings, your playing style, and the leave dialog.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use holdem::game::chips_text;
use holdem::protocol::TableView;
use holdem::stats;

use crate::app::{App, LeaveChoice, RaisePrompt, SettingsRow, pot_raise_to};
use crate::help::{self, Item};
use crate::keys::{self, Cmd, Place};
use crate::session::Kind;
use crate::ui::{
    centered, cyan, dim, fit, key_for, log_view, reminder, reminder_spans, text_width, wrap_text,
    yellow,
};

fn window(title: String, color: Color) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(title)
}

// ------------------------------------------------------------------ leave

pub fn leave(frame: &mut Frame, app: &App, selected: usize, area: Rect) {
    let rect = centered(area, 60, 10);
    frame.render_widget(Clear, rect);
    let warning = match app.session.as_ref() {
        Some(s) if s.kind == Kind::Host => {
            "You are hosting: everyone at your table will be disconnected."
        }
        Some(s) if s.kind == Kind::Guest => "You give up your seat at this table.",
        Some(_) => "This game ends.",
        None => "",
    };
    let mut lines = vec![Line::from(Span::styled(format!(" {}", warning), dim()))];
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
    frame.render_widget(
        Paragraph::new(lines).block(window(" Leave? ".to_string(), Color::Red)),
        rect,
    );
}

// ------------------------------------------------------------------ settings

pub fn settings(frame: &mut Frame, app: &App, area: Rect) {
    let rect = centered(area, 58, 11);
    frame.render_widget(Clear, rect);
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
            format!(" ▶ {:<17}◀ {} ▶", label, value)
        } else {
            format!("   {:<17}  {}", label, value)
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
    frame.render_widget(
        Paragraph::new(lines).block(window(" Settings ".to_string(), Color::Cyan)),
        rect,
    );
}

// ------------------------------------------------------------------ log

pub fn log(frame: &mut Frame, app: &App, area: Rect) {
    let Some(session) = app.session.as_ref() else {
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
    let (lines, max) = log_view(&session.log, inner_h, inner_w, app.log_scroll);
    app.log_max.set(max);
    let scroll = app.log_scroll.min(max);
    let keys = keys::reminder_text(Place::LogWindow);
    let title = if scroll > 0 {
        format!(" Hand log, {} lines up  ({}) ", scroll, keys)
    } else {
        format!(" Hand log  ({}) ", keys)
    };
    frame.render_widget(
        Paragraph::new(lines).block(window(fit(&title, inner_w), Color::Cyan)),
        rect,
    );
}

// ------------------------------------------------------------------ profile

fn bar(share: f32, width: usize) -> String {
    let filled = (share.clamp(0.0, 1.0) * width as f32).round() as usize;
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

pub fn profile(frame: &mut Frame, app: &App, area: Rect) {
    let rect = centered(area, 74, 20);
    frame.render_widget(Clear, rect);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let stats = app.view().map(|v| v.your_stats).unwrap_or_default();
    let facts = app.help_facts();
    let (min_hands, min_stack) = (facts.style_min_hands, facts.style_min_stack_bb);
    let mut lines: Vec<Line<'static>> = vec![Line::from("")];
    let counted = format!(
        " Hands counted: {}  (only hands you started with at least {} big blinds:",
        stats.hands, min_stack
    );
    lines.push(Line::from(fit(&counted, inner_w)));
    lines.push(Line::from(Span::styled(
        "  with fewer, everybody has to play push-or-fold whatever their style)",
        dim(),
    )));
    lines.push(Line::from(""));
    match stats::profile(&stats, min_hands) {
        None => {
            lines.push(Line::from(format!(
                " Play at least {} counted hands for an estimate.",
                min_hands
            )));
        }
        Some(p) => {
            let x = p.features;
            let row = |label: &str, value: f32, about: &str| {
                Line::from(vec![
                    Span::raw(format!(" {:<12}", label)),
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
                    Span::raw(format!("   {:<18}", name)),
                    Span::styled(bar(*score as f32 / 100.0, 20), yellow()),
                    Span::raw(format!(" {}% match", score)),
                ]));
            }
        }
    }
    while lines.len() < 17 {
        lines.push(Line::from(""));
    }
    let mut spans = vec![Span::raw(
        " The styles are explained on the last help page.  ",
    )];
    spans.extend(reminder(&[], Place::ProfileWindow));
    lines.push(Line::from(spans));
    frame.render_widget(
        Paragraph::new(lines).block(window(" Your playing style ".to_string(), Color::Cyan)),
        rect,
    );
}

// ------------------------------------------------------------------ raise box

pub fn raise(frame: &mut Frame, v: &TableView, prompt: &RaisePrompt, area: Rect) {
    let Some(legal) = v.legal else {
        return;
    };
    let word = if v.current_bet == 0 {
        "Bet"
    } else {
        "Raise to"
    };
    let rect = centered(area, 60, 9);
    frame.render_widget(Clear, rect);

    let error = match &prompt.error {
        Some(e) => Line::from(Span::styled(e.clone(), Style::new().fg(Color::LightRed))),
        None => Line::from(""),
    };
    let lines = vec![
        Line::from(vec![
            Span::raw(format!("{}: ", word)),
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
    frame.render_widget(
        Paragraph::new(lines).block(window(format!(" {} ", word), Color::Yellow)),
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
                    out.push(Line::from(format!(" {}", piece)));
                }
            }
            Item::Pair(label, desc) => {
                let pieces = wrap_text(desc, desc_w, 0);
                let label_w = text_width(label);
                let pad = " ".repeat(LABEL_COL);
                if label_w + 3 > LABEL_COL {
                    out.push(Line::from(Span::styled(format!("  {}", label), cyan())));
                    for piece in pieces {
                        out.push(Line::from(format!("{}{}", pad, piece)));
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
                            out.push(Line::from(format!("{}{}", pad, piece)));
                        }
                    }
                }
            }
        }
    }
    out
}

pub fn help(frame: &mut Frame, app: &App, area: Rect) {
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
    let mut block = window(title, Color::Cyan);
    if max_scroll > 0 {
        let hint = if scroll < max_scroll {
            format!(" ↓ more below (line {} of {}) ", scroll + visible, total)
        } else {
            " end of page  (↑ scrolls back) ".to_string()
        };
        block = block.title_bottom(Line::from(hint).alignment(Alignment::Right));
    }
    frame.render_widget(Paragraph::new(shown).block(block), rect);
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
