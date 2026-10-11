//! The Grimmoire visual theme.
//!
//! The palette is expressed as **roles** rather than raw colours, so screen code
//! reads like art direction: a panel is filled with a surface, its edge with the
//! border, its heading with an accent, and its body with text.
//!
//! Two sets are provided: a portable 16-colour set, and a richer 256-colour set
//! used when the terminal reports support for it ([`Palette::for_colors`]). The
//! 16-colour set necessarily collapses the surface and shade hierarchy - the
//! intended experience wants 256 colours or truecolour - but the viewport is
//! opaque on both.
//!
//! Few tokens, used consistently, is the design rule. Nothing here should be
//! reached for because a screen wants "a different colour".

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
    /// Panel edges and box borders.
    pub border: Color,
    /// Thin rules separating content inside a surface.
    pub separator: Color,
    /// Primary body text.
    pub text: Color,
    /// Secondary text: values, labels, and less important prose.
    pub text_secondary: Color,
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
    /// Focus: the thing the player is attending to.
    pub focus: Color,
    /// Interactive objects and points of interest.
    pub interactive: Color,
    /// The player.
    pub player: Color,
    /// Important characters and creatures.
    pub character: Color,
    /// Growth and natural materials.
    pub nature: Color,
    /// Relics and cool accents.
    pub relic: Color,
    /// Warnings and errors.
    pub warning: Color,
    /// Ambient shade far from the player's light.
    pub shade_deep: Color,
    /// Ambient shade at middle distance.
    pub shade_mid: Color,
    /// Ambient shade close to the player's light.
    pub shade_near: Color,
    /// Foreground used on top of a bright highlight background.
    pub highlight_fg: Color,
}

impl Palette {
    /// The portable 16-colour palette.
    ///
    /// Surfaces and shades collapse onto the background here: with sixteen
    /// colours there is no honest way to express a raised hierarchy, so panels
    /// are distinguished by their borders instead.
    pub const BASIC: Palette = Palette {
        background: Color::Black,
        surface: Color::Black,
        surface_raised: Color::DarkGrey,
        border: Color::DarkGrey,
        separator: Color::DarkGrey,
        text: Color::LightYellow,
        text_secondary: Color::White,
        text_muted: Color::DarkGrey,
        accent: Color::LightMagenta,
        accent_dim: Color::Magenta,
        accent_alt: Color::LightCyan,
        accent_alt_dim: Color::Cyan,
        effect: Color::LightRed,
        effect_dim: Color::Red,
        focus: Color::LightYellow,
        interactive: Color::LightBlue,
        player: Color::LightYellow,
        character: Color::LightMagenta,
        nature: Color::LightGreen,
        relic: Color::LightBlue,
        warning: Color::LightRed,
        shade_deep: Color::Black,
        shade_mid: Color::Black,
        shade_near: Color::Blue,
        highlight_fg: Color::Black,
    };

    /// The richer 256-colour palette: an inky violet night with warm stone.
    pub const EXTENDED: Palette = Palette {
        background: Color::Indexed(233),
        surface: Color::Indexed(235),
        surface_raised: Color::Indexed(237),
        border: Color::Indexed(240),
        separator: Color::Indexed(238),
        text: Color::Indexed(223),
        text_secondary: Color::Indexed(250),
        text_muted: Color::Indexed(244),
        accent: Color::Indexed(141),
        accent_dim: Color::Indexed(97),
        accent_alt: Color::Indexed(117),
        accent_alt_dim: Color::Indexed(67),
        effect: Color::Indexed(209),
        effect_dim: Color::Indexed(131),
        focus: Color::Indexed(222),
        interactive: Color::Indexed(110),
        player: Color::Indexed(231),
        character: Color::Indexed(176),
        nature: Color::Indexed(107),
        relic: Color::Indexed(110),
        warning: Color::Indexed(203),
        shade_deep: Color::Indexed(233),
        shade_mid: Color::Indexed(235),
        shade_near: Color::Indexed(238),
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
    /// This is what makes the viewport opaque: every cell of the frame is given
    /// the theme background, and draws made without an explicit background
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

    /// Secondary text on a surface.
    #[must_use]
    pub const fn secondary_style(&self) -> Style {
        Style::new().with_fg(self.text_secondary)
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

    /// Warnings and errors.
    #[must_use]
    pub const fn warning_style(&self) -> Style {
        Style::new().with_fg(self.warning).bold()
    }

    /// The ambient shade tint for a squared distance in cells from the player's
    /// light. Squared, so the caller needs no square root.
    ///
    /// Three steps is deliberate: enough to read as light falling off, few
    /// enough to stay a tint rather than a rainbow.
    #[must_use]
    pub const fn shade_at(&self, squared_distance: u32) -> Color {
        match squared_distance {
            0..=9 => self.shade_near,
            10..=49 => self.shade_mid,
            _ => self.shade_deep,
        }
    }

    /// The ambient shade tint for a distance in cells from the player's light.
    #[must_use]
    pub const fn shade_for(&self, distance: u16) -> Color {
        self.shade_at((distance as u32) * (distance as u32))
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
        assert_eq!(theme.text_secondary, theme.secondary_style().fg);
        assert_eq!(theme.text_muted, theme.muted_style().fg);
        assert_eq!(theme.border, theme.border_style().fg);
        assert_eq!(theme.accent, theme.accent_style().fg);
        assert_eq!(theme.effect, theme.effect_style().fg);
        assert_eq!(theme.warning, theme.warning_style().fg);
    }

    #[test]
    fn surfaces_are_distinct_in_the_rich_palette() {
        let theme = Palette::EXTENDED;
        assert_ne!(theme.background, theme.surface);
        assert_ne!(theme.surface, theme.surface_raised);
        assert_ne!(theme.border, theme.surface);
    }

    #[test]
    fn the_shade_ramp_brightens_towards_the_player() {
        let theme = Palette::EXTENDED;
        assert_eq!(theme.shade_near, theme.shade_at(0));
        assert_eq!(theme.shade_near, theme.shade_at(9));
        assert_eq!(theme.shade_mid, theme.shade_at(10));
        assert_eq!(theme.shade_mid, theme.shade_at(49));
        assert_eq!(theme.shade_deep, theme.shade_at(50));
        assert_eq!(theme.shade_near, theme.shade_for(3));
        assert_eq!(theme.shade_mid, theme.shade_for(4));
        assert_ne!(theme.shade_near, theme.shade_deep);
    }
}
