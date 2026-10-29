//! holdem: no-limit Texas Hold'em in the terminal, against bots or other people.

mod app;
mod cli;
mod config_command;
mod help;
mod keys;
mod serve;
mod session;
mod sound;
mod ui;
mod uninstall_command;

use std::io;
use std::time::Duration;

use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use holdem::config::Config;

use app::App;
use cli::{Action, Options, cli};
use config_command::{absolute, run_config, source_text};
use serve::run_server;
use sound::Sound;
use uninstall_command::run_uninstall;

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if app.quit {
            return Ok(());
        }
        if event::poll(Duration::from_millis(40))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            app.on_key(key);
        }
        app.tick();
    }
}

fn fail(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {message}");
    std::process::exit(1);
}

fn main() {
    // clap prints help, the version and usage errors itself, and exits.
    let options = Options::from_matches(&cli().get_matches());
    let chosen = options.config.as_deref();
    let (config, source) = match &options.action {
        // Before reading the settings: a broken file must not stand in the way.
        Action::Uninstall { yes, purge } => {
            if let Err(e) = run_uninstall(*yes, *purge) {
                fail(e);
            }
            return;
        }
        Action::Config(action) => {
            if let Err(e) = run_config(action, chosen) {
                fail(e);
            }
            return;
        }
        _ => match Config::load(chosen) {
            Ok((c, source)) => (c, source.map(absolute)),
            Err(e) => fail(format!("in the settings file: {e}")),
        },
    };
    if let Action::Server(server) = &options.action {
        if let Err(e) = run_server(&config, source.as_deref(), server, options.seed) {
            fail(e);
        }
        return;
    }
    // Open the audio device before taking over the terminal: audio back ends may
    // print warnings, which would garble the screen later on.
    let sound = Sound::new(options.mute);
    let mut app = App::new(config, options.seed, sound);
    app.config_note = format!("Settings: {}", source_text(source.as_deref()));
    app.config_path = source;
    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    if let Err(e) = result {
        fail(e);
    }
}
