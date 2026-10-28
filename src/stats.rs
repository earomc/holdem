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

fn distance(a: &Features, b: &Features) -> f32 {
    let d = |x: f32, y: f32, w: f32| (w * (x - y)).powi(2);
    (d(a.vpip, b.vpip, WEIGHTS.vpip)
        + d(a.pfr, b.pfr, WEIGHTS.pfr)
        + d(a.aggression, b.aggression, WEIGHTS.aggression)
        + d(a.bluff, b.bluff, WEIGHTS.bluff))
    .sqrt()
}

/// An estimate of a player's style.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub features: Features,
    /// Closest archetype first, with a match score from 0 to 100.
    pub ranking: Vec<(&'static str, u32)>,
    /// "Tight" / "Loose" etc. in plain words.
    pub summary: String,
    /// Based on fewer hands than make a reliable estimate.
    pub rough: bool,
}

/// The archetype whose measured statistics are nearest to `features`.
pub fn closest(features: &Features) -> &'static str {
    rank(features)[0].0
}

fn rank(features: &Features) -> Vec<(&'static str, u32)> {
    let mut v: Vec<(&'static str, f32)> = ARCHETYPES
        .iter()
        .map(|a| (a.personality.style, distance(features, &a.measured)))
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1));
    // A distance of 0 is a perfect match; 0.6 is about as far apart as two styles get.
    v.into_iter()
        .map(|(name, d)| {
            (
                name,
                ((1.0 - d / 0.6).clamp(0.0, 1.0) * 100.0).round() as u32,
            )
        })
        .collect()
}

fn describe(x: &Features) -> String {
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
    format!("{hands}, {style}, {bluff}")
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
    Some(Profile {
        features,
        ranking: rank(&features),
        summary: describe(&features),
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
            assert_eq!(rank(&a.measured)[0].1, 100);
        }
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
