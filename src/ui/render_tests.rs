//! Drawing every screen at every size, from the outside.

use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use holdem::config::Config;
use holdem::game::State;

use super::draw;
use crate::app::{App, Screen};
use crate::sound::Sound;

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
    c.speeds.push(holdem::config::SpeedConfig {
        name: "Test".to_string(),
        ms: 20,
    });
    c.client.speed = "Test".to_string();
    c.network.port = 0; // any free port
    c.tables[0].next_hand_seconds = 1;
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
    config.tables[0].rules.starting_chips = 1000;
    config.tables[0].rules.antes = true;
    config.tables[0].rules.straddle = true;
    config.tables[0].rules.rebuy_levels = 1;
    config.tables[0].rules.dead_button = true;
    config.tables[0].rules.muck_losers = true;
    config.tables[0].rules.hand_limit = 6;
    config.tables[0].seats = 9;
    config.tables[0].fluctuation.enabled = true;
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

#[test]
fn the_choose_screen_renders_at_every_size() {
    let table = |name: &str| holdem::table::TableSettings {
        name: name.to_string(),
        ..holdem::table::TableSettings::default()
    };
    let (server, _log) = holdem::net::Server::start_tables(
        vec![table("Low stakes"), table("Turbo"), table("Heads-up")],
        Some(0),
        1,
    )
    .expect("starts");
    let mut app = App::new(fast_config(), Some(1), Sound::new(true));
    app.screen = Screen::Join;
    app.join.name = "Ann".to_string();
    app.join.address = format!("127.0.0.1:{}", server.port.unwrap());
    app.join.row = 1;
    press(&mut app, KeyCode::Enter);
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.screen != Screen::Choose {
        assert!(Instant::now() < deadline, "no list of tables");
        app.tick();
        std::thread::sleep(Duration::from_millis(10));
    }
    draw_everything(&mut app);
    let text = screen_text(&app, 80, 23);
    assert!(
        text.contains("Turbo") && text.contains("Choose a table"),
        "{text}"
    );
}

/// Text only reaches the screen through `boxed`, `plain`, `one_line` and
/// `fitted` in `ui/mod.rs`, which wrap or fit every line, so it never runs over
/// the edge of its box. A `Paragraph` made anywhere else could.
#[test]
fn text_is_only_drawn_through_the_wrapping_helpers() {
    let ui = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join("src").join("ui"))
        .find(|d| d.is_dir())
        .expect("src/ui is in the repository");
    let mut stack = vec![ui];
    let mut offenders = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            // Paths compare by component, so this also matches ui\mod.rs on Windows.
            let is_helpers = path.ends_with("ui/mod.rs");
            let text = std::fs::read_to_string(&path).unwrap();
            let needle = ["Paragraph", "::new("].concat();
            if !is_helpers && text.contains(&needle) {
                offenders.push(path.display().to_string());
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "draw text with boxed/plain/one_line instead: {offenders:?}"
    );
}
