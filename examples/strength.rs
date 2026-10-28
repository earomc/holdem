use holdem::bot::equity;
use holdem::card::{Deck, Rng};
fn main() {
    let mut rng = Rng::seeded(5);
    for n in [2usize, 3, 6, 9] {
        let opp = n - 1;
        let fair = 1.0 / n as f32;
        let best = 0.85 * 0.875f32.powi(n as i32 - 2);
        let spread = (best - fair).max(0.05);
        let mut v: Vec<f32> = (0..3000)
            .map(|_| {
                let mut d = Deck::new();
                d.shuffle(&mut rng);
                let hole = [d.draw(), d.draw()];
                (equity(&hole, &[], opp, 220, &mut rng) - fair) / spread
            })
            .collect();
        v.sort_by(f32::total_cmp);
        let q = |p: f32| v[((v.len() as f32 * p) as usize).min(v.len() - 1)];
        println!(
            "n={} pct(strength>=x): 90%:{:.2} 75%:{:.2} 60%:{:.2} 50%:{:.2} 40%:{:.2} 30%:{:.2} 20%:{:.2} 10%:{:.2} 5%:{:.2}",
            n,
            q(0.10),
            q(0.25),
            q(0.40),
            q(0.50),
            q(0.60),
            q(0.70),
            q(0.80),
            q(0.90),
            q(0.95)
        );
    }
}
