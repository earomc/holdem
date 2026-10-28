//! Computer opponents.
//!
//! Every bot estimates its winning chances with a Monte Carlo simulation against
//! random opposing hands, converts that into a table-size-independent strength
//! rating, and then decides what to do according to its personality: how loose
//! or tight it plays, how aggressive it is, and how often it bluffs. Bots only
//! ever look at information a real player at the table could see.
//!
//! This module has the styles, names and rebuy choices; `decide` has the
//! simulation and the decisions.

use crate::card::Rng;
use crate::stats::Features;

mod decide;

pub use decide::{decide, equity};

/// How a bot plays. All values are between 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    /// Archetype name shown in the interface.
    pub style: &'static str,
    /// Higher means more hands are played.
    pub looseness: f32,
    /// Higher means more betting and raising instead of checking and calling.
    pub aggression: f32,
    /// How often weak hands are bet as bluffs.
    pub bluff: f32,
}

impl Default for Personality {
    fn default() -> Personality {
        Personality {
            style: "Balanced",
            looseness: 0.5,
            aggression: 0.5,
            bluff: 0.1,
        }
    }
}

/// A playing style: how a bot with it plays, what it is like, and the
/// statistics it really produces.
#[derive(Clone, Copy, Debug)]
pub struct Archetype {
    pub personality: Personality,
    /// What the style is like, for the help.
    pub description: &'static str,
    /// Its typical statistics, measured over 400 bot-only games (about 110,000
    /// counted hands) with `cargo run --release --example calibrate`. The
    /// style estimate compares a player's numbers with these.
    pub measured: Features,
}

const fn archetype(
    style: &'static str,
    (looseness, aggression, bluff): (f32, f32, f32),
    description: &'static str,
    (vpip, pfr, aggressive, bluffs): (f32, f32, f32, f32),
) -> Archetype {
    Archetype {
        personality: Personality {
            style,
            looseness,
            aggression,
            bluff,
        },
        description,
        measured: Features {
            vpip,
            pfr,
            aggression: aggressive,
            bluff: bluffs,
        },
    }
}

/// Eight distinct playing styles; a table never has two bots with the same
/// one. Each: name, (looseness, aggression, bluff), description, and the
/// measured (VPIP, PFR, aggression, bluff share).
pub const ARCHETYPES: [Archetype; 8] = [
    archetype(
        "Rock",
        (0.15, 0.35, 0.02),
        "tight and cautious: few hands, mostly called rather than raised",
        (0.134, 0.021, 0.246, 0.032),
    ),
    archetype(
        "Nit",
        (0.05, 0.50, 0.00),
        "plays almost nothing but premium hands",
        (0.073, 0.019, 0.372, 0.004),
    ),
    archetype(
        "Tight-aggressive",
        (0.35, 0.65, 0.08),
        "few hands, played forcefully",
        (0.280, 0.116, 0.444, 0.110),
    ),
    archetype(
        "Shark",
        (0.50, 0.60, 0.12),
        "solid and balanced, the hardest to read",
        (0.387, 0.139, 0.403, 0.145),
    ),
    archetype(
        "Loose-aggressive",
        (0.65, 0.80, 0.18),
        "many hands, lots of raising",
        (0.503, 0.279, 0.556, 0.187),
    ),
    archetype(
        "Gambler",
        (0.70, 0.55, 0.22),
        "plays many hands and bluffs often",
        (0.527, 0.161, 0.390, 0.226),
    ),
    archetype(
        "Calling station",
        (0.80, 0.15, 0.02),
        "calls a lot, rarely raises, hard to bluff",
        (0.576, 0.019, 0.101, 0.061),
    ),
    archetype(
        "Maniac",
        (0.85, 0.95, 0.30),
        "raises and bluffs constantly",
        (0.656, 0.434, 0.650, 0.211),
    ),
];

/// Bot names used when the config file does not list any.
pub const DEFAULT_NAMES: [&str; 16] = [
    "Ava", "Ben", "Chloe", "Dmitri", "Elena", "Frank", "Gina", "Hiro", "Ivan", "Jade", "Kofi",
    "Lena", "Mateo", "Nora", "Omar", "Priya",
];

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        let j = rng.below(i as u64 + 1) as usize;
        items.swap(i, j);
    }
}

/// Picks `count` bots with the default names: unique names and, for up to
/// eight bots, unique playing styles.
pub fn roster(count: usize, rng: &mut Rng) -> Vec<(String, Personality)> {
    let names: Vec<String> = DEFAULT_NAMES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();
    roster_from(&names, count, rng)
}

/// Like [`roster`], but with names from the given list. If there are fewer
/// names than bots, numbers are added ("Ava 2").
pub fn roster_from(names: &[String], count: usize, rng: &mut Rng) -> Vec<(String, Personality)> {
    let mut styles: Vec<Personality> = ARCHETYPES.iter().map(|a| a.personality).collect();
    shuffle(&mut styles, rng);
    let mut taken: Vec<String> = Vec::new();
    (0..count)
        .map(|i| {
            let name = unused_name(names, &taken, rng);
            taken.push(name.clone());
            (name, styles[i % styles.len()])
        })
        .collect()
}

/// Whether someone in `taken` already goes by `name`. Case does not count:
/// "ava" and "Ava" look like the same player.
pub fn name_taken(taken: &[String], name: &str) -> bool {
    taken
        .iter()
        .any(|t| t.to_lowercase() == name.to_lowercase())
}

/// A random name from `names` that nobody in `taken` uses yet.
pub fn unused_name(names: &[String], taken: &[String], rng: &mut Rng) -> String {
    let free: Vec<&String> = names.iter().filter(|n| !name_taken(taken, n)).collect();
    if !free.is_empty() {
        return free[rng.below(free.len() as u64) as usize].clone();
    }
    let base = if names.is_empty() {
        "Bot".to_string()
    } else {
        names[rng.below(names.len() as u64) as usize].clone()
    };
    (2..)
        .map(|k| format!("{base} {k}"))
        .find(|candidate| !name_taken(taken, candidate))
        .expect("an unused number exists")
}

/// A new bot for an empty seat: an unused name and a random style.
pub fn newcomer(names: &[String], taken: &[String], rng: &mut Rng) -> (String, Personality) {
    let name = unused_name(names, taken, rng);
    let style = ARCHETYPES[rng.below(ARCHETYPES.len() as u64) as usize].personality;
    (name, style)
}

/// Whether a busted bot buys back in. Loose players rebuy more often.
pub fn wants_rebuy(personality: &Personality, rng: &mut Rng) -> bool {
    rng.chance(0.35 + 0.6 * personality.looseness)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_has_unique_names_and_styles() {
        let mut rng = Rng::seeded(5);
        let r = roster(8, &mut rng);
        assert_eq!(r.len(), 8);
        let mut names: Vec<&String> = r.iter().map(|x| &x.0).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), 8);
        let mut styles: Vec<&str> = r.iter().map(|x| x.1.style).collect();
        styles.sort_unstable();
        styles.dedup();
        assert_eq!(styles.len(), 8);
        assert!(!names.iter().any(|n| n.as_str() == "You"));
    }

    #[test]
    fn names_run_out_gracefully() {
        let mut rng = Rng::seeded(8);
        let names = vec!["Zed".to_string()];
        let r = roster_from(&names, 3, &mut rng);
        let got: Vec<&str> = r.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(got[0], "Zed");
        assert!(got[1].starts_with("Zed ") && got[2].starts_with("Zed "));
        assert_ne!(got[1], got[2]);
        assert_eq!(unused_name(&[], &[], &mut rng), "Bot 2");
        let taken = vec!["Zed".to_string()];
        let (name, _) = newcomer(&names, &taken, &mut rng);
        assert_ne!(name, "Zed");
    }

    #[test]
    fn names_that_differ_only_in_case_are_taken() {
        let mut rng = Rng::seeded(3);
        let names = vec!["Ava".to_string(), "Ben".to_string()];
        let taken = vec!["ava".to_string()];
        for _ in 0..20 {
            assert_eq!(unused_name(&names, &taken, &mut rng), "Ben");
        }
        assert!(name_taken(&taken, "AVA"));
    }
}
