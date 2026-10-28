use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use holdem::config::Config;

use super::*;
use crate::sound::Sound;

#[test]
fn choices_step_and_snap_to_the_list() {
    let c = Config::default().client.setup;
    assert_eq!(step_choice(&c.hand_limits, 0, 1), 10);
    assert_eq!(step_choice(&c.hand_limits, 0, -1), 200);
    assert_eq!(
        step_choice(&c.starting_chips, 1700, 1),
        1500,
        "unknown values snap to the nearest"
    );
}

#[test]
fn solo_setup_hides_the_bot_option() {
    let mut app = App::new(Config::default(), Some(1), Sound::new(true));
    assert!(!app.setup.rows().contains(&SetupRow::FillWithBots));
    app.setup.host = true;
    assert!(app.setup.rows().contains(&SetupRow::FillWithBots));
}

#[test]
fn the_name_is_typed_on_the_setup_screen() {
    let mut config = Config::default();
    config.client.name = "Mo".to_string();
    let mut app = App::new(config, Some(1), Sound::new(true));
    let press = |app: &mut App, code| app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    press(&mut app, KeyCode::Enter); // Play against bots
    assert_eq!(app.screen, Screen::Setup);
    assert_eq!(
        app.setup.rows()[app.setup.row],
        SetupRow::Seats,
        "starts below the name"
    );
    assert_eq!(app.join.name, "Mo", "the configured name");

    press(&mut app, KeyCode::Up);
    for c in "Qh?".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    assert_eq!(app.join.name, "MoQh?", "letters are typed, not commands");
    assert_eq!(app.screen, Screen::Setup);
    assert!(!app.show_help);

    for _ in 0..5 {
        press(&mut app, KeyCode::Backspace);
    }
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Setup, "no game without a name");
    assert_eq!(app.setup.row, 0);

    for c in "Ann".chars() {
        press(&mut app, KeyCode::Char(c));
    }
    press(&mut app, KeyCode::Down);
    press(&mut app, KeyCode::Char('q'));
    assert_eq!(app.screen, Screen::Menu, "Q goes back on the other rows");
    assert_eq!(app.join.name, "Ann", "the join screen uses the same name");
}

#[test]
fn w_saves_the_settings_and_a_second_w_has_nothing_to_do() {
    let dir = std::env::temp_dir().join(format!("holdem-app-save-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.join("holdem.toml");
    let mut app = App::new(Config::default(), Some(1), Sound::new(true));
    app.config_path = Some(path.clone());
    let press = |app: &mut App, code| app.on_key(KeyEvent::new(code, KeyModifiers::NONE));

    press(&mut app, KeyCode::Enter); // Play against bots, on the Seats row
    press(&mut app, KeyCode::Left);
    press(&mut app, KeyCode::Char('w'));
    assert!(
        app.toast_text()
            .is_some_and(|t| t.starts_with("Saved 1 setting to"))
    );
    let saved = Config::read(&path).unwrap();
    assert_eq!(saved.game.seats, Config::default().game.seats - 1);
    assert_eq!(saved.client.name, "", "the login name is not written down");

    press(&mut app, KeyCode::Char('w'));
    assert!(
        app.toast_text()
            .is_some_and(|t| t.starts_with("Already saved"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn speeds_come_from_the_config() {
    let mut config = Config::default();
    config.game.speeds.push(holdem::config::SpeedConfig {
        name: "Ludicrous".to_string(),
        ms: 40,
    });
    config.client.speed = "ludicrous".to_string();
    let mut app = App::new(config, Some(1), Sound::new(true));
    assert_eq!(app.speed().name, "Ludicrous");
    assert_eq!(app.speed().ms, 40);
    app.change_speed(1);
    assert_eq!(app.speed().name, "Slow", "the list wraps around");
    app.change_speed(-1);
    assert_eq!(app.speed().name, "Ludicrous");
}

#[test]
fn a_decision_is_answered_only_once() {
    let mut config = Config::default();
    config.game.seats = 2;
    let mut app = App::new(config, Some(5), Sound::new(true));
    app.setup.seats = 2;
    app.start_game();
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    // Wait for our turn, then press C twice as fast as possible.
    loop {
        assert!(std::time::Instant::now() < deadline, "never our turn");
        app.tick();
        if app.view().is_some_and(|v| v.legal.is_some()) {
            break;
        }
        if app
            .view()
            .is_some_and(|v| v.state == State::HandOver && !v.you_ready)
        {
            app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let turn = app.view().unwrap().turn;
    let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
    app.on_key(key);
    app.on_key(key);
    assert_eq!(app.answered, Some(turn));
    // Whatever happens next, our second press never acted on a later decision:
    // the next view that asks us again carries a different turn.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        app.tick();
        std::thread::sleep(Duration::from_millis(5));
        if let Some(v) = app.view()
            && v.legal.is_some()
            && v.turn != turn
        {
            let me = v.you.unwrap();
            assert!(
                v.seats[me].last_action.is_empty() || v.street == holdem::game::Street::PreFlop,
                "nothing was done for us on the new street: {:?}",
                v.seats[me].last_action
            );
            break;
        }
    }
}

#[test]
fn the_join_screen_starts_on_the_name() {
    let app = App::new(Config::default(), Some(1), Sound::new(true));
    assert_eq!(app.join.row, 0);
}

#[test]
fn the_hint_cannot_be_turned_on_where_it_is_not_allowed() {
    let mut config = Config::default();
    config.game.allow_win_chance = false;
    let mut app = App::new(config, Some(1), Sound::new(true));
    app.setup.allow_win_chance = false;
    app.start_game();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while app.view().is_none() {
        assert!(std::time::Instant::now() < deadline);
        app.tick();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(app.show_odds, "the wish is kept");
    assert!(!app.hints_on(), "but the table says no");
    app.settings_row = SettingsRow::ALL
        .iter()
        .position(|&r| r == SettingsRow::WinChance)
        .unwrap();
    app.change_setting(1);
    assert!(app.show_odds, "toggling is refused");
    assert!(
        app.toast_text()
            .is_some_and(|t| t.contains("does not allow"))
    );
}
