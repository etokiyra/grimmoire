//! Text width policy for the cell grid.
//!
//! A terminal cell holds exactly one column of output. Grimmoire is
//! ASCII-first, and this module makes that policy explicit rather than
//! pretending every Unicode scalar occupies one column.
//!
//! * Printable glyphs that occupy **exactly one** column are drawable.
//! * Control characters and zero-width glyphs (combining marks, joiners) are
//!   **dropped**.
//! * Double-width glyphs (CJK, emoji, most symbol pictographs) are **dropped**,
//!   because the cell model has no notion of a continuation cell yet. This is a
//!   documented limitation, not an accident; see `docs/renderer-design.md`.
//!
//! Dropping rather than substituting keeps the columns of the surviving glyphs
//! exactly as a caller expects and never desynchronises the grid.

use unicode_width::UnicodeWidthChar;

/// Whether `ch` occupies exactly one terminal column and may be drawn.
#[must_use]
pub fn is_drawable(ch: char) -> bool {
    char_width(ch) == 1
}

/// The number of terminal columns `ch` would occupy: `0`, `1`, or `2`.
///
/// Returns `0` for control characters and zero-width glyphs, and `2` (or more,
/// clamped to `2`) for double-width glyphs.
#[must_use]
pub fn char_width(ch: char) -> u8 {
    match ch.width() {
        Some(0) | None => 0,
        Some(1) => 1,
        Some(_) => 2,
    }
}

/// The number of columns `text` occupies once undrawable glyphs are dropped.
#[must_use]
pub fn display_width(text: &str) -> u16 {
    let columns: usize = text.chars().filter(|&ch| is_drawable(ch)).count();
    u16::try_from(columns).unwrap_or(u16::MAX)
}

/// Greedily word-wraps `text` to `width` columns, honouring explicit `\n`.
///
/// Words longer than `width` are placed on their own line and left to be clipped
/// by the drawing surface. Whitespace runs collapse to single spaces, and
/// trailing whitespace on a line is trimmed.
#[must_use]
pub fn wrap(text: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let word_width = display_width(word);
            if line.is_empty() {
                line.push_str(word);
            } else if display_width(&line)
                .saturating_add(1)
                .saturating_add(word_width)
                <= width
            {
                line.push(' ');
                line.push_str(word);
            } else {
                lines.push(std::mem::take(&mut line));
                line.push_str(word);
            }
        }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{char_width, display_width, is_drawable, wrap};

    #[test]
    fn ascii_is_one_column() {
        assert!(is_drawable('A'));
        assert_eq!(1, char_width(' '));
        assert_eq!(5, display_width("hello"));
    }

    #[test]
    fn control_and_zero_width_glyphs_are_dropped() {
        assert!(!is_drawable('\n'));
        assert!(!is_drawable('\t'));
        assert_eq!(0, char_width('\u{0301}'));
        assert!(!is_drawable('\u{0301}'));
        assert_eq!(3, display_width("a\u{0301}b\u{0301}c"));
    }

    #[test]
    fn double_width_glyphs_are_dropped() {
        assert_eq!(2, char_width('\u{4e2d}'));
        assert!(!is_drawable('\u{4e2d}'));
        // The wide glyph contributes no columns, so this measures 2.
        assert_eq!(2, display_width("a\u{4e2d}b"));
    }

    #[test]
    fn wrap_respects_explicit_newlines_and_width() {
        assert_eq!(vec!["a".to_string(), "b".to_string()], wrap("a\nb", 10));
    }

    #[test]
    fn wrap_breaks_on_word_boundaries() {
        assert_eq!(
            vec!["the quick".to_string(), "brown fox".to_string()],
            wrap("the quick brown fox", 9)
        );
    }

    #[test]
    fn wrap_keeps_overlong_words_on_their_own_line() {
        assert_eq!(
            vec![
                "a".to_string(),
                "supercalifragilistic".to_string(),
                "b".to_string()
            ],
            wrap("a supercalifragilistic b", 5)
        );
    }

    #[test]
    fn wrap_with_zero_width_is_empty() {
        assert!(wrap("anything", 0).is_empty());
    }
}
