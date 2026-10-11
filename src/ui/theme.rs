//! The Grimmoire visual theme.
//!
//! The palette is expressed as **roles** rather than raw colours, so screen code
//! reads like art direction: a panel is filled with the surface, its edge with
//! the border, its heading with an accent, and its body with text.
//!
//! Two sets are provided: a portable 16-colour set, and a richer 256-colour set
//! used when the terminal reports support for it ([`Palette::for_colors`]). The
//! 16-colour set necessarily collapses the surface hierarchy — the intended
//! experience wants 256 colours or truecolour — but the viewport is still opaque
//! on both.

use crate::render::{Cell, Color, Style};
use crate::terminal::ColorSupport;

/// The theme, resolved into concrete colours for the terminal's capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The application viewport background. Painted over every cell so the
    /// terminal's own background and any desktop transparency never show.
    pub background: Color,
    /// The default panel surface, a step above the background.
    pub surface: Color,
    /// A raised surface for headers, selected rows, and inset panels.
    pub surface_raised: Color,
    /// Panel edges, rules, and other structure.
    pub border: Color,
    /// Primary body text.
    pub text: Color,
    /// De-emphasised text: hints, chrome, and units.
    pub text_muted: Color,
    /// The primary accent, used for titles and emphasis.
    pub accent: Color,
    /// A muted variant of the primary accent.
    pub accent_dim: Color,
    /// The secondary accent, used for cold or contrasting emphasis.
    pub accent_alt: Color,
    /// A muted variant of the secondary accent.
    pub accent_alt_dim: Color,
    /// The magical-effect colour, for light, embers, and spellwork.
    pub effect: Color,
    /// A muted variant of the effect colour.
    pub effect_dim: Color,
    /// Growth and natural materials.
    pub nature: Color,
    /// Relics, objects, and other cool highlights.
    pub relic: Color,
    /// Foreground used on top of a bright highlight background.
    pub highlight_fg: Color,
}

impl Palette {
    /// The portable 16-colour palette.
    ///
    /// Surfaces collapse onto the background here: with sixteen colours there is
    /// no honest way to express a raised hierarchy, so panels are distinguished
    /// by their borders instead.
    pub const BASIC: Palette = Palette {
        background: Color::Black,
        surface: Color::Black,
        surface_raised: Color::DarkGrey,
        border: Color::DarkGrey,
        text: Color::LightYellow,
        text_muted: Color::DarkGrey,
        accent: Color::LightMagenta,
        accent_dim: Color::Magenta,
        accent_alt: Color::LightCyan,
        accent_alt_dim: Color::Cyan,
        effect: Color::LightRed,
        effect_dim: Color::Red,
        nature: Color::LightGreen,
        relic: Color::LightBlue,
        highlight_fg: Color::Black,
    };

    /// The richer 256-colour palette.
    pub const EXTENDED: Palette = Palette {
        background: Color::Indexed(233),
        surface: Color::Indexed(235),
        surface_raised: Color::Indexed(237),
        border: Color::Indexed(240),
        text: Color::Indexed(223),
        text_muted: Color::Indexed(245),
        accent: Color::Indexed(141),
        accent_dim: Color::Indexed(97),
        accent_alt: Color::Indexed(117),
        accent_alt_dim: Color::Indexed(67),
        effect: Color::Indexed(209),
        effect_dim: Color::Indexed(131),
        nature: Color::Indexed(114),
        relic: Color::Indexed(110),
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

    /// The cell used to clear the whole viewport before composing.
    ///
    /// This is what makes the game's viewport opaque: every cell of the frame is
    /// given the theme background, and draws made without an explicit background
    /// inherit it (see `Canvas::set`).
    #[must_use]
    pub const fn backdrop(&self) -> Cell {
        Cell::new(
            ' ',
            Style::new()
                .with_fg(self.accent_alt_dim)
                .with_bg(self.background),
        )
    }

    /// Fills a panel or bar with the base surface.
    #[must_use]
    pub const fn surface_style(&self) -> Style {
        Style::new().with_bg(self.surface)
    }

    /// Fills a raised surface: headers, selected rows, inset panels.
    #[must_use]
    pub const fn raised_style(&self) -> Style {
        Style::new().with_bg(self.surface_raised)
    }

    /// Primary body text.
    #[must_use]
    pub const fn text_style(&self) -> Style {
        Style::new().with_fg(self.text)
    }

    /// De-emphasised text.
    #[must_use]
    pub const fn muted_style(&self) -> Style {
        Style::new().with_fg(self.text_muted)
    }

    /// Panel edges and rules.
    #[must_use]
    pub const fn border_style(&self) -> Style {
        Style::new().with_fg(self.border)
    }

    /// The primary accent.
    #[must_use]
    pub const fn accent_style(&self) -> Style {
        Style::new().with_fg(self.accent)
    }

    /// The magical-effect accent.
    #[must_use]
    pub const fn effect_style(&self) -> Style {
        Style::new().with_fg(self.effect)
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
    fn backdrop_carries_the_theme_background() {
        let cell = Palette::BASIC.backdrop();
        assert_eq!(' ', cell.ch);
        assert_eq!(Palette::BASIC.background, cell.style.bg);
    }

    #[test]
    fn role_styles_use_the_matching_colours() {
        let theme = Palette::EXTENDED;
        assert_eq!(theme.surface, theme.surface_style().bg);
        assert_eq!(theme.surface_raised, theme.raised_style().bg);
        assert_eq!(theme.text, theme.text_style().fg);
        assert_eq!(theme.text_muted, theme.muted_style().fg);
        assert_eq!(theme.border, theme.border_style().fg);
        assert_eq!(theme.accent, theme.accent_style().fg);
        assert_eq!(theme.effect, theme.effect_style().fg);
    }

    #[test]
    fn surfaces_are_distinct_in_the_rich_palette() {
        let theme = Palette::EXTENDED;
        assert_ne!(theme.background, theme.surface);
        assert_ne!(theme.surface, theme.surface_raised);
    }
}
