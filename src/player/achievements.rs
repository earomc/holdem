//! Achievements: goals across all your games, some in stages (Winner I, II,
//! III). [`Progress`] counts what they are measured by; it grows hand by hand
//! ([`Progress::hand`]) and game by game ([`Progress::game`]), and is kept
//! with your statistics. Some stages unlock extra symbol shapes.

use serde::{Deserialize, Serialize};

use crate::eval::Category;
use crate::game::{HandResult, Outcome};

/// What the achievements are measured by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Progress {
    pub hands_played: u32,
    pub hands_won: u32,
    /// Pots taken by betting without a pair until everyone folded.
    pub bluffs_won: u32,
    pub all_ins_won: u32,
    /// The made hands won with at a showdown, one bit each (see [`MADE`]).
    pub made_hands: u16,
    pub kicker_wins: u32,
    /// The biggest pot won, in big blinds.
    pub biggest_pot_bb: u32,
    /// Hands won that another person was dealt into.
    pub wins_against_people: u32,
    /// The most players at a table you won a game at.
    pub biggest_table_won: u32,
    /// The fewest hands a game of five or more players took to win (0: none yet).
    pub fastest_win: u32,
    /// The most hands you played in one game.
    pub longest_game: u32,
    pub comebacks: u32,
    pub double_ups: u32,
    pub clean_wins: u32,
    pub games_hosted_with_people: u32,
    pub network_wins: u32,
}

/// How one game ended for you, for [`Progress::game`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GameEnd {
    /// Your final place, if the game ran to its end (`None`: you left).
    pub place: Option<usize>,
    /// Players who took part.
    pub players: u32,
    /// Hands the game took.
    pub hands: u32,
    /// Hands you were dealt in.
    pub hands_played: u32,
    /// Your smallest and biggest stack after a hand.
    pub lowest: u32,
    pub highest: u32,
    pub starting: u32,
    pub rebought: bool,
    /// Another person played at the table.
    pub people: bool,
    /// You hosted the table.
    pub hosted: bool,
}

/// The made hands with an achievement each, in the order of their bits.
pub const MADE: [&str; 6] = [
    "Straight",
    "Flush",
    "Full House",
    "Four of a Kind",
    "Straight Flush",
    "Royal Flush",
];

fn made_bit(category: Category, made: Option<&str>) -> Option<u16> {
    let i = match category {
        Category::Straight => 0,
        Category::Flush => 1,
        Category::FullHouse => 2,
        Category::FourOfAKind => 3,
        Category::StraightFlush if made == Some("Royal Flush") => 5,
        Category::StraightFlush => 4,
        _ => return None,
    };
    Some(1 << i)
}

impl Progress {
    /// Counts one hand you were dealt in.
    pub fn hand(&mut self, r: &HandResult) {
        self.hands_played += 1;
        let won = matches!(
            r.outcome,
            Outcome::WonUncontested | Outcome::WonShowdown | Outcome::Split
        );
        if !won {
            return;
        }
        self.hands_won += 1;
        if r.outcome == Outcome::WonUncontested && r.bluffed {
            self.bluffs_won += 1;
        }
        if r.all_in && r.outcome != Outcome::WonUncontested {
            self.all_ins_won += 1;
        }
        if let Some(bit) = r.category.and_then(|c| made_bit(c, r.made.as_deref())) {
            self.made_hands |= bit;
        }
        if r.kicker {
            self.kicker_wins += 1;
        }
        if let Some(bb) = r.payout.checked_div(r.big_blind) {
            self.biggest_pot_bb = self.biggest_pot_bb.max(bb);
        }
        if r.people {
            self.wins_against_people += 1;
        }
    }

    /// Counts the end of a game.
    pub fn game(&mut self, g: &GameEnd) {
        self.longest_game = self.longest_game.max(g.hands_played);
        if g.starting > 0 && g.highest >= 2 * g.starting {
            self.double_ups += 1;
        }
        if g.hosted && g.people {
            self.games_hosted_with_people += 1;
        }
        if g.place != Some(1) {
            return;
        }
        self.biggest_table_won = self.biggest_table_won.max(g.players);
        if g.players >= 5 && g.hands > 0 && (self.fastest_win == 0 || g.hands < self.fastest_win) {
            self.fastest_win = g.hands;
        }
        if g.starting > 0 && g.lowest * 10 < g.starting {
            self.comebacks += 1;
        }
        if !g.rebought {
            self.clean_wins += 1;
        }
        if g.people {
            self.network_wins += 1;
        }
    }

    /// These counts and `other`'s together (your saved ones and this game's).
    pub fn plus(&self, other: &Progress) -> Progress {
        let fastest = match (self.fastest_win, other.fastest_win) {
            (0, b) => b,
            (a, 0) => a,
            (a, b) => a.min(b),
        };
        Progress {
            hands_played: self.hands_played + other.hands_played,
            hands_won: self.hands_won + other.hands_won,
            bluffs_won: self.bluffs_won + other.bluffs_won,
            all_ins_won: self.all_ins_won + other.all_ins_won,
            made_hands: self.made_hands | other.made_hands,
            kicker_wins: self.kicker_wins + other.kicker_wins,
            biggest_pot_bb: self.biggest_pot_bb.max(other.biggest_pot_bb),
            wins_against_people: self.wins_against_people + other.wins_against_people,
            biggest_table_won: self.biggest_table_won.max(other.biggest_table_won),
            fastest_win: fastest,
            longest_game: self.longest_game.max(other.longest_game),
            comebacks: self.comebacks + other.comebacks,
            double_ups: self.double_ups + other.double_ups,
            clean_wins: self.clean_wins + other.clean_wins,
            games_hosted_with_people: self.games_hosted_with_people
                + other.games_hosted_with_people,
            network_wins: self.network_wins + other.network_wins,
        }
    }
}

/// One stage of an achievement.
#[derive(Debug)]
pub struct Stage {
    /// What the measure must reach (or, for "at most" achievements, stay within).
    pub goal: u32,
    /// "Win 100 hands".
    pub text: &'static str,
    /// The symbol shape it unlocks, as an index into the symbol shapes.
    pub unlocks: Option<u8>,
}

/// An achievement: a name, its stages from easiest to hardest, and what it
/// is measured by.
#[derive(Debug)]
pub struct Achievement {
    pub name: &'static str,
    pub stages: &'static [Stage],
    /// The measure from your progress.
    pub measure: fn(&Progress) -> u32,
    /// Lower is better (the goal is a most, like "within 40 hands"); 0 means
    /// not reached at all.
    pub at_most: bool,
    /// Progress is worth showing as "37 of 100" (not for yes-or-no goals).
    pub counts: bool,
}

const fn stage(goal: u32, text: &'static str) -> Stage {
    Stage {
        goal,
        text,
        unlocks: None,
    }
}

const fn unlocking(goal: u32, text: &'static str, glyph: u8) -> Stage {
    Stage {
        goal,
        text,
        unlocks: Some(glyph),
    }
}

const fn counted(
    name: &'static str,
    stages: &'static [Stage],
    measure: fn(&Progress) -> u32,
) -> Achievement {
    Achievement {
        name,
        stages,
        measure,
        at_most: false,
        counts: true,
    }
}

const fn once(
    name: &'static str,
    stage: &'static [Stage],
    measure: fn(&Progress) -> u32,
) -> Achievement {
    Achievement {
        name,
        stages: stage,
        measure,
        at_most: false,
        counts: false,
    }
}

fn made(p: &Progress, i: u16) -> u32 {
    u32::from(p.made_hands & (1 << i) != 0)
}

// The shapes achievements unlock, as indices into `symbol::GLYPHS`.
const CROWN: u8 = 12; // ♛
const STAR: u8 = 13; // ✦
const CROSS: u8 = 14; // ✠
const BOLT: u8 = 15; // ↯
const INTERROBANG: u8 = 16; // ‽
const KNIGHT: u8 = 17; // ♞
const RETURN: u8 = 18; // ↺
const COMMAND: u8 = 19; // ⌘

/// Every achievement, in the order the achievements screen shows them.
pub const ACHIEVEMENTS: [Achievement; 21] = [
    counted(
        "Winner",
        &[
            stage(1, "Win a hand"),
            stage(100, "Win 100 hands"),
            unlocking(1000, "Win 1,000 hands", STAR),
        ],
        |p| p.hands_won,
    ),
    once("Straight", &[stage(1, "Win a hand with a straight")], |p| {
        made(p, 0)
    }),
    once("Flush", &[stage(1, "Win a hand with a flush")], |p| {
        made(p, 1)
    }),
    once(
        "Full House",
        &[stage(1, "Win a hand with a full house")],
        |p| made(p, 2),
    ),
    once(
        "Four of a Kind",
        &[stage(1, "Win a hand with four of a kind")],
        |p| made(p, 3),
    ),
    once(
        "Straight Flush",
        &[stage(1, "Win a hand with a straight flush")],
        |p| made(p, 4),
    ),
    once(
        "Royal Flush",
        &[unlocking(1, "Win a hand with a royal flush", CROWN)],
        |p| made(p, 5),
    ),
    Achievement {
        name: "Table champion",
        stages: &[
            stage(3, "Win a game of 3 or more players"),
            stage(6, "Win a game of 6 or more players"),
            unlocking(9, "Win a game of 9 players", CROSS),
        ],
        measure: |p| p.biggest_table_won,
        at_most: false,
        counts: false,
    },
    Achievement {
        name: "Quick work",
        stages: &[
            stage(60, "Win a game of 5 or more players within 60 hands"),
            stage(40, "Win a game of 5 or more players within 40 hands"),
            unlocking(25, "Win a game of 5 or more players within 25 hands", BOLT),
        ],
        measure: |p| p.fastest_win,
        at_most: true,
        counts: false,
    },
    counted(
        "Bluffer",
        &[
            stage(
                1,
                "Take a pot by betting without a pair until everyone folds",
            ),
            stage(25, "Take 25 pots by bluffing"),
            unlocking(100, "Take 100 pots by bluffing", INTERROBANG),
        ],
        |p| p.bluffs_won,
    ),
    once(
        "Kicker",
        &[stage(1, "Win a pot your kicker decided")],
        |p| p.kicker_wins.min(1),
    ),
    Achievement {
        name: "Big pot",
        stages: &[
            stage(50, "Win a pot worth 50 big blinds"),
            stage(100, "Win a pot worth 100 big blinds"),
            stage(200, "Win a pot worth 200 big blinds"),
        ],
        measure: |p| p.biggest_pot_bb,
        at_most: false,
        counts: false,
    },
    counted(
        "All in",
        &[
            stage(1, "Win an all-in"),
            stage(10, "Win 10 all-ins"),
            stage(50, "Win 50 all-ins"),
        ],
        |p| p.all_ins_won,
    ),
    counted(
        "Regular",
        &[
            stage(100, "Play 100 hands"),
            stage(1000, "Play 1,000 hands"),
            unlocking(10_000, "Play 10,000 hands", KNIGHT),
        ],
        |p| p.hands_played,
    ),
    Achievement {
        name: "Marathon",
        stages: &[stage(200, "Play 200 hands in one game")],
        measure: |p| p.longest_game,
        at_most: false,
        counts: true,
    },
    once(
        "Comeback",
        &[unlocking(
            1,
            "Win a game after dropping below a tenth of the starting stack",
            RETURN,
        )],
        |p| p.comebacks.min(1),
    ),
    once(
        "Double up",
        &[stage(1, "Double your starting stack in one game")],
        |p| p.double_ups.min(1),
    ),
    once(
        "Clean win",
        &[stage(1, "Win a game without a rebuy")],
        |p| p.clean_wins.min(1),
    ),
    once(
        "Not just bots",
        &[stage(1, "Win a hand another person was dealt into")],
        |p| p.wins_against_people.min(1),
    ),
    once(
        "Host",
        &[stage(1, "Host a game that other people join")],
        |p| p.games_hosted_with_people.min(1),
    ),
    once(
        "Network champion",
        &[unlocking(
            1,
            "Win a game with another person at the table",
            COMMAND,
        )],
        |p| p.network_wins.min(1),
    ),
];

impl Achievement {
    /// Whether `progress` reaches stage `i`.
    pub fn reached(&self, progress: &Progress, i: usize) -> bool {
        let v = (self.measure)(progress);
        let goal = self.stages[i].goal;
        if self.at_most {
            v != 0 && v <= goal
        } else {
            v >= goal
        }
    }

    /// How many stages `progress` reaches (0 to all).
    pub fn level(&self, progress: &Progress) -> usize {
        (0..self.stages.len())
            .take_while(|&i| self.reached(progress, i))
            .count()
    }

    /// "Winner II", or just the name for an achievement with one stage.
    pub fn stage_name(&self, stage: usize) -> String {
        if self.stages.len() == 1 {
            self.name.to_string()
        } else {
            format!("{} {}", self.name, roman(stage + 1))
        }
    }

    /// "37 of 100" towards the next stage, where counting is meaningful.
    pub fn progress_text(&self, progress: &Progress) -> Option<String> {
        let next = self.stages.get(self.level(progress))?;
        let v = (self.measure)(progress);
        if self.counts && next.goal > 1 {
            Some(format!("{v} of {}", next.goal))
        } else if self.counts {
            None
        } else if self.at_most && v > 0 {
            Some(format!("best so far: {v} hands"))
        } else if !self.at_most && v > 0 && next.goal > 1 {
            Some(format!("best so far: {v}"))
        } else {
            None
        }
    }
}

/// I, II, III...
pub fn roman(n: usize) -> &'static str {
    ["", "I", "II", "III", "IV", "V"]
        .get(n)
        .copied()
        .unwrap_or("?")
}

/// The stages reached in `after` but not in `before`, as "Winner II" with
/// the stage's description.
pub fn earned(before: &Progress, after: &Progress) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for a in &ACHIEVEMENTS {
        for i in a.level(before)..a.level(after) {
            out.push((a.stage_name(i), a.stages[i].text));
        }
    }
    out
}

/// The stage that unlocks the symbol shape `glyph` ("Table champion III").
pub fn unlocked_by(glyph: u8) -> Option<String> {
    ACHIEVEMENTS.iter().find_map(|a| {
        let i = a.stages.iter().position(|s| s.unlocks == Some(glyph))?;
        Some(a.stage_name(i))
    })
}

/// The extra symbol shapes `progress` has unlocked.
pub fn unlocked_glyphs(progress: &Progress) -> Vec<u8> {
    let mut out = Vec::new();
    for a in &ACHIEVEMENTS {
        for (i, s) in a.stages.iter().enumerate() {
            if let Some(g) = s.unlocks
                && a.reached(progress, i)
            {
                out.push(g);
            }
        }
    }
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::symbol::{BASE_GLYPHS, GLYPHS};

    fn won(outcome: Outcome) -> HandResult {
        HandResult {
            outcome,
            payout: 400,
            big_blind: 20,
            ..HandResult::default()
        }
    }

    fn find(name: &str) -> &'static Achievement {
        ACHIEVEMENTS.iter().find(|a| a.name == name).unwrap()
    }

    #[test]
    fn hands_count_towards_their_achievements() {
        let mut p = Progress::default();
        p.hand(&HandResult {
            outcome: Outcome::Folded(crate::game::Street::Flop),
            ..HandResult::default()
        });
        assert_eq!((p.hands_played, p.hands_won), (1, 0));
        p.hand(&HandResult {
            bluffed: true,
            people: true,
            ..won(Outcome::WonUncontested)
        });
        p.hand(&HandResult {
            all_in: true,
            kicker: true,
            category: Some(Category::StraightFlush),
            made: Some("Royal Flush".to_string()),
            payout: 2000,
            ..won(Outcome::WonShowdown)
        });
        assert_eq!((p.hands_played, p.hands_won), (3, 2));
        assert_eq!((p.bluffs_won, p.all_ins_won, p.kicker_wins), (1, 1, 1));
        assert_eq!(p.biggest_pot_bb, 100);
        assert_eq!(p.wins_against_people, 1);
        assert_eq!(find("Royal Flush").level(&p), 1);
        assert_eq!(
            find("Straight Flush").level(&p),
            0,
            "a royal flush is its own"
        );
        assert_eq!(find("Big pot").level(&p), 2);
        assert_eq!(find("Winner").level(&p), 1);
        assert_eq!(
            find("Winner").progress_text(&p).as_deref(),
            Some("2 of 100")
        );
    }

    #[test]
    fn games_count_towards_their_achievements() {
        let mut p = Progress::default();
        let game = GameEnd {
            place: Some(1),
            players: 6,
            hands: 38,
            hands_played: 38,
            lowest: 120,
            highest: 9000,
            starting: 1500,
            rebought: false,
            people: true,
            hosted: true,
        };
        p.game(&game);
        assert_eq!(find("Table champion").level(&p), 2);
        assert_eq!(find("Quick work").level(&p), 2, "within 60 and 40, not 25");
        for name in [
            "Comeback",
            "Double up",
            "Clean win",
            "Host",
            "Network champion",
        ] {
            assert_eq!(find(name).level(&p), 1, "{name}");
        }
        // A slower win does not undo the fastest one.
        p.game(&GameEnd { hands: 90, ..game });
        assert_eq!(p.fastest_win, 38);
        // Leaving early wins nothing but still counts the game's length.
        let mut q = Progress::default();
        q.game(&GameEnd {
            place: None,
            hands_played: 210,
            ..game
        });
        assert_eq!(find("Marathon").level(&q), 1);
        assert_eq!(find("Table champion").level(&q), 0);
    }

    #[test]
    fn only_new_stages_are_earned() {
        let before = Progress {
            hands_won: 99,
            ..Progress::default()
        };
        let after = Progress {
            hands_won: 100,
            ..before
        };
        let got = earned(&before, &after);
        assert_eq!(got, vec![("Winner II".to_string(), "Win 100 hands")]);
        assert!(earned(&after, &after).is_empty());
        let both = earned(&Progress::default(), &after);
        assert_eq!(both.len(), 2, "Winner I and II at once");
    }

    #[test]
    fn saved_and_new_progress_add_up() {
        let a = Progress {
            hands_won: 3,
            made_hands: 1,
            fastest_win: 50,
            ..Progress::default()
        };
        let b = Progress {
            hands_won: 2,
            made_hands: 2,
            fastest_win: 0,
            ..Progress::default()
        };
        let sum = a.plus(&b);
        assert_eq!((sum.hands_won, sum.made_hands, sum.fastest_win), (5, 3, 50));
        assert_eq!(b.plus(&a).fastest_win, 50);
    }

    #[test]
    fn unlocks_are_the_extra_shapes_once_each() {
        let mut all: Vec<u8> = ACHIEVEMENTS
            .iter()
            .flat_map(|a| a.stages.iter().filter_map(|s| s.unlocks))
            .collect();
        all.sort_unstable();
        let extra: Vec<u8> = (BASE_GLYPHS as u8..GLYPHS.len() as u8).collect();
        assert_eq!(
            all, extra,
            "every extra shape is unlocked by exactly one stage"
        );
        assert!(unlocked_glyphs(&Progress::default()).is_empty());
        let royal = Progress {
            made_hands: 1 << 5,
            ..Progress::default()
        };
        assert_eq!(unlocked_glyphs(&royal), vec![CROWN]);
        assert_eq!(unlocked_by(CROWN).as_deref(), Some("Royal Flush"));
        assert_eq!(unlocked_by(CROSS).as_deref(), Some("Table champion III"));
        assert_eq!(unlocked_by(0), None, "open to everyone");
    }
}
