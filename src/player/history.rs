//! Your playing statistics and your progress towards the achievements over
//! all your games, kept in `stats.toml` in your settings folder (next to
//! `holdem.toml`), one entry per player name.
//!
//! The file is only a convenience: when it is missing or unreadable, the
//! counts simply start from nothing.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::achievements::Progress;
use crate::stats::Stats;

pub const FILE_NAME: &str = "stats.toml";

/// One player's counts over all their games.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Record {
    pub games: u32,
    #[serde(flatten)]
    pub stats: Stats,
    /// Towards the achievements (missing from files of holdem 0.9).
    pub progress: Progress,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct File {
    players: BTreeMap<String, Record>,
}

/// `stats.toml` in your settings folder, or `None` if there is none.
pub fn file() -> Option<PathBuf> {
    let settings = crate::config::user_file()?;
    Some(settings.parent()?.join(FILE_NAME))
}

fn read(path: &Path) -> Result<File, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(File::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// The key under which `name` is kept. Case does not count, as at the table.
fn key<'a>(file: &'a File, name: &'a str) -> &'a str {
    file.players
        .keys()
        .find(|k| k.to_lowercase() == name.to_lowercase())
        .map_or(name, String::as_str)
}

/// `name`'s counts so far; nothing if the file is missing or broken.
pub fn load(path: &Path, name: &str) -> Record {
    let Ok(file) = read(path) else {
        return Record::default();
    };
    file.players
        .get(key(&file, name))
        .copied()
        .unwrap_or_default()
}

/// Adds one game to `name`'s counts. A broken file is left alone (and the
/// game not counted) rather than overwritten.
pub fn add_game(path: &Path, name: &str, game: &Stats, progress: &Progress) -> Result<(), String> {
    let mut file = read(path)?;
    let name = key(&file, name).to_string();
    let record = file.players.entry(name).or_default();
    record.games += 1;
    record.stats.add(game);
    record.progress = record.progress.plus(progress);
    let text = toml::to_string(&file).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("holdem-history-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(FILE_NAME)
    }

    #[test]
    fn games_add_up_under_one_name() {
        let path = scratch("add");
        assert_eq!(load(&path, "Mo"), Record::default(), "no file yet");
        let game = Stats {
            hands: 12,
            vpip: 4,
            bluffs: 1,
            ..Stats::default()
        };
        add_game(&path, "Mo", &game, &Progress::default()).unwrap();
        add_game(&path, "mo", &game, &Progress::default()).unwrap();
        add_game(&path, "Ann", &game, &Progress::default()).unwrap();
        let mo = load(&path, "MO");
        assert_eq!(mo.games, 2);
        assert_eq!((mo.stats.hands, mo.stats.vpip, mo.stats.bluffs), (24, 8, 2));
        assert_eq!(load(&path, "Ann").games, 1);
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            !text.contains("[players.mo]"),
            "kept under the first spelling:\n{text}"
        );
    }

    #[test]
    fn a_broken_file_reads_as_nothing_and_is_not_overwritten() {
        let path = scratch("broken");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is [not toml").unwrap();
        assert_eq!(load(&path, "Mo"), Record::default());
        assert!(add_game(&path, "Mo", &Stats::default(), &Progress::default()).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "this is [not toml");
    }

    #[test]
    fn progress_is_kept_and_old_files_still_read() {
        let path = scratch("progress");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A file of holdem 0.9: statistics, no progress.
        std::fs::write(
            &path,
            "[players.Mo]\ngames = 2\nhands = 30\nvpip = 9\npfr = 4\nbets = 12\ncalls = 8\n\
             postflop_bets = 6\nbluffs = 1\n",
        )
        .unwrap();
        let old = load(&path, "Mo");
        assert_eq!((old.games, old.stats.hands), (2, 30));
        assert_eq!(old.progress, Progress::default());
        let won = Progress {
            hands_won: 4,
            ..Progress::default()
        };
        add_game(&path, "Mo", &Stats::default(), &won).unwrap();
        add_game(&path, "Mo", &Stats::default(), &won).unwrap();
        let now = load(&path, "Mo");
        assert_eq!(
            (now.games, now.stats.hands, now.progress.hands_won),
            (4, 30, 8)
        );
    }
}
