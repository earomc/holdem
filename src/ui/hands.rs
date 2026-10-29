//! Your hands in this game: what each one brought or cost, and how it ended.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

use holdem::game::{HandResult, Outcome, Street, chips_text};

use super::cards::small_cards;
use super::style::{dim, green};
use super::text::{text_width, wrap_text};

/// "+840", "-60" or "±0".
fn net_text(net: i64) -> String {
    let chips = chips_text(net.unsigned_abs().min(u64::from(u32::MAX)) as u32);
    match net {
        0 => "±0".to_string(),
        n if n > 0 => format!("+{chips}"),
        _ => format!("-{chips}"),
    }
}

/// "Nora", "Nora and Ben", "Nora, Ben and Ava".
fn names(list: &[String]) -> String {
    match list {
        [] => "nobody".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// How the hand ended, in words: "won at the showdown against Nora".
pub(super) fn how(r: &HandResult) -> String {
    let ended = match r.outcome {
        Outcome::Folded(Street::PreFlop) => "folded before the flop".to_string(),
        Outcome::Folded(street) => format!("folded on the {}", street.name().to_lowercase()),
        Outcome::WonUncontested if r.bluffed => "bluff: everyone folded".to_string(),
        Outcome::WonUncontested => "everyone else folded".to_string(),
        Outcome::WonShowdown => format!("won at the showdown against {}", names(&r.against)),
        Outcome::Split => format!("split the pot with {}", names(&r.against)),
        Outcome::LostShowdown => format!("lost at the showdown against {}", names(&r.against)),
    };
    // A bet without a pair may still have made a hand by the showdown, so only
    // the uncontested win is called a bluff outright.
    let ended = if r.bluffed && r.outcome != Outcome::WonUncontested {
        format!("{ended}, after betting without a pair")
    } else {
        ended
    };
    match &r.made {
        Some(made) => format!("{made}: {ended}"),
        None => ended,
    }
}

/// Columns the words of an entry need beside its numbers; with less room
/// (the narrow panel beside the table) they go on the lines below instead.
const BESIDE: usize = 28;

/// One hand, wrapped to `width`: "#14   +840   A♦ K♣  Pair of Kings: won at…".
fn entry(r: &HandResult, width: usize) -> Vec<Line<'static>> {
    let quiet = r.net == 0 && matches!(r.outcome, Outcome::Folded(_));
    let net_style = match r.net {
        0 => dim(),
        n if n > 0 => green(),
        _ => Style::new().fg(Color::LightRed),
    };
    let mut prefix = vec![
        Span::styled(format!("#{:<4}", r.hand), dim()),
        Span::styled(format!("{:>6}  ", net_text(r.net)), net_style),
    ];
    prefix.extend(small_cards(&r.hole, quiet));
    let text_style = if quiet { dim() } else { Style::new() };
    let used: usize = prefix.iter().map(|s| text_width(&s.content)).sum::<usize>() + 2;
    let mut lines = Vec::new();
    if width >= used + BESIDE {
        prefix.push(Span::raw("  "));
        for (i, piece) in wrap_text(&how(r), width - used, 0).into_iter().enumerate() {
            let mut spans = if i == 0 {
                prefix.clone()
            } else {
                vec![Span::raw(" ".repeat(used))]
            };
            spans.push(Span::styled(piece, text_style));
            lines.push(Line::from(spans));
        }
    } else {
        lines.push(Line::from(prefix));
        for piece in wrap_text(&how(r), width.saturating_sub(2).max(1), 0) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(piece, text_style),
            ]));
        }
    }
    lines
}

/// "This game: +880 over 14 hands · 5 won · 2 bluffs, 1 took the pot".
pub(super) fn summary(hands: &[HandResult]) -> String {
    let net: i64 = hands.iter().map(|h| h.net).sum();
    let won = hands
        .iter()
        .filter(|h| matches!(h.outcome, Outcome::WonUncontested | Outcome::WonShowdown))
        .count();
    let bluffs = hands.iter().filter(|h| h.bluffed).count();
    let bluffs_won = hands.iter().filter(|h| h.bluffed && h.net > 0).count();
    let n = hands.len();
    let mut text = format!(
        "This game: {} over {n} hand{} · {won} won",
        net_text(net),
        if n == 1 { "" } else { "s" }
    );
    if bluffs > 0 {
        text.push_str(&format!(
            " · {bluffs} bluff{}, {bluffs_won} paid off",
            if bluffs == 1 { "" } else { "s" }
        ));
    }
    text
}

/// Your hands wrapped to `width` columns, newest at the bottom. Returns the
/// `height` lines that end `scroll` lines above the newest one, and the
/// largest useful `scroll` (as [`super::log::log_view`] does).
pub(super) fn hands_view(
    hands: &[HandResult],
    height: usize,
    width: usize,
    scroll: usize,
) -> (Vec<Line<'static>>, usize) {
    if hands.is_empty() {
        let note = Line::from(Span::styled(
            "Your hands show here once the first one is over.",
            dim(),
        ));
        return (vec![note], 0);
    }
    let mut all: Vec<Line<'static>> = hands.iter().flat_map(|h| entry(h, width)).collect();
    let max = all.len().saturating_sub(height);
    let scroll = scroll.min(max);
    let end = all.len() - scroll;
    let start = end.saturating_sub(height);
    (all.drain(start..end).collect(), max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use holdem::card::parse_cards;

    fn hand(outcome: Outcome, net: i64, bluffed: bool) -> HandResult {
        HandResult {
            hand: 3,
            net,
            hole: parse_cards("As Kd").unwrap(),
            made: None,
            outcome,
            bluffed,
            against: vec!["Nora".to_string(), "Ben".to_string()],
            ..HandResult::default()
        }
    }

    #[test]
    fn every_ending_reads_as_words() {
        assert_eq!(
            how(&hand(Outcome::Folded(Street::PreFlop), 0, false)),
            "folded before the flop"
        );
        assert_eq!(
            how(&hand(Outcome::Folded(Street::Turn), -60, true)),
            "folded on the turn, after betting without a pair"
        );
        assert_eq!(
            how(&hand(Outcome::WonUncontested, 30, true)),
            "bluff: everyone folded"
        );
        let mut won = hand(Outcome::WonShowdown, 840, false);
        won.made = Some("Pair of Kings, Ace kicker".to_string());
        assert_eq!(
            how(&won),
            "Pair of Kings, Ace kicker: won at the showdown against Nora and Ben"
        );
        assert_eq!(
            how(&hand(Outcome::LostShowdown, -200, true)),
            "lost at the showdown against Nora and Ben, after betting without a pair"
        );
    }

    #[test]
    fn the_summary_adds_up_the_game() {
        let hands = [
            hand(Outcome::WonShowdown, 840, false),
            hand(Outcome::Folded(Street::Flop), -60, false),
            hand(Outcome::WonUncontested, 100, true),
            hand(Outcome::LostShowdown, -200, true),
        ];
        assert_eq!(
            summary(&hands),
            "This game: +680 over 4 hands · 2 won · 2 bluffs, 1 paid off"
        );
        assert_eq!(net_text(-1500), "-1,500");
    }

    #[test]
    fn entries_fit_wide_and_narrow() {
        let mut won = hand(Outcome::WonShowdown, 840, false);
        won.made = Some("Pair of Kings, Ace kicker".to_string());
        for width in [100, 50, 34, 20] {
            let lines = entry(&won, width);
            assert!(lines.len() > 1 || width == 100);
            for l in &lines {
                assert!(l.width() <= width, "{width}: {l:?}");
            }
        }
        // Narrow: the numbers on one line, the words below, indented.
        let lines = entry(&won, 34);
        assert!(
            lines[1].to_string().starts_with("  Pair of Kings"),
            "{lines:?}"
        );
    }
}
