//! Playing statistics, and an estimate of which bot archetype a player's style
//! is closest to.
//!
//! The engine counts a few standard poker statistics for every player. Each
//! archetype's typical values were measured by letting the bots play thousands
//! of hands (`bot::Archetype::measured`; `examples/calibrate.rs` re-measures
//! them), so a player is compared with what the bots actually do, not with a
//! guess.

use serde::{Deserialize, Serialize};

use crate::bot::ARCHETYPES;

/// Hands needed before the estimate is reliable (default; `client.style_min_hands`).
pub const MIN_HANDS: u32 = 15;
/// Hands needed for a first, rough estimate.
pub const ROUGH_HANDS: u32 = 5;
/// Only hands started with at least this many big blinds are counted: with a
/// short stack everybody has to play push-or-fold, whatever their style
/// (default; `game.style_min_stack_bb`).
pub const MIN_STACK_BB: u32 = 15;

/// Raw counts, updated by the engine as the player acts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    /// Hands dealt in (with at least [`MIN_STACK_BB`] big blinds).
    pub hands: u32,
    /// Hands in which the player voluntarily put chips in before the flop
    /// (a call or raise; posting a blind does not count).
    pub vpip: u32,
    /// Hands in which the player raised before the flop.
    pub pfr: u32,
    /// Bets and raises on any street.
    pub bets: u32,
    /// Calls on any street.
    pub calls: u32,
    /// Bets and raises after the flop.
    pub postflop_bets: u32,
    /// Post-flop bets and raises made without even a pair (bluffs and semi-bluffs).
    pub bluffs: u32,
}

/// The four numbers a style is judged by, each between 0 and 1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Features {
    /// Share of hands played voluntarily ("VPIP").
    pub vpip: f32,
    /// Share of hands raised before the flop ("PFR").
    pub pfr: f32,
    /// Share of bets and raises among all bets, raises and calls.
    pub aggression: f32,
    /// Share of post-flop bets and raises made without a pair.
    pub bluff: f32,
}

impl Stats {
    /// Adds another set of counts (another game) to these.
    pub fn add(&mut self, other: &Stats) {
        self.hands += other.hands;
        self.vpip += other.vpip;
        self.pfr += other.pfr;
        self.bets += other.bets;
        self.calls += other.calls;
        self.postflop_bets += other.postflop_bets;
        self.bluffs += other.bluffs;
    }

    /// Which of the four features have anything to go by yet, in the order of
    /// [`Features`]: without bets or calls there is no aggression to measure,
    /// and without bets after the flop no bluffing.
    pub fn known(&self) -> [bool; 4] {
        [
            self.hands > 0,
            self.hands > 0,
            self.bets + self.calls > 0,
            self.postflop_bets > 0,
        ]
    }

    pub fn features(&self) -> Features {
        let ratio = |a: u32, b: u32| if b == 0 { 0.0 } else { a as f32 / b as f32 };
        Features {
            vpip: ratio(self.vpip, self.hands),
            pfr: ratio(self.pfr, self.hands),
            aggression: ratio(self.bets, self.bets + self.calls),
            bluff: ratio(self.bluffs, self.postflop_bets),
        }
    }
}

const fn f(vpip: f32, pfr: f32, aggression: f32, bluff: f32) -> Features {
    Features {
        vpip,
        pfr,
        aggression,
        bluff,
    }
}

/// How much each feature counts when comparing styles. How many hands someone
/// plays and how often they raise say the most; bluffing is rare and noisy.
const WEIGHTS: Features = f(1.6, 1.2, 1.0, 0.6);

/// Every feature known.
const ALL_KNOWN: [bool; 4] = [true; 4];

/// How far apart `a` and `b` are, over the features `known` (the others do not count).
fn distance(a: &Features, b: &Features, known: [bool; 4]) -> f32 {
    let d = |x: f32, y: f32, w: f32| (w * (x - y)).powi(2);
    let parts = [
        d(a.vpip, b.vpip, WEIGHTS.vpip),
        d(a.pfr, b.pfr, WEIGHTS.pfr),
        d(a.aggression, b.aggression, WEIGHTS.aggression),
        d(a.bluff, b.bluff, WEIGHTS.bluff),
    ];
    parts
        .iter()
        .zip(known)
        .filter(|(_, k)| *k)
        .map(|(p, _)| p)
        .sum::<f32>()
        .sqrt()
}

/// How quickly the match falls off with distance. Neighboring styles are
/// about 0.1 to 0.15 apart, so a player between two of them shares the match.
const SPREAD: f32 = 0.12;

/// An estimate of a player's style.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub features: Features,
    /// Closest archetype first, each with its share of the match in percent:
    /// how much the play resembles it compared with the others (they add up
    /// to about 100). A player is usually a mix of styles.
    pub ranking: Vec<(&'static str, u32)>,
    /// Which features have anything to go by yet (see [`Stats::known`]).
    pub known: [bool; 4],
    /// "Tight" / "Loose" etc. in plain words.
    pub summary: String,
    /// Based on fewer hands than make a reliable estimate.
    pub rough: bool,
}

/// The archetype whose measured statistics are nearest to `features`.
pub fn closest(features: &Features) -> &'static str {
    rank(features, ALL_KNOWN)[0].0
}

/// Every archetype, closest first, with its share of the match.
fn rank(features: &Features, known: [bool; 4]) -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, f32)> = ARCHETYPES
        .iter()
        .map(|a| (a.personality.style, distance(features, &a.measured, known)))
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    // Weights fall off with distance; measured from the closest style, so that
    // even a sample far from every style shares the match instead of all 0.
    let nearest = v.first().map_or(0.0, |x| x.1);
    let weights: Vec<f32> = v
        .iter()
        .map(|(_, d)| (-(d * d - nearest * nearest) / (SPREAD * SPREAD)).exp())
        .collect();
    let total: f32 = weights.iter().sum();
    v.iter()
        .zip(&weights)
        .map(|((name, _), w)| (*name, (100.0 * w / total).round() as u32))
        .collect()
}

fn describe(x: &Features, known: [bool; 4]) -> String {
    let hands = match x.vpip {
        v if v < 0.15 => "very tight",
        v if v < 0.28 => "tight",
        v if v < 0.45 => "fairly loose",
        _ => "very loose",
    };
    let style = match x.aggression {
        a if a < 0.35 => "passive",
        a if a < 0.55 => "balanced",
        a if a < 0.72 => "aggressive",
        _ => "very aggressive",
    };
    let bluff = match x.bluff {
        b if b < 0.12 => "rarely bluffs",
        b if b < 0.30 => "bluffs sometimes",
        _ => "bluffs often",
    };
    let mut words = vec![hands];
    if known[2] {
        words.push(style);
    }
    if known[3] {
        words.push(bluff);
    }
    words.join(", ")
}

/// `None` until the player has played [`ROUGH_HANDS`] counted hands (or
/// `min_hands`, if that is fewer); rough until they have played `min_hands`
/// ([`MIN_HANDS`] unless configured otherwise).
pub fn profile(stats: &Stats, min_hands: u32) -> Option<Profile> {
    let min_hands = min_hands.max(1);
    if stats.hands < ROUGH_HANDS.min(min_hands) {
        return None;
    }
    let features = stats.features();
    let known = stats.known();
    Some(Profile {
        features,
        ranking: rank(&features, known),
        known,
        summary: describe(&features, known),
        rough: stats.hands < min_hands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_centroid_is_its_own_closest_match() {
        for a in &ARCHETYPES {
            assert_eq!(closest(&a.measured), a.personality.style);
            let r = rank(&a.measured, ALL_KNOWN);
            assert_eq!(r[0].0, a.personality.style);
            assert!(r[0].1 > r[1].1, "{r:?}");
            let total: u32 = r.iter().map(|x| x.1).sum();
            assert!((98..=102).contains(&total), "shares add up to {total}");
        }
    }

    #[test]
    fn what_is_not_seen_yet_does_not_count() {
        // Five hands of only folding and checking: no bets or calls at all.
        let s = Stats {
            hands: 5,
            ..Stats::default()
        };
        let p = profile(&s, MIN_HANDS).unwrap();
        assert_eq!(p.known, [true, true, false, false]);
        assert_eq!(
            p.summary, "very tight",
            "no word for aggression or bluffing yet"
        );
        assert!(p.ranking[0].1 > 0);
        assert_eq!(p.ranking[0].0, "Nit");
        // Never 0 for everyone: even a sample far from every style shares the match.
        let wild = Stats {
            hands: 5,
            vpip: 5,
            pfr: 5,
            bets: 9,
            calls: 0,
            postflop_bets: 4,
            bluffs: 4,
        };
        let p = profile(&wild, MIN_HANDS).unwrap();
        assert!(p.ranking[0].1 >= 20, "{:?}", p.ranking);
    }

    #[test]
    fn too_few_hands_give_no_profile() {
        let s = Stats {
            hands: ROUGH_HANDS - 1,
            ..Stats::default()
        };
        assert!(profile(&s, MIN_HANDS).is_none());
        let s = Stats {
            hands: ROUGH_HANDS,
            ..Stats::default()
        };
        assert!(profile(&s, MIN_HANDS).unwrap().rough);
        assert!(!profile(&s, ROUGH_HANDS).unwrap().rough);
        assert!(!profile(&s, 2).unwrap().rough, "a lower setting wins");
        let s = Stats {
            hands: 100,
            vpip: 70,
            pfr: 55,
            bets: 90,
            calls: 15,
            postflop_bets: 40,
            bluffs: 20,
        };
        let p = profile(&s, MIN_HANDS).unwrap();
        assert!(!p.rough);
        assert_eq!(p.ranking[0].0, "Maniac");
        assert!(p.summary.contains("very loose"));
    }

    #[test]
    fn games_add_up() {
        let mut all = Stats {
            hands: 10,
            bluffs: 1,
            ..Stats::default()
        };
        all.add(&Stats {
            hands: 5,
            vpip: 2,
            bluffs: 1,
            ..Stats::default()
        });
        assert_eq!((all.hands, all.vpip, all.bluffs), (15, 2, 2));
    }

    #[test]
    fn features_handle_zero_counts() {
        let x = Stats::default().features();
        assert_eq!((x.vpip, x.pfr, x.aggression, x.bluff), (0.0, 0.0, 0.0, 0.0));
    }
}
