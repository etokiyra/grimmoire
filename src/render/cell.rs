//! A single character cell, the smallest unit the renderer composes.

use super::Style;

/// One character cell in the renderer's grid.
///
/// Grimmoire is ASCII-first: a cell holds exactly one `char`. Wide (double-width)
/// glyphs and combining marks are intentionally out of scope for this phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The glyph drawn in this cell.
    pub ch: char,
    /// The attributes applied to [`Cell::ch`].
    pub style: Style,
}

impl Cell {
    /// A blank cell: a space painted with the plain style.
    pub const BLANK: Cell = Cell {
        ch: ' ',
        style: Style::PLAIN,
    };

    /// Creates a cell from an explicit glyph and style.
    #[must_use]
    pub const fn new(ch: char, style: Style) -> Self {
        Cell { ch, style }
    }
}

impl Default for Cell {
    fn default() -> Self {
        Cell::BLANK
    }
}
