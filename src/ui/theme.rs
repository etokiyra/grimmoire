//! The Grimmoire colour palette.
//!
//! Names are thematic so screen code reads like art direction rather than raw
//! colour values. Two palettes are provided: a portable 16-colour set that works
//! on any ANSI terminal, and a richer 256-colour set used when the terminal
//! reports support for it. [`Palette::for_colors`] selects between them.

use crate::render::{Cell, Color, Style};
use crate::terminal::ColorSupport;

/// The theme palette, resolved for the terminal's colour capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Background colour for the whole frame.
    pub night: Color,
    /// Primary arcane accent, used for titles and sigils.
    pub arcane: Color,
    /// Muted arcane accent.
    pub arcane_dim: Color,
    /// Body text colour.
    pub parchment: Color,
    /// Ember accent for warm highlights.
    pub ember: Color,
    /// Muted ember accent.
    pub ember_dim: Color,
    /// Cold accent for secondary text and edges.
    pub frost: Color,
    /// Muted cold accent.
    pub frost_dim: Color,
    /// Green accent reserved for future environments.
    pub moss: Color,
    /// Cool accent for relics and interactable objects.
    pub relic: Color,
    /// De-emphasised chrome such as borders and hints.
    pub shadow: Color,
    /// Foreground used on top of a bright highlight background.
    pub highlight_fg: Color,
}

impl Palette {
    /// The portable 16-colour palette.
    pub const BASIC: Palette = Palette {
        night: Color::Black,
        arcane: Color::LightMagenta,
        arcane_dim: Color::Magenta,
        parchment: Color::LightYellow,
        ember: Color::LightRed,
        ember_dim: Color::Red,
        frost: Color::LightCyan,
        frost_dim: Color::Cyan,
        moss: Color::LightGreen,
        relic: Color::LightBlue,
        shadow: Color::DarkGrey,
        highlight_fg: Color::Black,
    };

    /// The richer 256-colour palette.
    pub const EXTENDED: Palette = Palette {
        night: Color::Indexed(233),
        arcane: Color::Indexed(141),
        arcane_dim: Color::Indexed(97),
        parchment: Color::Indexed(223),
        ember: Color::Indexed(209),
        ember_dim: Color::Indexed(131),
        frost: Color::Indexed(117),
        frost_dim: Color::Indexed(67),
        moss: Color::Indexed(114),
        relic: Color::Indexed(110),
        shadow: Color::Indexed(240),
        highlight_fg: Color::Indexed(232),
    };

    /// Selects a palette appropriate for the reported colour support.
    #[must_use]
    pub const fn for_colors(support: ColorSupport) -> Palette {
        if support.has_extended_palette() {
            Palette::EXTENDED
        } else {
            Palette::BASIC
        }
    }

    /// The cell used to clear the frame before composing, carrying the night
    /// background so negative space reads as atmosphere rather than default.
    #[must_use]
    pub const fn backdrop(&self) -> Cell {
        Cell::new(
            ' ',
            Style::new().with_fg(self.frost_dim).with_bg(self.night),
        )
    }

    /// A muted style for chrome and hints.
    #[must_use]
    pub const fn muted(&self) -> Style {
        Style::new().with_fg(self.shadow)
    }
}

#[cfg(test)]
mod tests {
    use super::Palette;
    use crate::terminal::ColorSupport;

    #[test]
    fn extended_support_selects_the_richer_palette() {
        assert_eq!(
            Palette::EXTENDED,
            Palette::for_colors(ColorSupport::Ansi256)
        );
        assert_eq!(
            Palette::EXTENDED,
            Palette::for_colors(ColorSupport::TrueColor)
        );
        assert_eq!(Palette::BASIC, Palette::for_colors(ColorSupport::Ansi16));
    }

    #[test]
    fn backdrop_carries_the_night_background() {
        let cell = Palette::BASIC.backdrop();
        assert_eq!(' ', cell.ch);
        assert_eq!(Palette::BASIC.night, cell.style.bg);
    }
}
