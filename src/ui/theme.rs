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
//!
//! Characters are the one place where a *set* of colours is needed at once - hair,
//! skin, cloth, trim and eyes all appear in the same few cells - so each cast
//! member carries an [`Inks`] value rather than reaching into the role table for
//! eight colours by hand.

use crate::render::{Cell, Color, Style};
use crate::terminal::ColorSupport;

/// The colours one character is drawn from.
///
/// A figure's legend maps a character in its ASCII rows to one of these, so the
/// art says "this cell is hair" and the palette says what hair is. That keeps the
/// art legible in the source and lets a character be recoloured without touching
/// a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inks {
    /// Skin, for a face and hands.
    pub skin: Color,
    /// Hair, a hat, or a hood.
    pub hair: Color,
    /// The main garment.
    pub cloth: Color,
    /// The garment in shadow, or its underside.
    pub cloth_dark: Color,
    /// Metal, embroidery, and other small bright details.
    pub trim: Color,
    /// Eyes, and any small point of light on the figure.
    pub eye: Color,
    /// A staff or other wooden haft.
    pub wood: Color,
    /// Magic held in a focus.
    pub magic: Color,
    /// A void the figure is cut out of, such as a hood's interior.
    pub shadow: Color,
}

/// The colours one place is built from.
///
/// Architecture, not decoration: a wall's lit cap, its face, the shadow at its
/// base, the floor it stands on, the glazing light comes through, and the tint of
/// the pool the player carries. Six places sharing one grey is what made the
/// world look like a spreadsheet; each having its own stone is what makes it look
/// like somewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arte {
    /// The lit top edge of masonry.
    pub cap: Color,
    /// The face of a wall.
    pub wall: Color,
    /// The shadowed base of a wall.
    pub base: Color,
    /// The interior mass of deep stone.
    pub mass: Color,
    /// The open floor.
    pub floor: Color,
    /// Grit on the floor: rubble, seams, joints.
    pub grit: Color,
    /// Light coming through: glass, runes, apertures.
    pub window: Color,
    /// The background of the pool the player carries. Dark and only a step or
    /// two above the dark: the *light* comes from the glyphs it brightens, not
    /// from a lit rectangle behind them.
    pub light: Color,
    /// The background beyond the pool. Also dark, and the colour of the place's
    /// darkness rather than a neutral grey.
    pub dark: Color,
}

impl Arte {
    /// The palette a place is built from when the terminal can show it.
    #[must_use]
    pub fn rich(area: crate::game::AreaId) -> Arte {
        use crate::game::areas;
        match area {
            // Aged stone, gold glass, and candlelight.
            areas::CHAPEL => Arte {
                cap: Color::Indexed(180),
                wall: Color::Indexed(137),
                base: Color::Indexed(137),
                mass: Color::Indexed(233),
                floor: Color::Indexed(95),
                grit: Color::Indexed(180),
                window: Color::Indexed(214),
                light: Color::Indexed(235),
                dark: Color::Indexed(233),
            },
            // Damp masonry, deep water, and a cold refracted light.
            areas::CELLAR => Arte {
                cap: Color::Indexed(66),
                wall: Color::Indexed(24),
                base: Color::Indexed(24),
                mass: Color::Indexed(233),
                floor: Color::Indexed(60),
                grit: Color::Indexed(110),
                window: Color::Indexed(117),
                light: Color::Indexed(235),
                dark: Color::Indexed(233),
            },
            // Bark and moss, with lantern-amber in the gaps.
            areas::WOODS => Arte {
                cap: Color::Indexed(65),
                wall: Color::Indexed(22),
                base: Color::Indexed(22),
                mass: Color::Indexed(233),
                floor: Color::Indexed(22),
                grit: Color::Indexed(65),
                window: Color::Indexed(156),
                light: Color::Indexed(234),
                dark: Color::Indexed(233),
            },
            // Timber and dust, with a lamp behind every window.
            areas::VILLAGE => Arte {
                cap: Color::Indexed(222),
                wall: Color::Indexed(137),
                base: Color::Indexed(137),
                mass: Color::Indexed(234),
                floor: Color::Indexed(95),
                grit: Color::Indexed(180),
                window: Color::Indexed(214),
                light: Color::Indexed(235),
                dark: Color::Indexed(234),
            },
            // Pale marble, silver, and a light that comes from the water.
            areas::SHRINE => Arte {
                cap: Color::Indexed(103),
                wall: Color::Indexed(60),
                base: Color::Indexed(60),
                mass: Color::Indexed(233),
                floor: Color::Indexed(61),
                grit: Color::Indexed(110),
                window: Color::Indexed(195),
                light: Color::Indexed(234),
                dark: Color::Indexed(233),
            },
            // Bone, cold stone, and one spectral green.
            areas::CATACOMBS => Arte {
                cap: Color::Indexed(245),
                wall: Color::Indexed(180),
                base: Color::Indexed(180),
                mass: Color::Indexed(234),
                floor: Color::Indexed(180),
                grit: Color::Indexed(245),
                window: Color::Indexed(155),
                light: Color::Indexed(236),
                dark: Color::Indexed(234),
            },
            // Worn grey flagstones, for anywhere without a stone of its own.
            _ => Arte {
                cap: Color::Indexed(180),
                wall: Color::Indexed(137),
                base: Color::Indexed(95),
                mass: Color::Indexed(233),
                floor: Color::Indexed(95),
                grit: Color::Indexed(180),
                window: Color::Indexed(214),
                light: Color::Indexed(235),
                dark: Color::Indexed(233),
            },
        }
    }

    /// The same palette reduced to the sixteen colours a basic terminal has.
    #[must_use]
    pub fn reduced(self) -> Arte {
        Arte {
            cap: self.cap.nearest_basic(),
            wall: self.wall.nearest_basic(),
            base: self.base.nearest_basic(),
            mass: self.mass.nearest_basic(),
            floor: self.floor.nearest_basic(),
            grit: self.grit.nearest_basic(),
            window: self.window.nearest_basic(),
            light: self.light.nearest_basic(),
            dark: self.dark.nearest_basic(),
        }
    }
}

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
    /// The same, in shade: the mass of a wood rather than its edges.
    ///
    /// A separate role rather than `nature` plus the dim attribute, because a
    /// terminal may not honour dim on an indexed colour at all — which is exactly
    /// what made the first forest a wall of full-brightness green.
    pub nature_dark: Color,
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
    /// The player character's colours: the mage bunny.
    pub mage: Inks,
    /// Whether this palette has the colours the art wants. A basic terminal gets
    /// the same art reduced, never a second hand-written table.
    pub rich: bool,
    /// The veiled figure's colours.
    pub veiled: Inks,
    /// A small keeper carrying a lantern: warm gold and amber.
    pub lantern: Inks,
    /// A scavenger: weathered tan, dull brown, and a glint of brass.
    pub scavenger: Inks,
    /// A priestess: pale silver, deep indigo, and moonlight.
    pub priestess: Inks,
    /// A bound revenant: bone, grave-wrappings, and rust.
    pub revenant: Inks,
    /// A shrine apprentice: copper hair, teal cloth, white ribbons.
    pub apprentice: Inks,
    /// A wandering mage: weathered, crimson cloak, and gold.
    pub wanderer: Inks,
    /// A wood spirit: moss, bark, and a warm cap.
    pub spirit: Inks,
    /// A warden in steel: silver plate and a cold blue cloak.
    pub warden: Inks,
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
        nature_dark: Color::Green,
        relic: Color::LightBlue,
        warning: Color::LightRed,
        shade_deep: Color::Black,
        shade_mid: Color::Black,
        shade_near: Color::Blue,
        highlight_fg: Color::Black,
        rich: false,
        mage: Inks {
            skin: Color::LightYellow,
            hair: Color::White,
            cloth: Color::Blue,
            cloth_dark: Color::Black,
            trim: Color::Yellow,
            eye: Color::LightCyan,
            wood: Color::Yellow,
            magic: Color::LightMagenta,
            shadow: Color::Black,
        },
        veiled: Inks {
            skin: Color::DarkGrey,
            hair: Color::DarkGrey,
            cloth: Color::Black,
            cloth_dark: Color::Black,
            trim: Color::Magenta,
            eye: Color::LightMagenta,
            wood: Color::Black,
            magic: Color::LightMagenta,
            shadow: Color::Black,
        },
        lantern: Inks {
            skin: Color::LightYellow,
            hair: Color::Yellow,
            cloth: Color::LightRed,
            cloth_dark: Color::Red,
            trim: Color::White,
            eye: Color::LightCyan,
            wood: Color::Yellow,
            magic: Color::LightYellow,
            shadow: Color::Black,
        },
        scavenger: Inks {
            skin: Color::Yellow,
            hair: Color::DarkGrey,
            cloth: Color::DarkGrey,
            cloth_dark: Color::Black,
            trim: Color::Red,
            eye: Color::White,
            wood: Color::Yellow,
            magic: Color::Red,
            shadow: Color::Black,
        },
        priestess: Inks {
            skin: Color::White,
            hair: Color::LightCyan,
            cloth: Color::Blue,
            cloth_dark: Color::Black,
            trim: Color::White,
            eye: Color::LightCyan,
            wood: Color::Cyan,
            magic: Color::LightCyan,
            shadow: Color::Black,
        },
        revenant: Inks {
            skin: Color::White,
            hair: Color::DarkGrey,
            cloth: Color::DarkGrey,
            cloth_dark: Color::Black,
            trim: Color::Red,
            eye: Color::LightGreen,
            wood: Color::Red,
            magic: Color::LightGreen,
            shadow: Color::Black,
        },
        apprentice: Inks {
            skin: Color::LightYellow,
            hair: Color::LightRed,
            cloth: Color::Cyan,
            cloth_dark: Color::Blue,
            trim: Color::White,
            eye: Color::LightCyan,
            wood: Color::Yellow,
            magic: Color::LightMagenta,
            shadow: Color::Black,
        },
        wanderer: Inks {
            skin: Color::Yellow,
            hair: Color::Red,
            cloth: Color::Red,
            cloth_dark: Color::Black,
            trim: Color::Yellow,
            eye: Color::LightYellow,
            wood: Color::Yellow,
            magic: Color::LightRed,
            shadow: Color::Black,
        },
        spirit: Inks {
            skin: Color::White,
            hair: Color::Green,
            cloth: Color::Green,
            cloth_dark: Color::Black,
            trim: Color::Yellow,
            eye: Color::LightGreen,
            wood: Color::Yellow,
            magic: Color::LightGreen,
            shadow: Color::Black,
        },
        warden: Inks {
            skin: Color::White,
            hair: Color::LightCyan,
            cloth: Color::Blue,
            cloth_dark: Color::Black,
            trim: Color::White,
            eye: Color::LightCyan,
            wood: Color::Cyan,
            magic: Color::LightBlue,
            shadow: Color::Black,
        },
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
        nature_dark: Color::Indexed(22),
        relic: Color::Indexed(110),
        warning: Color::Indexed(203),
        shade_deep: Color::Indexed(234),
        shade_mid: Color::Indexed(236),
        shade_near: Color::Indexed(240),
        highlight_fg: Color::Indexed(232),
        rich: true,
        // A kawaii mage bunny: pink ears, pale fur, a violet robe with white
        // lace, and big bright eyes. Deliberately the most saturated thing on
        // screen - the point is that she pops off a world made almost entirely
        // of cold stone.
        mage: Inks {
            skin: Color::Indexed(224),
            hair: Color::Indexed(219),
            cloth: Color::Indexed(98),
            cloth_dark: Color::Indexed(60),
            trim: Color::Indexed(231),
            eye: Color::Indexed(51),
            wood: Color::Indexed(180),
            magic: Color::Indexed(213),
            shadow: Color::Indexed(234),
        },
        // The veiled one: no skin, a deep cowl, and cold violet light where a
        // face should be.
        veiled: Inks {
            skin: Color::Indexed(236),
            hair: Color::Indexed(236),
            cloth: Color::Indexed(236),
            cloth_dark: Color::Indexed(234),
            trim: Color::Indexed(97),
            eye: Color::Indexed(141),
            wood: Color::Indexed(234),
            magic: Color::Indexed(141),
            shadow: Color::Indexed(233),
        },
        // Rin, the keeper: a warm, bright little figure, the second most
        // saturated thing on screen after the player.
        lantern: Inks {
            skin: Color::Indexed(223),
            hair: Color::Indexed(222),
            cloth: Color::Indexed(179),
            cloth_dark: Color::Indexed(94),
            trim: Color::Indexed(231),
            eye: Color::Indexed(87),
            wood: Color::Indexed(137),
            magic: Color::Indexed(214),
            shadow: Color::Indexed(234),
        },
        // Cassen, the scavenger: weathered, dusty, and a little dishonest. Warm
        // but desaturated, so he sits in the landscape rather than on it.
        scavenger: Inks {
            skin: Color::Indexed(180),
            hair: Color::Indexed(101),
            cloth: Color::Indexed(95),
            cloth_dark: Color::Indexed(237),
            trim: Color::Indexed(137),
            eye: Color::Indexed(144),
            wood: Color::Indexed(101),
            magic: Color::Indexed(137),
            shadow: Color::Indexed(234),
        },
        // Ioreth, the priestess: cold silver and deep indigo, lit from above.
        priestess: Inks {
            skin: Color::Indexed(189),
            hair: Color::Indexed(189),
            cloth: Color::Indexed(61),
            cloth_dark: Color::Indexed(17),
            trim: Color::Indexed(255),
            eye: Color::Indexed(159),
            wood: Color::Indexed(144),
            magic: Color::Indexed(195),
            shadow: Color::Indexed(234),
        },
        // Vesper, the revenant: bone, grave-cloth, and rust. The only figure
        // with a green light in it, which is what makes it read as wrong.
        revenant: Inks {
            skin: Color::Indexed(187),
            hair: Color::Indexed(180),
            cloth: Color::Indexed(238),
            cloth_dark: Color::Indexed(236),
            trim: Color::Indexed(130),
            eye: Color::Indexed(155),
            wood: Color::Indexed(137),
            magic: Color::Indexed(155),
            shadow: Color::Indexed(233),
        },
        // Nim: warm copper hair, teal shrine cloth, and white ribbon. Bright,
        // warm, and the only character dressed in a colour that is not also in
        // the landscape.
        apprentice: Inks {
            skin: Color::Indexed(223),
            hair: Color::Indexed(215),
            cloth: Color::Indexed(80),
            cloth_dark: Color::Indexed(31),
            trim: Color::Indexed(231),
            eye: Color::Indexed(87),
            wood: Color::Indexed(137),
            magic: Color::Indexed(213),
            shadow: Color::Indexed(234),
        },
        // Maren: sun-bleached, crimson, and gold. The loudest of the wanderers,
        // which is what the hat is for.
        wanderer: Inks {
            skin: Color::Indexed(180),
            hair: Color::Indexed(131),
            cloth: Color::Indexed(124),
            cloth_dark: Color::Indexed(88),
            trim: Color::Indexed(220),
            eye: Color::Indexed(222),
            wood: Color::Indexed(137),
            magic: Color::Indexed(209),
            shadow: Color::Indexed(234),
        },
        // Wren: moss, bark, and a warm cap. Almost camouflage, so a glimpse of
        // her is easy to miss and worth having.
        spirit: Inks {
            skin: Color::Indexed(187),
            hair: Color::Indexed(113),
            cloth: Color::Indexed(65),
            cloth_dark: Color::Indexed(22),
            trim: Color::Indexed(180),
            eye: Color::Indexed(194),
            wood: Color::Indexed(101),
            magic: Color::Indexed(156),
            shadow: Color::Indexed(234),
        },
        // Tamsin: steel and silver, lit cold. She is the only figure in the
        // catacombs that is not bone-coloured, which is the point of her.
        warden: Inks {
            skin: Color::Indexed(223),
            hair: Color::Indexed(250),
            cloth: Color::Indexed(60),
            cloth_dark: Color::Indexed(17),
            trim: Color::Indexed(255),
            eye: Color::Indexed(117),
            wood: Color::Indexed(144),
            magic: Color::Indexed(195),
            shadow: Color::Indexed(234),
        },
    };

    /// The architecture of a place, in this palette's capability.
    #[must_use]
    pub fn arte(&self, area: crate::game::AreaId) -> Arte {
        let rich = Arte::rich(area);
        if self.rich { rich } else { rich.reduced() }
    }

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
    use super::{Arte, Palette};
    use crate::render::Color;
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
    fn every_place_is_built_from_its_own_stone() {
        // Six places sharing one grey is exactly what made the world look like a
        // spreadsheet, so each one must differ from every other.
        let places = [
            crate::game::areas::CHAPEL,
            crate::game::areas::CELLAR,
            crate::game::areas::WOODS,
            crate::game::areas::VILLAGE,
            crate::game::areas::SHRINE,
            crate::game::areas::CATACOMBS,
        ];
        for (index, place) in places.iter().enumerate() {
            for other in &places[index + 1..] {
                assert_ne!(
                    Arte::rich(*place),
                    Arte::rich(*other),
                    "{place:?} and {other:?} are built from the same stone"
                );
            }
        }
    }

    #[test]
    fn the_palette_hands_out_architecture_and_reduces_it() {
        let rich = Palette::EXTENDED.arte(crate::game::areas::CHAPEL);
        assert!(rich.wall != rich.cap, "a wall needs a lit edge");
        assert!(rich.floor != rich.grit, "a floor needs a light on it");
        // A basic terminal gets the same building, in the colours it has.
        let poor = Palette::BASIC.arte(crate::game::areas::CHAPEL);
        assert_ne!(rich, poor);
        assert!(matches!(
            poor.wall,
            Color::DarkGrey | Color::White | Color::Yellow
        ));
    }

    #[test]
    fn the_pool_of_light_is_dark_enough_to_draw_on() {
        // The pool is a *background*: a bright one becomes a lit rectangle, which
        // is the mistake this test exists to catch being made again.
        for place in [
            crate::game::areas::CHAPEL,
            crate::game::areas::CELLAR,
            crate::game::areas::WOODS,
            crate::game::areas::VILLAGE,
            crate::game::areas::SHRINE,
            crate::game::areas::CATACOMBS,
        ] {
            let arte = Arte::rich(place);
            let (r, g, b) = arte.light.as_rgb().expect("an indexed colour");
            assert!(
                r < 150 && g < 150 && b < 150,
                "{place:?} would draw a bright rectangle behind the player: {r},{g},{b}"
            );
            assert_ne!(arte.light, arte.dark, "{place:?} has no pool at all");
        }
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
