//! Drawing every screen. Each screen has its module (`menu`, `setup`, `join`,
//! `lobby`, `table`, `over`), and the windows that open on top (help, log,
//! settings, ...) are in `popups`. Shared pieces: `style` (colors, frames, key
//! reminders), `text` (widths and wrapping), `cards` and `log`.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use self::style::{framed, yellow};
use self::text::fit;
use crate::app::{App, Screen};

mod achievements;
mod avatar;
mod cards;
mod choose;
mod hands;
mod join;
mod lobby;
mod log;
mod menu;
mod over;
mod popups;
mod setup;
mod style;
mod symbol_picker;
mod table;
mod text;

#[cfg(test)]
mod render_tests;
#[cfg(test)]
mod screenshots;

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    match app.screen {
        Screen::Menu => menu::draw(frame, app, area),
        Screen::Setup => setup::draw(frame, app, area),
        Screen::Join => join::draw(frame, app, area),
        Screen::Choose => choose::draw(frame, app, area),
        Screen::Lobby => lobby::draw(frame, app, area),
        Screen::Table => table::draw(frame, app, area),
        Screen::Over => over::draw(frame, app, area),
        Screen::Achievements => achievements::draw(frame, app, area),
    }
    if app.show_settings {
        popups::settings(frame, app, area);
    }
    if app.info.is_some() {
        popups::info(frame, app, area);
    }
    if app.picker.is_some() {
        symbol_picker::draw(frame, app, area);
    }
    if app.show_help {
        popups::help(frame, app, area);
    }
    if let Some(selected) = app.game_menu {
        popups::game_menu(frame, app, selected, area);
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
            one_line(t, rect)
                .style(Style::new().fg(Color::Black).bg(Color::LightRed))
                .alignment(Alignment::Center),
            rect,
        );
    }
}

/// The frame of a screen, its title cut to fit a box `rect` wide.
fn title_block(text: &str, rect: Rect) -> Block<'static> {
    let room = (rect.width as usize).saturating_sub(4);
    framed(
        Line::from(Span::styled(format!(" {} ", fit(text, room)), yellow())),
        Color::Green,
    )
    .title_alignment(Alignment::Center)
}

// Text is only ever drawn through the next four functions, so it never runs
// over the edge of its box: lines are wrapped (or, on one-line bars, cut with
// an ellipsis). A test checks that nothing else in `ui` makes a `Paragraph`.

/// Text in a box drawn at `rect`: every line is wrapped to the inside width.
fn boxed(lines: Vec<Line<'static>>, block: Block<'static>, rect: Rect) -> Paragraph<'static> {
    let inside = rect.width.saturating_sub(2) as usize;
    Paragraph::new(text::wrap_lines(lines, inside)).block(block)
}

/// Text without a box, drawn at `rect`: every line is wrapped to its width.
fn plain(lines: Vec<Line<'static>>, rect: Rect) -> Paragraph<'static> {
    Paragraph::new(text::wrap_lines(lines, rect.width as usize))
}

/// A one-line bar at `rect`, cut with an ellipsis if it is too long.
fn one_line(text: &str, rect: Rect) -> Paragraph<'static> {
    Paragraph::new(fit(text, rect.width as usize))
}

/// A screen's box in the middle of `area`: `width` wide (at most the screen)
/// and at least `min_height` high, taller if its wrapped text needs it (up to
/// the screen). `body` goes at the top and `footer` at the bottom, with blank
/// lines between. If even the screen is too small, the body gives way: the
/// footer holds the keys. Returns where to draw and the lines.
fn screen_box(
    area: Rect,
    width: u16,
    min_height: u16,
    body: Vec<Line<'static>>,
    footer: Vec<Line<'static>>,
) -> (Rect, Vec<Line<'static>>) {
    let width = width.min(area.width);
    let inside = width.saturating_sub(2) as usize;
    let footer = text::wrap_lines(footer, inside);
    let mut lines = text::wrap_lines(body, inside);
    let needed = (lines.len() + footer.len() + 2) as u16;
    let height = needed.max(min_height).min(area.height);
    let room = (height as usize).saturating_sub(2);
    lines.truncate(room.saturating_sub(footer.len()));
    while lines.len() + footer.len() < room {
        lines.push(Line::from(""));
    }
    lines.extend(footer);
    (style::centered(area, width, height), lines)
}

/// Where to draw a box `width` wide (at most the screen) and as high as its
/// `lines` once wrapped (at most the screen), in the middle of `area`; and the
/// wrapped lines.
fn fitted(area: Rect, width: u16, lines: Vec<Line<'static>>) -> (Rect, Vec<Line<'static>>) {
    let width = width.min(area.width);
    let lines = text::wrap_lines(lines, width.saturating_sub(2) as usize);
    let height = (lines.len() as u16).saturating_add(2).min(area.height);
    (style::centered(area, width, height), lines)
}
