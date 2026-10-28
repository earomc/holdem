//! Drawing every screen. Each screen has its module (`menu`, `setup`, `join`,
//! `lobby`, `table`, `over`), and the windows that open on top (help, log,
//! settings, ...) are in `popups`. Shared pieces: `style` (colours, frames, key
//! reminders), `text` (widths and wrapping), `cards` and `log`.

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use self::style::{framed, yellow};
use self::text::fit;
use crate::app::{App, Screen};

mod cards;
mod choose;
mod join;
mod lobby;
mod log;
mod menu;
mod over;
mod popups;
mod setup;
mod style;
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
