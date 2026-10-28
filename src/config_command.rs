//! `holdem config write | show | check`, and naming the settings file in use.

use std::path::{Path, PathBuf};

use holdem::config::{self, Config};

use crate::cli::ConfigAction;

/// Where the settings came from, for people to read.
pub(crate) fn source_text(source: Option<&Path>) -> String {
    match source {
        Some(p) => p.display().to_string(),
        None => format!(
            "built-in defaults (no {} found; `holdem config write` writes one)",
            config::FILE_NAME
        ),
    }
}

/// The full path, so it is clear which file is meant.
pub(crate) fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

/// `holdem config ...`
pub(crate) fn run_config(action: &ConfigAction, chosen: Option<&Path>) -> Result<(), String> {
    match action {
        ConfigAction::Write { file, force } => {
            let path = file
                .as_deref()
                .or(chosen)
                .unwrap_or(Path::new(config::FILE_NAME));
            if path.exists() && !force {
                return Err(format!(
                    "{} already exists; add --force to replace it",
                    path.display()
                ));
            }
            // Replacing a file keeps its values, now with every setting explained.
            let text = if path.exists() {
                match Config::read(path) {
                    Ok(current) => config::render(&current),
                    Err(e) => {
                        eprintln!("Could not read the old file ({e}); writing the defaults.");
                        config::template()
                    }
                }
            } else {
                config::template()
            };
            std::fs::write(path, text)
                .map_err(|e| format!("could not write {}: {}", path.display(), e))?;
            println!("Wrote {}", absolute(path.to_path_buf()).display());
        }
        ConfigAction::Show => {
            let (config, source) = Config::load(chosen)?;
            println!(
                "# Settings from {}\n",
                source_text(source.map(absolute).as_deref())
            );
            print!("{}", config::render(&config));
        }
        ConfigAction::Check { file } => {
            let Some(path) = Config::locate(file.as_deref().or(chosen)) else {
                println!("{}", source_text(None));
                return Ok(());
            };
            let config = Config::read(&path)?;
            let changed = config::differences(&config);
            println!(
                "{} is fine: {}",
                absolute(path).display(),
                match changed.len() {
                    0 => "every setting is at its default".to_string(),
                    1 => format!("1 setting differs from the default: {}", changed[0]),
                    n => format!(
                        "{} settings differ from the defaults: {}",
                        n,
                        changed.join(", ")
                    ),
                }
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_write_force_keeps_the_values() {
        let dir = std::env::temp_dir().join(format!("holdem-write-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("holdem.toml");
        std::fs::write(&path, "[game]\nseats = 4\n").unwrap();
        let write = |force| {
            run_config(
                &ConfigAction::Write {
                    file: Some(path.clone()),
                    force,
                },
                None,
            )
        };
        assert!(write(false).is_err(), "an existing file needs --force");
        write(true).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("seats = 4"), "the value is kept");
        assert!(text.contains("# Default: 6"), "and explained");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
