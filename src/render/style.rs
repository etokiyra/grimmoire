//! Character attributes (colour and emphasis) applied to a [`super::Cell`].

use super::Color;

/// The visual attributes used to paint a single cell.
///
/// [`Style`] is a small, `Copy` value type. It is deliberately independent of
/// the terminal backend so that frame composition stays pure and testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    /// Foreground (glyph) colour.
    pub fg: Color,
    /// Background colour.
    pub bg: Color,
    /// Render the glyph in bold / high intensity.
    pub bold: bool,
    /// Render the glyph in a dimmed intensity.
    pub dim: bool,
    /// Render the glyph in italics.
    pub italic: bool,
    /// Underline the glyph.
    pub underline: bool,
}

impl Style {
    /// A style that uses default colours and emphasises nothing.
    pub const PLAIN: Style = Style {
        fg: Color::Default,
        bg: Color::Default,
        bold: false,
        dim: false,
        italic: false,
        underline: false,
    };

    /// Creates a plain style. Equivalent to [`Style::PLAIN`].
    #[must_use]
    pub const fn new() -> Self {
        Style::PLAIN
    }

    /// Returns a copy of this style with the foreground colour replaced.
    #[must_use]
    pub const fn with_fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }

    /// Returns a copy of this style with the background colour replaced.
    #[must_use]
    pub const fn with_bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    /// Returns a copy of this style with bold emphasis enabled.
    #[must_use]
    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Returns a copy of this style with dim emphasis enabled.
    #[must_use]
    pub const fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    /// Returns a copy of this style with italics enabled.
    #[must_use]
    pub const fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    /// Returns a copy of this style with underlining enabled.
    #[must_use]
    pub const fn underline(mut self) -> Self {
        self.underline = true;
        self
    }
}
