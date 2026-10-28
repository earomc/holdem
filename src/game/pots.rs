//! Main and side pots, and who wins which (pure functions, tested on their own).

use crate::eval::Score;

/// One pot (main or side) and the players who can win it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pot {
    pub amount: u32,
    pub eligible: Vec<usize>,
}

/// The outcome of one pot: winners (in odd-chip order) and their shares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Award {
    pub amount: u32,
    pub winners: Vec<usize>,
    pub shares: Vec<u32>,
}

/// Splits everything committed into a main pot and side pots.
///
/// `totals[i]` is what seat `i` put in over the hand; `live[i]` says whether the
/// seat can still win. Chips from folded players stay in the pots they paid into.
/// Every returned pot has at least one eligible player as long as one seat is live.
pub fn compute_pots(totals: &[u32], live: &[bool]) -> Vec<Pot> {
    let n = totals.len();
    let mut levels: Vec<u32> = (0..n)
        .filter(|&i| live[i] && totals[i] > 0)
        .map(|i| totals[i])
        .collect();
    levels.sort_unstable();
    levels.dedup();

    let mut pots: Vec<Pot> = Vec::new();
    let mut prev = 0u32;
    for &level in &levels {
        let amount: u32 = totals.iter().map(|&t| t.min(level) - t.min(prev)).sum();
        let eligible: Vec<usize> = (0..n).filter(|&i| live[i] && totals[i] >= level).collect();
        if amount > 0 {
            pots.push(Pot { amount, eligible });
        }
        prev = level;
    }

    // Dead money above the highest live level joins the last pot.
    let assigned: u32 = pots.iter().map(|p| p.amount).sum();
    let total: u32 = totals.iter().sum();
    if total > assigned {
        if let Some(last) = pots.last_mut() {
            last.amount += total - assigned;
        } else {
            let eligible: Vec<usize> = (0..n).filter(|&i| live[i]).collect();
            if !eligible.is_empty() {
                pots.push(Pot {
                    amount: total,
                    eligible,
                });
            }
        }
    }
    pots
}

/// Decides each pot. `scores[i]` is `None` for seats without a hand (folded or
/// never dealt). Ties split the pot; any odd chips go one at a time to the
/// winners in clockwise order starting left of the button.
pub fn award_pots(pots: &[Pot], scores: &[Option<Score>], button: usize) -> Vec<Award> {
    let n = scores.len();
    let first = (button + 1) % n;
    let mut awards = Vec::with_capacity(pots.len());
    for pot in pots {
        let best = pot.eligible.iter().filter_map(|&i| scores[i]).max();
        let mut winners: Vec<usize> = match best {
            Some(b) => pot
                .eligible
                .iter()
                .copied()
                .filter(|&i| scores[i] == Some(b))
                .collect(),
            // Nobody showed a hand (everyone else folded): the eligible player(s) take it.
            None => pot.eligible.clone(),
        };
        winners.sort_by_key(|&i| (i + n - first) % n);
        let k = winners.len() as u32;
        let base = pot.amount / k;
        let rem = pot.amount % k;
        let shares: Vec<u32> = (0..k).map(|j| base + if j < rem { 1 } else { 0 }).collect();
        awards.push(Award {
            amount: pot.amount,
            winners,
            shares,
        });
    }
    awards
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Rng;
    use crate::game::{Action, Game, Rules};

    #[test]
    fn unmatched_bets_do_not_make_side_pots() {
        let mut g = Game::new(Rules::default(), 4, Rng::seeded(1));
        g.setup(&[1500, 1500, 5, 1500], 0);
        g.start_hand();
        assert_eq!(g.players[2].bet, 5, "seat 2 is short in the big blind");
        g.players[2].all_in = false; // pretend for a moment it is not
        assert_eq!(g.pots_now(), vec![g.pot()]);
        g.players[2].all_in = true;
        // The short big blind's all-in closes a pot of 10 (5 from each blind);
        // the rest of the small blind is in the open pot...
        assert_eq!(g.pots_now(), vec![10, 5]);
        // ...and calls above it go to the open pot, even before others match.
        g.act(Action::Call).unwrap();
        assert_eq!(g.pots_now(), vec![15, 20]);
        assert_eq!(g.pots_now().iter().sum::<u32>(), g.pot());
    }

    #[test]
    fn pots_with_three_all_ins() {
        let pots = compute_pots(&[100, 300, 500], &[true, true, true]);
        assert_eq!(
            pots,
            vec![
                Pot {
                    amount: 300,
                    eligible: vec![0, 1, 2]
                },
                Pot {
                    amount: 400,
                    eligible: vec![1, 2]
                },
                Pot {
                    amount: 200,
                    eligible: vec![2]
                },
            ]
        );
    }

    #[test]
    fn folded_chips_stay_in_the_pot() {
        let pots = compute_pots(&[100, 300, 300, 40], &[true, true, true, false]);
        assert_eq!(
            pots,
            vec![
                Pot {
                    amount: 340,
                    eligible: vec![0, 1, 2]
                },
                Pot {
                    amount: 400,
                    eligible: vec![1, 2]
                },
            ]
        );
    }

    #[test]
    fn odd_chip_goes_left_of_the_button() {
        let pots = vec![Pot {
            amount: 101,
            eligible: vec![1, 3],
        }];
        let scores = vec![None, Some(5), None, Some(5)];
        let a = award_pots(&pots, &scores, 2);
        assert_eq!(a[0].winners, vec![3, 1]);
        assert_eq!(a[0].shares, vec![51, 50]);
        let a = award_pots(&pots, &scores, 0);
        assert_eq!(a[0].winners, vec![1, 3]);
        assert_eq!(a[0].shares, vec![51, 50]);
    }
}
