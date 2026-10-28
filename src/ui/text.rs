//! Measuring, cutting and wrapping text by its width on screen.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::style::NO_BREAK;

/// Display width of a string in terminal columns.
pub(super) fn text_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Splits off the longest prefix of `text` that fits in `width` columns
/// (always at least one character, so wrapping always makes progress).
fn split_at_width(text: &str, width: usize) -> (String, String) {
    let mut used = 0;
    let mut cut = text.len();
    for (i, c) in text.char_indices() {
        let w = c.width().unwrap_or(0);
        if used + w > width && i > 0 {
            cut = i;
            break;
        }
        used += w;
    }
    (text[..cut].to_string(), text[cut..].to_string())
}

/// Truncates `text` to at most `width` columns, ending in an ellipsis when cut.
pub(super) fn fit(text: &str, width: usize) -> String {
    if text_width(text) <= width {
        return text.to_string();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_string();
    }
    let (head, _) = split_at_width(text, width - 1);
    format!("{head}…")
}

/// Word-wraps `text` into lines of at most `width` columns. Continuation lines
/// start with `indent` spaces. Words longer than a line are broken.
pub(super) fn wrap_text(text: &str, width: usize, indent: usize) -> Vec<String> {
    let width = width.max(1);
    let indent = if indent * 2 >= width { 0 } else { indent };
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        loop {
            let limit = if lines.is_empty() {
                width
            } else {
                width - indent
            };
            let ww = text_width(&word);
            let needed = if current.is_empty() {
                ww
            } else {
                current_w + 1 + ww
            };
            if needed <= limit {
                if !current.is_empty() {
                    current.push(' ');
                    current_w += 1;
                }
                current.push_str(&word);
                current_w += ww;
                break;
            }
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_w = 0;
                continue;
            }
            // A single word wider than the line: break it.
            let (head, tail) = split_at_width(&word, limit);
            lines.push(head);
            word = tail;
            if word.is_empty() {
                break;
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    let pad = " ".repeat(indent);
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| if i == 0 { l } else { format!("{pad}{l}") })
        .collect()
}

/// Word-wraps styled lines to at most `width` columns, keeping every span's
/// style and each line's style and alignment. Lines that fit are kept as they
/// are; continuation lines start with the line's own leading spaces. Words
/// wider than a line are broken.
pub(super) fn wrap_lines(lines: Vec<Line<'static>>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        if line.width() <= width {
            out.push(line);
        } else {
            out.extend(wrap_line(&line, width));
        }
    }
    out
}

/// A word, or the space between two words: its pieces, each with its style.
struct Chunk {
    space: bool,
    pieces: Vec<(String, Style)>,
}

impl Chunk {
    fn width(&self) -> usize {
        self.pieces.iter().map(|(t, _)| text_width(t)).sum()
    }
}

/// Splits a line's spans into words and the spaces between them. A word may
/// run over several spans (a coloured part next to a plain one), and a no-break
/// space ([`NO_BREAK`]) belongs to the word.
fn chunks(line: &Line<'static>) -> Vec<Chunk> {
    let breaks = |c: char| c.is_whitespace() && c != NO_BREAK;
    let mut chunks: Vec<Chunk> = Vec::new();
    for span in &line.spans {
        let mut rest = span.content.as_ref();
        while let Some(first) = rest.chars().next() {
            let space = breaks(first);
            let end = rest
                .find(|c: char| breaks(c) != space)
                .unwrap_or(rest.len());
            let (part, tail) = rest.split_at(end);
            match chunks.last_mut() {
                Some(c) if c.space == space => c.pieces.push((part.to_string(), span.style)),
                _ => chunks.push(Chunk {
                    space,
                    pieces: vec![(part.to_string(), span.style)],
                }),
            }
            rest = tail;
        }
    }
    chunks
}

fn wrap_line(line: &Line<'static>, width: usize) -> Vec<Line<'static>> {
    let chunks = chunks(line);
    let lead = match chunks.first() {
        Some(c) if c.space => c.width(),
        _ => 0,
    };
    let indent = if lead * 2 >= width { 0 } else { lead };

    let mut out: Vec<Line<'static>> = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    // Nothing but the indent on the current line yet.
    let mut fresh = true;
    let finish = |spans: &mut Vec<Span<'static>>, out: &mut Vec<Line<'static>>| {
        let mut l = Line::from(std::mem::take(spans)).style(line.style);
        l.alignment = line.alignment;
        out.push(l);
    };
    let next_line = |spans: &mut Vec<Span<'static>>,
                     out: &mut Vec<Line<'static>>,
                     used: &mut usize,
                     fresh: &mut bool| {
        finish(spans, out);
        if indent > 0 {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        *used = indent;
        *fresh = true;
    };

    for chunk in chunks {
        let w = chunk.width();
        if chunk.space {
            // Spaces at the start of a continuation line, or that do not fit, go.
            if fresh && !out.is_empty() {
                continue;
            }
            if used + w <= width {
                spans.extend(chunk.pieces.into_iter().map(|(t, s)| Span::styled(t, s)));
                used += w;
            } else {
                next_line(&mut spans, &mut out, &mut used, &mut fresh);
            }
            continue;
        }
        if used + w > width && !fresh {
            next_line(&mut spans, &mut out, &mut used, &mut fresh);
        }
        if used + w <= width {
            spans.extend(chunk.pieces.into_iter().map(|(t, s)| Span::styled(t, s)));
            used += w;
            fresh = false;
            continue;
        }
        // A word wider than a whole line: break it.
        for (text, style) in chunk.pieces {
            let mut rest = text;
            while !rest.is_empty() {
                if used >= width {
                    next_line(&mut spans, &mut out, &mut used, &mut fresh);
                }
                let (head, tail) = split_at_width(&rest, width - used);
                used += text_width(&head);
                spans.push(Span::styled(head, style));
                fresh = false;
                rest = tail;
            }
        }
    }
    finish(&mut spans, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_lines_never_exceed_the_width() {
        let texts = [
            "Ava raises to 1200 (all-in)",
            "--- Hand #12 (level 3, blinds 25/50) - the button is on Chloe ---",
            "Ben and Dmitri split side pot 2 (1234) with Full House, Kings full of Fours",
            "Showdown (board: A♠ K♦ 10♥ 5♣ 2♠)",
            "Averyveryveryverylongwordthatcannotfitonasinglelineatall and more",
            "",
        ];
        for width in [5usize, 10, 20, 36, 60] {
            for t in &texts {
                let lines = wrap_text(t, width, 2);
                assert!(!lines.is_empty());
                for l in &lines {
                    assert!(text_width(l) <= width, "{l:?} is wider than {width}");
                }
                let joined: Vec<String> = lines.iter().map(|l| l.trim().to_string()).collect();
                assert_eq!(joined.join(" ").replace(' ', ""), t.replace(' ', ""));
            }
        }
    }

    /// The text of some lines, joined, without spaces.
    fn letters(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.replace(' ', ""))
            .collect()
    }

    #[test]
    fn styled_lines_wrap_within_the_width_and_keep_their_styles() {
        use ratatui::style::Color;
        let yellow = Style::new().fg(Color::Yellow);
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled("[F]", yellow),
            Span::raw(" Fold   "),
            Span::styled("[C]", yellow),
            Span::raw(" Call 170   "),
            Span::styled("[R]", yellow),
            Span::raw(" Raise to (min 340)   "),
            Span::styled("[A]", yellow),
            Span::raw(" All-in 1,390"),
        ]);
        for width in [8usize, 12, 20, 30, 50] {
            let wrapped = wrap_lines(vec![line.clone()], width);
            for l in &wrapped {
                assert!(l.width() <= width, "{l:?} is wider than {width}");
            }
            assert_eq!(letters(&wrapped), letters(std::slice::from_ref(&line)));
            // Every key keeps its colour.
            let keys = wrapped
                .iter()
                .flat_map(|l| l.spans.iter())
                .filter(|s| s.content.starts_with('['))
                .count();
            assert_eq!(keys, 4, "at width {width}");
            assert!(
                wrapped
                    .iter()
                    .flat_map(|l| l.spans.iter())
                    .filter(|s| s.content.starts_with('['))
                    .all(|s| s.style == yellow)
            );
        }
        // Continuation lines keep the indent.
        let wrapped = wrap_lines(vec![line.clone()], 30);
        assert!(wrapped.len() > 1);
        assert!(wrapped[1].spans[0].content.starts_with("  "));
    }

    #[test]
    fn lines_that_fit_stay_as_they_are() {
        let line = Line::from(vec![Span::raw(" ▶ "), Span::raw("Seats      6")]);
        assert_eq!(wrap_lines(vec![line.clone()], 40), vec![line]);
        let long = Line::from("x".repeat(25));
        let broken = wrap_lines(vec![long], 10);
        assert_eq!(broken.len(), 3, "a word wider than the line is broken");
        assert!(broken.iter().all(|l| l.width() <= 10));
    }

    #[test]
    fn fit_truncates_by_display_width() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello…");
        assert_eq!(text_width(&fit("A♠ K♦ Q♥ J♣", 5)), 5);
        assert_eq!(fit("abc", 0), "");
    }
}
