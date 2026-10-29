//! The achievements screen: every achievement, its stages, what comes next
//! and the symbol shapes it unlocks.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use holdem::player::achievements::{ACHIEVEMENTS, Achievement, Progress, roman};
use holdem::player::symbol::GLYPHS;

use super::style::{centered, dim, green, reminder, yellow};
use super::text::wrap_text;
use super::{boxed, title_block};
use crate::app::App;
use crate::keys::Place;

/// Columns for the stage marks ("I II III") and the name.
const MARKS: usize = 9;
const NAME: usize = 17;

/// "I II III" with the reached stages in yellow, or ✔ / · for a single stage.
fn marks(a: &Achievement, level: usize) -> Vec<Span<'static>> {
    let reached = yellow().add_modifier(Modifier::BOLD);
    if a.stages.len() == 1 {
        let (mark, style) = if level == 1 {
            ("✔", reached)
        } else {
            ("·", dim())
        };
        return vec![Span::styled(mark, style), Span::raw(" ".repeat(MARKS - 1))];
    }
    let mut spans = Vec::new();
    let mut width = 0;
    for i in 0..a.stages.len() {
        let text = roman(i + 1);
        spans.push(Span::styled(text, if i < level { reached } else { dim() }));
        spans.push(Span::raw(" "));
        width += text.len() + 1;
    }
    spans.push(Span::raw(" ".repeat(MARKS.saturating_sub(width))));
    spans
}

/// What comes next for `a`, or that it is done; with what it unlocks.
fn next_text(a: &Achievement, p: &Progress, level: usize) -> (String, Style) {
    let Some(next) = a.stages.get(level) else {
        let last = &a.stages[a.stages.len() - 1];
        return (format!("{}: done", last.text), green());
    };
    let mut text = next.text.to_string();
    if let Some(progress) = a.progress_text(p) {
        text.push_str(&format!(" ({progress})"));
    }
    if let Some(g) = next.unlocks {
        text.push_str(&format!(", unlocks {}", GLYPHS[usize::from(g)]));
    }
    (text, Style::new())
}

pub(super) fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let p = app.total_progress();
    let width = 92u16.min(area.width);
    let height = area.height.saturating_sub(2).max(3);
    let rect = centered(area, width, height);
    let inner_w = rect.width.saturating_sub(2) as usize;
    let stages: usize = ACHIEVEMENTS.iter().map(|a| a.stages.len()).sum();
    let earned: usize = ACHIEVEMENTS.iter().map(|a| a.level(&p)).sum();

    let mut head = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!(
                " {earned} of {stages} stages earned, as {}",
                app.stats_name()
            ),
            yellow(),
        )),
        Line::from(""),
    ];
    if app.stats_file.is_none() {
        head.push(Line::from(Span::styled(
            " Not kept: there is no settings folder to keep them in.",
            dim(),
        )));
        head.push(Line::from(""));
    }

    let mut body: Vec<Line<'static>> = Vec::new();
    let text_w = inner_w.saturating_sub(1 + MARKS + NAME).max(10);
    for a in &ACHIEVEMENTS {
        let level = a.level(&p);
        let (text, style) = next_text(a, &p, level);
        for (k, piece) in wrap_text(&text, text_w, 0).into_iter().enumerate() {
            let mut spans = vec![Span::raw(" ")];
            if k == 0 {
                spans.extend(marks(a, level));
                spans.push(Span::raw(format!("{:<NAME$}", a.name)));
            } else {
                spans.push(Span::raw(" ".repeat(MARKS + NAME)));
            }
            spans.push(Span::styled(piece, style));
            body.push(Line::from(spans));
        }
    }

    // Scroll the list when it does not fit.
    let footer = 2;
    let room = (height as usize)
        .saturating_sub(2 + head.len() + footer)
        .max(1);
    let max = body.len().saturating_sub(room);
    app.achievements_max.set(max);
    let scroll = app.achievements_scroll.min(max);
    let mut lines = head;
    lines.extend(body.into_iter().skip(scroll).take(room));
    while lines.len() < (height as usize).saturating_sub(2 + footer) {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(reminder(&[], Place::Achievements)));
    frame.render_widget(boxed(lines, title_block("Achievements", rect), rect), rect);
}
