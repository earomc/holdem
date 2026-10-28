//! The table rules: blinds and their schedule, antes, straddle, rebuys, the
//! button and showdown rules, and how each reads in words.

use serde::{Deserialize, Serialize};

use super::chips_text;

/// The table rules. Everything optional is off by default.
///
/// Rules travel between server and clients, so unknown fields are ignored and
/// missing ones get their defaults: a slightly newer or older peer still
/// understands the rules it knows about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rules {
    pub starting_chips: u32,
    pub small_blind: u32,
    /// Hands played at each blind level; 0 keeps the blinds fixed.
    pub hands_per_level: u32,
    /// The small blind at each level, in percent of the starting small blind.
    pub blind_schedule: Vec<u32>,
    /// After the last level of the schedule, each level is this many percent of the previous one.
    pub blind_growth_percent: u32,
    /// The game ends after this many hands (the biggest stack wins); 0 = no limit.
    pub hand_limit: u32,
    /// Every player posts an ante each hand.
    pub antes: bool,
    /// The ante is the big blind divided by this (at least 1 chip).
    pub ante_divisor: u32,
    /// The player left of the big blind posts a straddle and acts last before the flop.
    pub straddle: bool,
    /// The straddle, in big blinds.
    pub straddle_multiplier: u32,
    /// Fewest players at the table for a straddle to be posted.
    pub straddle_min_players: usize,
    /// A busted player may buy back in while the blind level is at most this; 0 = no rebuys.
    pub rebuy_levels: u32,
    /// Chips a rebuy gives; 0 = the starting stack.
    pub rebuy_chips: u32,
    /// Dead-button rule: the big blind always moves to the next player, so the
    /// small blind or the button can fall on an empty seat.
    pub dead_button: bool,
    /// At a showdown, players who cannot win may throw their cards away unseen.
    pub muck_losers: bool,
}

/// The default blind schedule: level 1 is the starting small blind (100%).
pub const DEFAULT_BLIND_SCHEDULE: [u32; 12] = [
    100, 150, 250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000, 6000,
];

impl Default for Rules {
    fn default() -> Rules {
        Rules {
            starting_chips: 1500,
            small_blind: 10,
            hands_per_level: 8,
            blind_schedule: DEFAULT_BLIND_SCHEDULE.to_vec(),
            blind_growth_percent: 150,
            hand_limit: 0,
            antes: false,
            ante_divisor: 8,
            straddle: false,
            straddle_multiplier: 2,
            straddle_min_players: 4,
            rebuy_levels: 0,
            rebuy_chips: 0,
            dead_button: false,
            muck_losers: false,
        }
    }
}

impl Rules {
    /// The ante for a given big blind (big blind / divisor, at least 1).
    pub fn ante_for(&self, bb: u32) -> u32 {
        if self.antes {
            (bb / self.ante_divisor.max(1)).max(1)
        } else {
            0
        }
    }

    /// Chips a rebuy gives.
    pub fn rebuy_stack(&self) -> u32 {
        if self.rebuy_chips > 0 {
            self.rebuy_chips
        } else {
            self.starting_chips
        }
    }

    /// The small blind at a level (1 = first), following the schedule and then
    /// growing by `blind_growth_percent` per level.
    pub fn small_blind_at(&self, level: u32) -> u32 {
        let base = u64::from(self.small_blind.max(1));
        let schedule: &[u32] = if self.blind_schedule.is_empty() {
            &[100]
        } else {
            &self.blind_schedule
        };
        let i = level.saturating_sub(1) as usize;
        let mut sb = if i < schedule.len() {
            base * u64::from(schedule[i]) / 100
        } else {
            let mut v = base * u64::from(*schedule.last().unwrap_or(&100)) / 100;
            let growth = u64::from(self.blind_growth_percent.max(100));
            for _ in schedule.len()..=i {
                v = v * growth / 100;
                if v > u64::from(u32::MAX / 8) {
                    break;
                }
            }
            v
        };
        sb = sb.clamp(1, u64::from(u32::MAX / 8));
        sb as u32
    }

    // The rules in plain words, shared by the setup screen and `holdem serve`.

    /// "10/20": the small and big blind at the start.
    pub fn blinds_text(&self) -> String {
        format!("{}/{}", self.small_blind, self.small_blind * 2)
    }

    /// The smallest and largest step from one blind level to the next, in
    /// percent (e.g. 20 and 100), over the schedule and the growth after it.
    pub fn level_steps(&self) -> (u32, u32) {
        let mut steps: Vec<u32> = self
            .blind_schedule
            .windows(2)
            .map(|w| (w[1] * 100 / w[0].max(1)).saturating_sub(100))
            .collect();
        steps.push(self.blind_growth_percent.saturating_sub(100));
        let low = steps.iter().copied().min().unwrap_or(0);
        let high = steps.iter().copied().max().unwrap_or(0);
        (low, high)
    }

    /// "every 8 hands" or "never".
    pub fn levels_text(&self) -> String {
        match self.hands_per_level {
            0 => "never".to_string(),
            1 => "every hand".to_string(),
            n => format!("every {n} hands"),
        }
    }

    /// "50 hands" or "no limit".
    pub fn hand_limit_text(&self) -> String {
        match self.hand_limit {
            0 => "no limit".to_string(),
            n => format!("{n} hands"),
        }
    }

    /// The ante's size in words: "1/8 of the big blind".
    pub fn ante_size_text(&self) -> String {
        format!("1/{} of the big blind", self.ante_divisor)
    }

    /// "on: 1/8 of the big blind" or "off".
    pub fn antes_text(&self) -> String {
        if self.antes {
            format!("on: {}", self.ante_size_text())
        } else {
            "off".to_string()
        }
    }

    /// The straddle's terms in words: "2 big blinds, with 4 or more players".
    pub fn straddle_size_text(&self) -> String {
        format!(
            "{} big blinds, with {} or more players",
            self.straddle_multiplier, self.straddle_min_players
        )
    }

    /// "on: 2 big blinds, with 4 or more players" or "off".
    pub fn straddle_text(&self) -> String {
        if self.straddle {
            format!("on: {}", self.straddle_size_text())
        } else {
            "off".to_string()
        }
    }

    /// "during the first 2 levels, for 1,500 chips" or "off".
    pub fn rebuys_text(&self) -> String {
        let when = match self.rebuy_levels {
            0 => return "off".to_string(),
            1 => "during level 1".to_string(),
            n => format!("during the first {n} levels"),
        };
        format!("{}, for {} chips", when, chips_text(self.rebuy_stack()))
    }

    /// "dead button" or "moving button".
    pub fn button_text(&self) -> &'static str {
        if self.dead_button {
            "dead button"
        } else {
            "moving button"
        }
    }

    /// "losers may muck" or "everyone shows".
    pub fn showdown_text(&self) -> &'static str {
        if self.muck_losers {
            "losers may muck"
        } else {
            "everyone shows"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_in_words() {
        let mut r = Rules::default();
        assert_eq!(r.blinds_text(), "10/20");
        assert_eq!(r.level_steps(), (20, 100));
        assert_eq!(r.levels_text(), "every 8 hands");
        assert_eq!(r.hand_limit_text(), "no limit");
        assert_eq!(r.antes_text(), "off");
        r.antes = true;
        assert_eq!(r.antes_text(), "on: 1/8 of the big blind");
        r.straddle = true;
        assert_eq!(
            r.straddle_text(),
            "on: 2 big blinds, with 4 or more players"
        );
        r.rebuy_levels = 2;
        assert_eq!(
            r.rebuys_text(),
            "during the first 2 levels, for 1,500 chips"
        );
        assert_eq!(r.button_text(), "moving button");
        assert_eq!(r.showdown_text(), "everyone shows");
    }

    #[test]
    fn the_default_blind_schedule_matches_the_classic_levels() {
        let r = Rules::default();
        let levels: Vec<u32> = (1..=14).map(|l| r.small_blind_at(l)).collect();
        assert_eq!(
            levels,
            vec![
                10, 15, 25, 50, 75, 100, 150, 200, 300, 400, 500, 600, 900, 1350
            ]
        );
        let custom = Rules {
            small_blind: 5,
            blind_schedule: vec![100, 200],
            blind_growth_percent: 200,
            ..Rules::default()
        };
        let levels: Vec<u32> = (1..=4).map(|l| custom.small_blind_at(l)).collect();
        assert_eq!(levels, vec![5, 10, 20, 40]);
        // Huge levels stop growing instead of overflowing.
        assert!(r.small_blind_at(10_000) <= u32::MAX / 8);
    }
}
