//! Measuring, cutting and wrapping text by its width on screen.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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

    #[test]
    fn fit_truncates_by_display_width() {
        assert_eq!(fit("hello", 10), "hello");
        assert_eq!(fit("hello world", 6), "hello…");
        assert_eq!(text_width(&fit("A♠ K♦ Q♥ J♣", 5)), 5);
        assert_eq!(fit("abc", 0), "");
    }
}
