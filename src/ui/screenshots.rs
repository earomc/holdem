//! The screenshots in `docs/screenshots/`, drawn by the game itself:
//! `cargo test -p holdem screenshots -- --ignored` redraws them. Each is the
//! screen in a test terminal, written as an SVG with the terminal's colours.

use std::fmt::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier};

use holdem::config::Config;
use holdem::game::{State, Street};

use super::draw;
use crate::app::App;
use crate::sound::Sound;

const WIDTH: u16 = 100;
const HEIGHT: u16 = 30;
/// Size of one character cell in the picture, in pixels.
const CELL_W: f32 = 9.0;
const CELL_H: f32 = 19.0;
const FOREGROUND: &str = "#d4d4d4";
const BACKGROUND: &str = "#1e1e1e";

/// A colour as a common dark terminal theme shows it.
fn hex(color: Color, default: &str) -> String {
    let named = match color {
        Color::Reset => return default.to_string(),
        Color::Black => "#000000",
        Color::Red => "#cd3131",
        Color::Green => "#0dbc79",
        Color::Yellow => "#e5e510",
        Color::Blue => "#2472c8",
        Color::Magenta => "#bc3fbc",
        Color::Cyan => "#11a8cd",
        Color::Gray => "#c0c0c0",
        Color::DarkGray => "#666666",
        Color::LightRed => "#f14c4c",
        Color::LightGreen => "#23d18b",
        Color::LightYellow => "#f5f543",
        Color::LightBlue => "#3b8eea",
        Color::LightMagenta => "#d670d6",
        Color::LightCyan => "#29b8db",
        Color::White => "#e5e5e5",
        Color::Rgb(r, g, b) => return format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Indexed(i) => return format!("#{i:02x}{i:02x}{i:02x}"),
    };
    named.to_string()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The terminal's contents as an SVG picture.
fn svg(buffer: &Buffer) -> String {
    let area = buffer.area;
    let (w, h) = (
        f32::from(area.width) * CELL_W,
        f32::from(area.height) * CELL_H,
    );
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n\
         <rect width=\"100%\" height=\"100%\" rx=\"6\" fill=\"{BACKGROUND}\"/>\n\
         <g font-family=\"'Cascadia Mono', 'DejaVu Sans Mono', Menlo, Consolas, monospace\" font-size=\"15\" xml:space=\"preserve\">\n"
    );
    for y in 0..area.height {
        let top = f32::from(y) * CELL_H;
        let baseline = top + CELL_H * 0.75;
        for x in 0..area.width {
            let cell = &buffer[(x, y)];
            let reversed = cell.modifier.contains(Modifier::REVERSED);
            let (fg, bg) = if reversed {
                (cell.bg, cell.fg)
            } else {
                (cell.fg, cell.bg)
            };
            let left = f32::from(x) * CELL_W;
            if bg != Color::Reset || reversed {
                let fill = hex(bg, FOREGROUND);
                let _ = writeln!(
                    out,
                    "<rect x=\"{left}\" y=\"{top}\" width=\"{CELL_W}\" height=\"{CELL_H}\" fill=\"{fill}\"/>"
                );
            }
            let symbol = cell.symbol();
            if symbol.trim().is_empty() {
                continue;
            }
            let mut style = format!(
                "fill=\"{}\"",
                hex(fg, if reversed { BACKGROUND } else { FOREGROUND })
            );
            if cell.modifier.contains(Modifier::BOLD) {
                style.push_str(" font-weight=\"bold\"");
            }
            if cell.modifier.contains(Modifier::DIM) {
                style.push_str(" opacity=\"0.6\"");
            }
            let _ = writeln!(
                out,
                "<text x=\"{left}\" y=\"{baseline}\" {style}>{}</text>",
                escape(symbol)
            );
        }
    }
    out.push_str("</g>\n</svg>\n");
    out
}

fn picture(app: &App) -> String {
    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    terminal.draw(|f| draw(f, app)).unwrap();
    svg(terminal.backend().buffer())
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn screenshots_folder() -> PathBuf {
    let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join("docs"))
        .find(|d| d.is_dir())
        .expect("the docs folder is in the repository");
    let dir = docs.join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
#[ignore]
fn screenshots() {
    let mut config = Config::default();
    config.client.name = "You".to_string();
    // The usual speed's name on screen, but quick bots.
    for speed in &mut config.game.speeds {
        speed.ms = 20;
    }
    let mut app = App::new(config, Some(7), Sound::silent());
    let dir = screenshots_folder();

    press(&mut app, KeyCode::Enter); // Play against bots
    std::fs::write(dir.join("setup.svg"), picture(&app)).unwrap();

    // Deal, then play cautiously until it is your turn after the flop.
    press(&mut app, KeyCode::Enter);
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        assert!(Instant::now() < deadline, "no turn after the flop came up");
        app.tick();
        if let Some(v) = app.view() {
            if let Some(l) = v.legal {
                if v.street != Street::PreFlop {
                    break;
                }
                // Call cheap bets to see a flop; fold to big ones.
                let key = if l.can_check || l.to_call <= l.stack / 10 {
                    'c'
                } else {
                    'f'
                };
                press(&mut app, KeyCode::Char(key));
            } else if v.state == State::HandOver && !v.you_ready {
                press(&mut app, KeyCode::Enter);
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::write(dir.join("table.svg"), picture(&app)).unwrap();
}
