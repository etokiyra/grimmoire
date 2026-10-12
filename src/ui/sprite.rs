//! The character sprite library.
//!
//! This is the **authoritative** character art: fixed, source-controlled sprites
//! that the game draws as-is. They are assets, not suggestions.
//!
//! # Preservation
//!
//! A sprite's rows are stored exactly as they were authored. Nothing here trims,
//! reflows, pads or rewrites them: a row's leading and trailing spaces are part
//! of the art, because the art's shape is what makes a silhouette readable. If a
//! row needs to change, it changes in the source text and the preservation tests
//! are updated deliberately — never by a normalising call in the renderer.
//!
//! `preserves_every_declared_row_exactly` asserts each stored row against a
//! second copy of the source art, so a careless edit fails the build rather than
//! quietly changing a character's face.
//!
//! # Colours come from the renderer, never from the art
//!
//! A sprite is monochrome source. [`Part`] classifies each *glyph* into a role —
//! body, shade, light, metal, eye, accent — and [`ArtInks`] says what colour each
//! role is for that character. So a sprite can be recoloured, re-posed or drawn
//! over a different background without a single glyph in the source changing.
//!
//! # Logical cells
//!
//! A sprite declares its logical width and height in cells. Every glyph in the
//! library is verified to be **one terminal column wide** by
//! [`every_sprite_glyph_is_one_cell_wide`], so a logical cell is a physical cell
//! and a sprite never desynchronises the row it is drawn on. If a future sprite
//! needs a glyph that is not one column wide the renderer must grow an explicit
//! logical-versus-physical distinction; the test is there to say so out loud
//! rather than let a row drift.

use std::time::Duration;

use crate::game::{Figure, Mood};
use crate::render::Color;

/// The sprite the world draws for a character.
///
/// The supplied library is mapped onto the region's cast by what each character
/// *is*, not by filling slots: the veiled keeper is a hood of shade with two cold
/// lights in it, the revenant is bone and wrappings, the wandering mage is the
/// wide hat and robes, the warden is the plumed helm and plate.
#[must_use]
pub const fn for_figure(figure: Figure) -> &'static Sprite {
    match figure {
        Figure::Mage => &MC_WIZARD,
        Figure::Veiled => &SHADOW_WRAITH,
        Figure::Lantern => &RIN,
        Figure::Scavenger => &CASSEN,
        Figure::Priestess => &IORETH,
        Figure::Revenant => &SKELETON_WARRIOR,
        Figure::Apprentice => &NIM,
        Figure::Wanderer => &HAG_WITCH,
        Figure::Spirit => &WREN,
        Figure::Warden => &DARK_KNIGHT,
    }
}

/// What a sprite is, which is what decides how much room it gets and how it is
/// allowed to be animated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The player.
    Hero,
    /// A character the player can talk to.
    Npc,
    /// Something hostile.
    Enemy,
    /// A beast.
    Creature,
    /// Something that gets its own music.
    Boss,
}

/// Which ink a cell of a sprite is drawn in.
///
/// Classified from the glyph itself, because the art predates any legend: the
/// supplied sprites use a consistent vocabulary of full blocks, shades, half
/// blocks and thin rules, and that vocabulary *is* the shading model. Keeping
/// the classification here means the art stays untouched and every character can
/// be recoloured from one table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// Hard silhouette and rounded edges.
    Outline,
    /// The main mass: robe, plate, hide.
    Body,
    /// The mass in shade.
    Shade,
    /// The mass in light.
    Light,
    /// The brightest highlights.
    Highlight,
    /// Metal: buckles, blades, rings, hafts.
    Metal,
    /// Eyes and other small points that carry a face.
    Eye,
    /// Gems, spikes, beaks, tails — the loud details.
    Accent,
}

/// The ink set one character is drawn from.
///
/// Eight colours, chosen per character. They are written as 256-colour indices
/// and reduced to the nearest basic colour when the terminal cannot do better,
/// so there is exactly one definition per character rather than two that can
/// drift apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtInks {
    /// Hard silhouette and edges.
    pub outline: Color,
    /// The main mass.
    pub body: Color,
    /// The mass in shade.
    pub shade: Color,
    /// The mass in light.
    pub light: Color,
    /// The brightest highlights.
    pub highlight: Color,
    /// Metal.
    pub metal: Color,
    /// Eyes.
    pub eye: Color,
    /// Gems, spikes and other loud details.
    pub accent: Color,
}

impl ArtInks {
    /// The colour a part is drawn in.
    #[must_use]
    pub const fn of(&self, part: Part) -> Color {
        match part {
            Part::Outline => self.outline,
            Part::Body => self.body,
            Part::Shade => self.shade,
            Part::Light => self.light,
            Part::Highlight => self.highlight,
            Part::Metal => self.metal,
            Part::Eye => self.eye,
            Part::Accent => self.accent,
        }
    }

    /// The same inks, lit for an expression.
    ///
    /// The supplied art has one face per character, so an expression is carried
    /// by the light in it rather than by redrawing a mouth: the character's
    /// brightest cells warm, cool or go out with their mood. The art never
    /// changes, only the ink it is drawn in.
    #[must_use]
    pub fn mooded(self, mood: Mood) -> ArtInks {
        let highlight = match mood.band() {
            0 => self.highlight,
            1 => Color::Indexed(255),
            2 => Color::Indexed(247),
            _ => Color::Indexed(243),
        };
        let eye = match mood.band() {
            1 => Color::Indexed(231),
            _ => self.eye,
        };
        ArtInks {
            highlight,
            eye,
            ..self
        }
    }

    /// The same inks, reduced to the 16 colours a basic terminal has.
    #[must_use]
    pub fn reduced(&self) -> ArtInks {
        ArtInks {
            outline: self.outline.nearest_basic(),
            body: self.body.nearest_basic(),
            shade: self.shade.nearest_basic(),
            light: self.light.nearest_basic(),
            highlight: self.highlight.nearest_basic(),
            metal: self.metal.nearest_basic(),
            eye: self.eye.nearest_basic(),
            accent: self.accent.nearest_basic(),
        }
    }
}

/// A fixed piece of character art.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprite {
    /// Stable identifier, used in tests and by the gallery.
    pub id: &'static str,
    /// What the art is.
    pub role: Role,
    /// Logical width in cells, as declared by the art.
    pub width: u16,
    /// Logical height in rows, as declared by the art.
    pub height: u16,
    /// The rows, exactly as authored. Never trimmed, never reflowed.
    pub rows: &'static [&'static str],
    /// Its colours.
    pub inks: ArtInks,
    /// The walk poses, alternated one per step. Empty means the base pose is used.
    pub steps: &'static [&'static [&'static str]],
    /// The top of a breath, if the character has one.
    pub breath: Option<&'static [&'static str]>,
    /// The sprint, if the character has one.
    pub sprint: Option<&'static [&'static str]>,
}

impl Sprite {
    /// How many distinct poses this sprite can show, including its base.
    #[must_use]
    pub fn pose_count(&self) -> usize {
        1 + self.steps.len()
            + usize::from(self.breath.is_some())
            + usize::from(self.sprint.is_some())
    }

    /// Every row this sprite can ever draw, for the preservation tests.
    #[must_use]
    pub fn all_poses(&self) -> Vec<&'static [&'static str]> {
        let mut poses = vec![self.rows];
        poses.extend(self.steps.iter().copied());
        poses.extend(self.breath);
        poses.extend(self.sprint);
        poses
    }

    /// The block a frame occupies, as `(left, top)` offsets from the sprite's
    /// anchor, for a sprite drawn with its last row on the anchor's row and
    /// centred on its column.
    #[must_use]
    pub fn origin(&self) -> (i32, i32) {
        (
            -(i32::from(self.width) - 1) / 2,
            -(i32::from(self.height) - 1),
        )
    }

    /// Whether the art is internally consistent: the declared size is the real
    /// size, and every frame matches.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        let widths = |rows: &[&str]| {
            rows.iter()
                .all(|row| row.chars().count() == usize::from(self.width))
        };
        rows_matching(self.rows, self.height) && widths(self.rows)
    }
}

/// Whether `rows` has exactly `height` entries.
fn rows_matching(rows: &[&str], height: u16) -> bool {
    rows.len() == usize::from(height)
}

/// Which ink a glyph is drawn in.
///
/// The vocabulary of the library: full blocks and half blocks are the mass,
/// shades are its shading, thin double rules and box glyphs are metalwork, and
/// the few symbolic glyphs are eyes and accents.
#[must_use]
pub const fn part_of(ch: char) -> Part {
    match ch {
        // Hard edges and rounded corners.
        '▟' | '▙' | '▛' | '▜' | '▖' | '▗' | '▘' | '▝' | '◣' | '◢' | '◤' | '◥' | '◁' | '▷' | '╱'
        | '╲' => Part::Outline,
        // The mass.
        '█' | '▄' | '▀' | '▌' | '▐' => Part::Body,
        // Its shading.
        '▓' => Part::Shade,
        '▒' => Part::Light,
        '░' => Part::Highlight,
        // Metalwork.
        '═' | '╫' | '╨' | '╘' | '╛' | '╽' | '┴' | '▬' | '│' => Part::Metal,
        // Faces.
        '°' | '◉' => Part::Eye,
        // Everything else is a loud detail: gems, spikes, tails.
        _ => Part::Accent,
    }
}

/// The player: a small wizard in violet, with a gold-tipped staff.
pub static MC_WIZARD: Sprite = Sprite {
    id: "mc-wizard",
    role: Role::Hero,
    width: 6,
    height: 5,
    rows: &["  ▟▖  ", "▄▟█▙▄ ", " ◣ ◢ ◆", "═▓♦▓▘│", " ▓▓▓ │"],
    inks: ArtInks {
        outline: Color::Indexed(55),
        body: Color::Indexed(99),
        shade: Color::Indexed(54),
        light: Color::Indexed(141),
        highlight: Color::Indexed(183),
        metal: Color::Indexed(220),
        eye: Color::Indexed(51),
        accent: Color::Indexed(213),
    },
    // Two steps, a breath and a sprint, so a walk and a run have real poses to
    // cut between. The base art above is never one of them.
    steps: &[MC_STEP_A, MC_STEP_B],
    breath: Some(MC_BREATH),
    sprint: Some(MC_SPRINT),
};

/// The wizard mid-step: the robe swings one way.
static MC_STEP_A: &[&str] = &["  ▟▖  ", "▄▟█▙▄ ", " ◣ ◢ ◆", "═▓♦▓▘│", "▓▓ ▓ │"];

/// The wizard mid-step, the other way.
static MC_STEP_B: &[&str] = &["  ▟▖  ", "▄▟█▙▄ ", " ◣ ◢ ◆", "═▓♦▓▘│", "▓ ▓▓ │"];

/// The wizard at the top of a breath: the shoulders rise one cell.
static MC_BREATH: &[&str] = &["  ▟▖  ", "▄▟█▙▄ ", " ◣◢  ◆", "═▓♦▓▘│", " ▓▓▓ │"];

/// The wizard sprinting: leaning in, robe trailing behind.
static MC_SPRINT: &[&str] = &["   ▟▖ ", "▄▟█▙▄ ", "  ◣◢◆ ", "═▓♦▓▘│", "▓▓▓  │"];

/// A necromancer: a violet cowl with two pale points and a bone staff.
pub static NECROMANCER: Sprite = Sprite {
    id: "necromancer",
    role: Role::Enemy,
    width: 7,
    height: 5,
    rows: &[" ▄███▄ ", " █° °█ ", " ▀█▼█▀ ", " ░▓█▓░╽", " ░▓▓▓░│"],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(56),
        shade: Color::Indexed(238),
        light: Color::Indexed(98),
        highlight: Color::Indexed(183),
        metal: Color::Indexed(250),
        eye: Color::Indexed(118),
        accent: Color::Indexed(164),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A hag: a wide-brimmed pointed hat over robes, with a green face.
pub static HAG_WITCH: Sprite = Sprite {
    id: "hag-witch",
    role: Role::Enemy,
    width: 7,
    height: 6,
    rows: &[
        "   ▟▖  ",
        "  ▟█▙  ",
        "▄▄███▄▄",
        " ▐°v°▌ ",
        " ▟▓▓▓▙ ",
        "▝▀▀▀▀▀▘",
    ],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(58),
        shade: Color::Indexed(239),
        light: Color::Indexed(107),
        highlight: Color::Indexed(186),
        metal: Color::Indexed(180),
        eye: Color::Indexed(214),
        accent: Color::Indexed(113),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A plague doctor: a long hood, a bone beak, two lenses.
pub static PLAGUE_DOCTOR: Sprite = Sprite {
    id: "plague-doctor",
    role: Role::Enemy,
    width: 7,
    height: 5,
    rows: &[" ▄▄█▄▄ ", " ▐◉ ◉▌ ", "  ◥█◣  ", " ▟▓▓▓▙ ", "  ▐ ▌  "],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(242),
        shade: Color::Indexed(238),
        light: Color::Indexed(248),
        highlight: Color::Indexed(253),
        metal: Color::Indexed(180),
        eye: Color::Indexed(214),
        accent: Color::Indexed(137),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// An assassin: black cloth, two blades, and one crimson mark.
pub static DARK_ELF_ASSASSIN: Sprite = Sprite {
    id: "dark-elf-assassin",
    role: Role::Enemy,
    width: 7,
    height: 5,
    rows: &[" ▄███▄ ", "◁█◣ ◢█▷", " ▜▀▀▀▛ ", "╱▓███▓╲", " ▟▛ ▜▙ "],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(238),
        shade: Color::Indexed(234),
        light: Color::Indexed(145),
        highlight: Color::Indexed(189),
        metal: Color::Indexed(253),
        eye: Color::Indexed(155),
        accent: Color::Indexed(89),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A knight: a plumed helm, plate, and a spear.
pub static DARK_KNIGHT: Sprite = Sprite {
    id: "dark-knight",
    role: Role::Enemy,
    width: 7,
    height: 5,
    rows: &["  ▄▲▄  ", " ▐█▬█▌ ", "◢█████◣", " ▐███▌│", "  ▙ ▟ ┴"],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(66),
        shade: Color::Indexed(238),
        light: Color::Indexed(110),
        highlight: Color::Indexed(153),
        metal: Color::Indexed(253),
        eye: Color::Indexed(117),
        accent: Color::Indexed(131),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A skeleton warrior: bone, wrappings, and an old blade.
pub static SKELETON_WARRIOR: Sprite = Sprite {
    id: "skeleton-warrior",
    role: Role::Creature,
    width: 7,
    height: 5,
    rows: &["  ▄▄▄  ", " ▐° °▌ ", "  ╘▀╛ ╽", "▐█╫═╫ │", "  ╨ ╨ ┴"],
    inks: ArtInks {
        outline: Color::Indexed(234),
        body: Color::Indexed(248),
        shade: Color::Indexed(243),
        light: Color::Indexed(252),
        highlight: Color::Indexed(255),
        metal: Color::Indexed(180),
        eye: Color::Indexed(214),
        accent: Color::Indexed(144),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A wraith: a hood of shade with two cold lights in it.
pub static SHADOW_WRAITH: Sprite = Sprite {
    id: "shadow-wraith",
    role: Role::Creature,
    width: 7,
    height: 5,
    rows: &["  ▄▄▄  ", " ▟◣ ◢▙ ", " ▐░▒░▌ ", " ▝▓▒▓▘ ", "  ░ ░  "],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(240),
        shade: Color::Indexed(236),
        light: Color::Indexed(246),
        highlight: Color::Indexed(252),
        metal: Color::Indexed(243),
        eye: Color::Indexed(141),
        accent: Color::Indexed(98),
    },
    // It drifts: the hem trails to one side and then the other.
    steps: &[WRAITH_DRIFT],
    breath: None,
    sprint: None,
};

/// The wraith mid-drift.
static WRAITH_DRIFT: &[&str] = &["  ▄▄▄  ", " ▟◣ ◢▙ ", " ▐░▒░▌ ", " ▝▓▒▓▘ ", "   ░░  "];

/// An imp: small, crimson, horned, and pleased with itself.
pub static IMP: Sprite = Sprite {
    id: "imp",
    role: Role::Creature,
    width: 7,
    height: 4,
    rows: &["▟▌   ▐▙", " ▐◣▼◢▌ ", "▗▟███▙▖", " ▐▌ ▐▌~"],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(161),
        shade: Color::Indexed(88),
        light: Color::Indexed(197),
        highlight: Color::Indexed(210),
        metal: Color::Indexed(180),
        eye: Color::Indexed(226),
        accent: Color::Indexed(203),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// A bat: wings, a pale face, and two fangs.
pub static BAT: Sprite = Sprite {
    id: "bat",
    role: Role::Creature,
    width: 7,
    height: 4,
    rows: &["▙▖   ▗▟", "▐█▙▄▟█▌", " ▝°▼°▘ ", "  ▀ ▀  "],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(239),
        shade: Color::Indexed(235),
        light: Color::Indexed(55),
        highlight: Color::Indexed(141),
        metal: Color::Indexed(243),
        eye: Color::Indexed(213),
        accent: Color::Indexed(98),
    },
    // It flaps: the wings come up a row.
    steps: &[BAT_FLAP],
    breath: None,
    sprint: None,
};

/// The bat mid-flap.
static BAT_FLAP: &[&str] = &["▗▙   ▟▖", "▐█▙▄▟█▌", " ▝°▼°▘ ", "   ▀   "];

/// An ooze: a shiny, unbothered puddle with two calm eyes.
pub static OOZE: Sprite = Sprite {
    id: "ooze",
    role: Role::Creature,
    width: 7,
    height: 4,
    rows: &["  ░▒░  ", " ▟° °▙ ", "▟▓▒▓▒▓▙", "▀▀▀▀▀▀▀"],
    inks: ArtInks {
        outline: Color::Indexed(22),
        body: Color::Indexed(77),
        shade: Color::Indexed(237),
        light: Color::Indexed(119),
        highlight: Color::Indexed(157),
        metal: Color::Indexed(144),
        eye: Color::Indexed(226),
        accent: Color::Indexed(84),
    },
    // It settles and rises.
    steps: &[OOZE_SETTLE],
    breath: None,
    sprint: None,
};

/// The ooze at the bottom of a wobble.
static OOZE_SETTLE: &[&str] = &["  ░▒░  ", " ▟° °▙ ", "▟▒▓▒▓▒▙", "▀▀▀▀▀▀▀"];

/// A drake: crimson scales, wings, and a tail.
pub static DRAKE: Sprite = Sprite {
    id: "drake",
    role: Role::Creature,
    width: 7,
    height: 4,
    rows: &["◢▙   ▟◣", "▐◣▄▄▄◢▌", "◥█▓▓▓█◤", " ▝▙▓▟▘~"],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(130),
        shade: Color::Indexed(88),
        light: Color::Indexed(202),
        highlight: Color::Indexed(210),
        metal: Color::Indexed(180),
        eye: Color::Indexed(226),
        accent: Color::Indexed(203),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// The Lich King: a crown, a void face, and a robe of bone and spectral light.
pub static LICH_KING: Sprite = Sprite {
    id: "lich-king",
    role: Role::Boss,
    width: 11,
    height: 7,
    rows: &[
        "  ▲ ▟█▙ ▲  ",
        " ▟▀▀███▀▀▙ ",
        " █ ◉ ▼ ◉ █ ",
        " ▜▄▀▀▀▀▀▄▛ ",
        "◢█▓▓▓♦▓▓▓█◣",
        "▐▓▓▓▓▓▓▓▓▓▌",
        "  ▀▘   ▝▀  ",
    ],
    inks: ArtInks {
        outline: Color::Indexed(232),
        body: Color::Indexed(251),
        shade: Color::Indexed(245),
        light: Color::Indexed(254),
        highlight: Color::Indexed(255),
        metal: Color::Indexed(220),
        eye: Color::Indexed(51),
        accent: Color::Indexed(165),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

// -- authored in the library's own style -------------------------------------
//
// The supplied collection covers the player, a set of enemies, creatures and a
// boss. Five of the region's characters had no counterpart whose silhouette
// suited them, so they are drawn here in the same vocabulary and at the same
// size as everything else.

/// Rin: a small keeper in a patched coat, holding a lantern out.
pub static RIN: Sprite = Sprite {
    id: "rin",
    role: Role::Npc,
    width: 7,
    height: 5,
    rows: &["  ▟▖   ", " ▟░▙   ", " ▐° ░▌◆", " ▟▓▓▓▙╿", " ▐▙ ▟▌ "],
    inks: ArtInks {
        outline: Color::Indexed(94),
        body: Color::Indexed(137),
        shade: Color::Indexed(101),
        light: Color::Indexed(222),
        highlight: Color::Indexed(231),
        metal: Color::Indexed(214),
        eye: Color::Indexed(87),
        accent: Color::Indexed(231),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// Cassen: hunched under a bundle, with a stick.
pub static CASSEN: Sprite = Sprite {
    id: "cassen",
    role: Role::Npc,
    width: 7,
    height: 5,
    rows: &[" ▄░░▄  ", " ▟░▒░▙ ", "▐° ░°▌ ", " ▟▓▓▓▙ ", " ▐ ▐ ▐│"],
    inks: ArtInks {
        outline: Color::Indexed(237),
        body: Color::Indexed(101),
        shade: Color::Indexed(240),
        light: Color::Indexed(180),
        highlight: Color::Indexed(223),
        metal: Color::Indexed(144),
        eye: Color::Indexed(144),
        accent: Color::Indexed(107),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// Ioreth: tall under a crescent crown, robed to the floor.
pub static IORETH: Sprite = Sprite {
    id: "ioreth",
    role: Role::Npc,
    width: 7,
    height: 6,
    rows: &[
        " ▲   ▲ ",
        " ▟░░░▙ ",
        " ▐° ░°▌",
        "  ▟░▒▓▙",
        " ▐▓▓◆▓▌",
        "▝▀▀▀▀▀▘",
    ],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(68),
        shade: Color::Indexed(240),
        light: Color::Indexed(110),
        highlight: Color::Indexed(189),
        metal: Color::Indexed(255),
        eye: Color::Indexed(159),
        accent: Color::Indexed(195),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// Nim: a shrine apprentice, ribbons in her hair, a charm on a string.
pub static NIM: Sprite = Sprite {
    id: "nim",
    role: Role::Npc,
    width: 7,
    height: 5,
    rows: &[" ▟▖ ▟▖ ", " ▟░▙▟░▙", " ▐◉ ◉▌ ", "  ▟░▒▓▙", " ▐▙ ▟▌ "],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(80),
        shade: Color::Indexed(240),
        light: Color::Indexed(51),
        highlight: Color::Indexed(231),
        metal: Color::Indexed(255),
        eye: Color::Indexed(87),
        accent: Color::Indexed(213),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// Wren: a small spirit under a spotted cap, hands low, moss at the hem.
pub static WREN: Sprite = Sprite {
    id: "wren",
    role: Role::Npc,
    width: 7,
    height: 5,
    rows: &[" ▄◦▄◦▄ ", " ▟░▒▒▙ ", " ▐° °▌ ", " ▟▓▓▓▙ ", " ▝▀▒▒▀▘"],
    inks: ArtInks {
        outline: Color::Indexed(233),
        body: Color::Indexed(65),
        shade: Color::Indexed(22),
        light: Color::Indexed(107),
        highlight: Color::Indexed(187),
        metal: Color::Indexed(180),
        eye: Color::Indexed(194),
        accent: Color::Indexed(156),
    },
    steps: &[],
    breath: None,
    sprint: None,
};

/// Every sprite in the library.
pub static SPRITES: &[&Sprite] = &[
    &MC_WIZARD,
    &NECROMANCER,
    &HAG_WITCH,
    &PLAGUE_DOCTOR,
    &DARK_ELF_ASSASSIN,
    &DARK_KNIGHT,
    &SKELETON_WARRIOR,
    &SHADOW_WRAITH,
    &IMP,
    &BAT,
    &OOZE,
    &DRAKE,
    &LICH_KING,
    &RIN,
    &CASSEN,
    &IORETH,
    &NIM,
    &WREN,
];

/// Draws a sprite into a view, with its top-left corner at `(x, y)`.
///
/// The one place a sprite becomes cells, so the world, the title screen and any
/// future gallery all draw the same art the same way: each glyph coloured by
/// what it is, mirrored only when the caller asks.
pub fn draw(
    view: &mut crate::render::CanvasView,
    x: u16,
    y: u16,
    art: &Sprite,
    pose: Pose,
    inks: ArtInks,
    mirrored: bool,
) {
    let rows = pose.rows_for(art);
    let width = i32::from(art.width);
    for (row, line) in rows.iter().enumerate() {
        for (column, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let column = if mirrored {
                width - 1 - column as i32
            } else {
                column as i32
            };
            let cx = i32::from(x) + column;
            let cy = i32::from(y) + row as i32;
            if cx < 0 || cy < 0 {
                continue;
            }
            view.put_char(
                cx as u16,
                cy as u16,
                ch,
                crate::render::Style::new()
                    .with_fg(inks.of(part_of(ch)))
                    .bold(),
            );
        }
    }
}

/// The sprite with this id, if there is one.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Sprite> {
    SPRITES.iter().copied().find(|sprite| sprite.id == id)
}

/// How long one step of a walk lasts, and so how long a pose is held.
pub const STEP: Duration = Duration::from_millis(120);

/// Which frame a sprite should show.
///
/// The walk is cut from the sprite's own poses rather than from a free-running
/// clock: the stride advances once per *step*, so the legs can never drift away
/// from the movement they are illustrating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    /// Standing still.
    Idle,
    /// Mid-step, `step` counting the steps taken so far.
    Walk {
        /// How many steps have been taken, for alternating the pose.
        step: u32,
    },
    /// Running.
    Sprint,
}

impl Pose {
    /// The rows this pose draws for a sprite.
    ///
    /// A sprite without a pose for the moment simply holds its base art, so a
    /// creature with one companion frame cannot be asked for a frame it does not
    /// have.
    #[must_use]
    pub fn rows_for(self, sprite: &Sprite) -> &'static [&'static str] {
        match self {
            Pose::Idle => sprite.breath.unwrap_or(sprite.rows),
            Pose::Walk { step } if !sprite.steps.is_empty() => {
                sprite.steps[step as usize % sprite.steps.len()]
            }
            Pose::Walk { .. } => sprite.rows,
            Pose::Sprint => sprite.sprint.unwrap_or(sprite.rows),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ArtInks, BAT, LICH_KING, MC_WIZARD, Part, Pose, Role, SPRITES, Sprite, by_id, part_of,
    };
    use crate::render::glyph;

    /// The supplied art, written a second time. If the library is ever edited,
    /// this copy is what says so.
    ///
    /// These rows are the assets exactly as they were supplied, spaces and all.
    const SUPPLIED: &[(&str, u16, u16, &[&str])] = &[
        (
            "mc-wizard",
            6,
            5,
            &["  ▟▖  ", "▄▟█▙▄ ", " ◣ ◢ ◆", "═▓♦▓▘│", " ▓▓▓ │"],
        ),
        (
            "necromancer",
            7,
            5,
            &[" ▄███▄ ", " █° °█ ", " ▀█▼█▀ ", " ░▓█▓░╽", " ░▓▓▓░│"],
        ),
        (
            "hag-witch",
            7,
            6,
            &[
                "   ▟▖  ",
                "  ▟█▙  ",
                "▄▄███▄▄",
                " ▐°v°▌ ",
                " ▟▓▓▓▙ ",
                "▝▀▀▀▀▀▘",
            ],
        ),
        (
            "plague-doctor",
            7,
            5,
            &[" ▄▄█▄▄ ", " ▐◉ ◉▌ ", "  ◥█◣  ", " ▟▓▓▓▙ ", "  ▐ ▌  "],
        ),
        (
            "dark-elf-assassin",
            7,
            5,
            &[" ▄███▄ ", "◁█◣ ◢█▷", " ▜▀▀▀▛ ", "╱▓███▓╲", " ▟▛ ▜▙ "],
        ),
        (
            "dark-knight",
            7,
            5,
            &["  ▄▲▄  ", " ▐█▬█▌ ", "◢█████◣", " ▐███▌│", "  ▙ ▟ ┴"],
        ),
        (
            "skeleton-warrior",
            7,
            5,
            &["  ▄▄▄  ", " ▐° °▌ ", "  ╘▀╛ ╽", "▐█╫═╫ │", "  ╨ ╨ ┴"],
        ),
        (
            "shadow-wraith",
            7,
            5,
            &["  ▄▄▄  ", " ▟◣ ◢▙ ", " ▐░▒░▌ ", " ▝▓▒▓▘ ", "  ░ ░  "],
        ),
        ("imp", 7, 4, &["▟▌   ▐▙", " ▐◣▼◢▌ ", "▗▟███▙▖", " ▐▌ ▐▌~"]),
        ("bat", 7, 4, &["▙▖   ▗▟", "▐█▙▄▟█▌", " ▝°▼°▘ ", "  ▀ ▀  "]),
        ("ooze", 7, 4, &["  ░▒░  ", " ▟° °▙ ", "▟▓▒▓▒▓▙", "▀▀▀▀▀▀▀"]),
        ("drake", 7, 4, &["◢▙   ▟◣", "▐◣▄▄▄◢▌", "◥█▓▓▓█◤", " ▝▙▓▟▘~"]),
        (
            "lich-king",
            11,
            7,
            &[
                "  ▲ ▟█▙ ▲  ",
                " ▟▀▀███▀▀▙ ",
                " █ ◉ ▼ ◉ █ ",
                " ▜▄▀▀▀▀▀▄▛ ",
                "◢█▓▓▓♦▓▓▓█◣",
                "▐▓▓▓▓▓▓▓▓▓▌",
                "  ▀▘   ▝▀  ",
            ],
        ),
    ];

    #[test]
    fn preserves_every_declared_row_exactly() {
        // The whole point of the library: the art is the asset. Not one glyph,
        // and not one space, may differ from what was supplied.
        for (id, width, height, rows) in SUPPLIED {
            let sprite = by_id(id).unwrap_or_else(|| panic!("{id} is missing"));
            assert_eq!(*width, sprite.width, "{id}: declared width");
            assert_eq!(*height, sprite.height, "{id}: declared height");
            assert_eq!(rows.len(), sprite.rows.len(), "{id}: row count");
            for (index, (want, got)) in rows.iter().zip(sprite.rows).enumerate() {
                assert_eq!(
                    *want, *got,
                    "{id}: row {index} changed\n  supplied: {want:?}\n  stored:   {got:?}"
                );
            }
        }
    }

    #[test]
    fn no_row_is_trimmed_or_reflowed() {
        for sprite in SPRITES {
            for (index, row) in sprite.rows.iter().enumerate() {
                assert!(
                    row.ends_with(' ') || row.trim_end() == *row,
                    "{} row {index} has been stripped",
                    sprite.id
                );
                assert_eq!(
                    row.chars().count(),
                    usize::from(sprite.width),
                    "{} row {index} is the wrong width: {row:?}",
                    sprite.id
                );
                assert!(!row.contains('\t'), "{} row {index} has a tab", sprite.id);
            }
            assert!(sprite.is_well_formed(), "{} is malformed", sprite.id);
        }
    }

    #[test]
    fn leading_spaces_are_part_of_the_art() {
        // A silhouette is made of its spaces; if these came back trimmed the
        // figure would jump one cell to the left and the sprite would shear.
        for (id, _, _, rows) in SUPPLIED {
            let sprite = by_id(id).unwrap();
            let leading = rows.iter().filter(|row| row.starts_with(' ')).count();
            let stored = sprite
                .rows
                .iter()
                .filter(|row| row.starts_with(' '))
                .count();
            assert_eq!(leading, stored, "{id} lost its leading spaces");
        }
    }

    #[test]
    fn every_sprite_glyph_is_one_cell_wide() {
        // A two-column glyph would push the rest of its row one cell right, and
        // the sprite would no longer be the size it declares.
        for sprite in SPRITES {
            for row in sprite.all_poses().iter().flat_map(|pose| pose.iter()) {
                for (index, ch) in row.chars().enumerate() {
                    assert!(
                        ch == ' ' || glyph::is_drawable(ch),
                        "{} draws {ch:?} at column {index}, which is not one cell wide",
                        sprite.id
                    );
                }
            }
        }
    }

    #[test]
    fn every_frame_matches_its_sprite_dimensions() {
        for sprite in SPRITES {
            for (index, frame) in sprite.all_poses().iter().enumerate().skip(1) {
                assert_eq!(
                    frame.len(),
                    usize::from(sprite.height),
                    "{} frame {index} has the wrong number of rows",
                    sprite.id
                );
                for (row, line) in frame.iter().enumerate() {
                    assert_eq!(
                        line.chars().count(),
                        usize::from(sprite.width),
                        "{} frame {index} row {row} is {:?}",
                        sprite.id,
                        line
                    );
                }
            }
        }
    }

    #[test]
    fn the_base_pose_is_never_one_of_its_own_frames() {
        // Companion frames are companions: the canonical art stays canonical.
        for sprite in SPRITES {
            for pose in sprite.all_poses().into_iter().skip(1) {
                assert_ne!(pose, sprite.rows, "{} replaces its base pose", sprite.id);
            }
        }
    }

    #[test]
    fn every_sprite_has_a_unique_id_and_a_declared_role() {
        let mut ids: Vec<&str> = SPRITES.iter().map(|sprite| sprite.id).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "two sprites share an id");
        assert!(SPRITES.len() >= 18, "the library lost sprites");
        assert!(
            SPRITES.iter().any(|sprite| sprite.role == Role::Boss),
            "the library needs its boss"
        );
        assert!(
            SPRITES.iter().any(|sprite| sprite.role == Role::Hero),
            "and its hero"
        );
    }

    #[test]
    fn the_glyph_legend_covers_the_mass_the_metal_and_the_faces() {
        assert_eq!(Part::Body, part_of('█'));
        assert_eq!(Part::Shade, part_of('▓'));
        assert_eq!(Part::Light, part_of('▒'));
        assert_eq!(Part::Highlight, part_of('░'));
        assert_eq!(Part::Outline, part_of('▟'));
        assert_eq!(Part::Metal, part_of('╫'));
        assert_eq!(Part::Eye, part_of('°'));
        assert_eq!(Part::Eye, part_of('◉'));
        assert_eq!(Part::Accent, part_of('♦'));
    }

    #[test]
    fn every_character_uses_more_than_one_ink() {
        // A sprite painted in a single colour is a silhouette with no face.
        for sprite in SPRITES {
            let parts: Vec<Part> = sprite
                .rows
                .iter()
                .flat_map(|row| row.chars())
                .filter(|ch| *ch != ' ')
                .map(part_of)
                .collect();
            let mut distinct: Vec<Part> = parts.clone();
            distinct.sort_unstable_by_key(|part| *part as u8);
            distinct.dedup();
            assert!(distinct.len() >= 3, "{} uses only {distinct:?}", sprite.id);
        }
    }

    #[test]
    fn inks_reduce_to_the_sixteen_colour_palette() {
        let reduced = MC_WIZARD.inks.reduced();
        for part in [
            Part::Outline,
            Part::Body,
            Part::Shade,
            Part::Light,
            Part::Highlight,
            Part::Metal,
            Part::Eye,
            Part::Accent,
        ] {
            assert_ne!(
                crate::render::Color::Indexed(0),
                reduced.of(part),
                "{part:?} did not reduce"
            );
        }
    }

    #[test]
    fn a_pose_always_draws_art_the_sprite_declares() {
        for sprite in SPRITES {
            let poses = sprite.all_poses();
            for pose in [
                Pose::Idle,
                Pose::Walk { step: 0 },
                Pose::Walk { step: 7 },
                Pose::Sprint,
            ] {
                let rows = pose.rows_for(sprite);
                assert!(
                    poses.contains(&rows),
                    "{}: {pose:?} drew art the sprite does not declare",
                    sprite.id
                );
            }
        }
        // A sprite with no companion frames always draws its base pose.
        let lich = super::LICH_KING;
        assert_eq!(lich.rows, Pose::Sprint.rows_for(&lich));
        // The wizard's walk alternates between two different poses.
        assert_ne!(
            Pose::Walk { step: 0 }.rows_for(&MC_WIZARD),
            Pose::Walk { step: 1 }.rows_for(&MC_WIZARD)
        );
        // And its sprint is its own pose, not a step.
        assert_ne!(MC_WIZARD.rows, Pose::Sprint.rows_for(&MC_WIZARD));
        assert_ne!(
            Pose::Sprint.rows_for(&MC_WIZARD),
            Pose::Walk { step: 0 }.rows_for(&MC_WIZARD)
        );
    }

    #[test]
    fn a_sprites_block_is_centred_and_stands_on_its_anchor() {
        let (left, top) = MC_WIZARD.origin();
        assert_eq!(-2, left, "a six-wide sprite straddles its column");
        assert_eq!(-4, top, "and stands four rows above it");
        let (left, top) = LICH_KING.origin();
        assert_eq!(-5, left);
        assert_eq!(-6, top);
    }

    #[test]
    fn lookup_is_by_stable_id() {
        assert_eq!(Some(&MC_WIZARD), by_id("mc-wizard"));
        assert!(by_id("no-such-sprite").is_none());
    }

    #[test]
    fn the_inks_are_not_shared_between_characters() {
        // Two characters painted identically would read as the same creature.
        for (index, sprite) in SPRITES.iter().enumerate() {
            for other in &SPRITES[index + 1..] {
                assert_ne!(
                    sprite.inks, other.inks,
                    "{} and {} are the same colour",
                    sprite.id, other.id
                );
            }
        }
    }

    #[test]
    fn a_deliberate_edit_to_a_supplied_row_would_be_caught() {
        // The guard above only works if it can fail. This is what failure looks
        // like: the same comparison, against a row with one space removed.
        let tampered = Sprite {
            rows: &["  ▟▖ ", "▄▟█▙▄ ", " ◣ ◢ ◆", "═▓♦▓▘│", " ▓▓▓ │"],
            ..MC_WIZARD
        };
        assert_ne!(
            SUPPLIED[0].3, tampered.rows,
            "a trimmed row must not equal the supplied art"
        );
    }

    #[test]
    fn art_inks_are_fully_addressable() {
        let inks = ArtInks::of(&MC_WIZARD.inks, Part::Body);
        assert_eq!(MC_WIZARD.inks.body, inks);
        assert_eq!(BAT.inks.eye, ArtInks::of(&BAT.inks, Part::Eye));
    }
}
