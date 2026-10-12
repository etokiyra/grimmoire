//! The cast: the one place that decides how a role the game model named is drawn.
//!
//! Every character in the world is a [`Figure`] — a role named by the game model
//! — and this module is the single place that decides what that role *looks*
//! like. One table holds each figure's ASCII art, one describes the inks it is
//! drawn in, and one holds its dialogue bust.
//!
//! Keeping it in one place is what makes the boundary real: the world model can
//! add a character by naming a role, the compiler then demands an entry here, and
//! nothing in the renderer has to learn a special case.
//!
//! # The legend
//!
//! A figure's rows are ASCII, and the characters in them are a *legend*: each
//! names an ink rather than a picture. `^` is hair, `o` is an eye, `#` is cloth,
//! and so on — see `ink_for` in [`crate::ui::scenery`]. That is what lets one
//! small figure have pink ears, a pale face, a violet robe and a bright wand at
//! once, and it is why a character can be recoloured without touching a glyph.
//!
//! # A figure, row by row
//!
//! A figure is five cells wide and four rows tall: two rows of head for each of
//! the four facings, then a body row and a hem row. The two profiles are authored
//! rather than mirrored, so the eye always sits toward the direction the
//! character faces — that is the only cue a two-row head needs to read as turned
//! instead of as missing an eye. The body and hem each come in a still and a
//! swaying pair, so a step makes the garment move.
//!
//! A [`Bust`] is the same character larger and front on, shown beside their
//! words. Its third row is the face, and it is swapped for one of four eye rows
//! to change the expression; the rest of the portrait is what makes the speaker
//! recognisable at a glance.

use super::theme::{Inks, Palette};
use crate::game::{Figure, Mood};

/// The portrait shown beside a character's words.
#[derive(Debug)]
pub struct Bust {
    /// Eight rows of twelve cells. Row 3 is replaced at draw time by the eye row
    /// for the speaker's current expression.
    pub rows: [&'static str; 8],
    /// The eye row for `[calm, warm, wary, sorry]`.
    pub eyes: [&'static str; 4],
}

/// One character's art.
#[derive(Debug)]
pub struct CharacterArt {
    /// Two head rows for `[north, east, south, west]`, five cells each.
    pub heads: [[&'static str; 2]; 4],
    /// The torso, still and swaying.
    pub bodies: [&'static str; 2],
    /// The hem, still and swaying.
    pub hems: [&'static str; 2],
    /// Something carried beside the figure, if anything: two rows, one cell each.
    pub held: Option<[&'static str; 2]>,
    /// The dialogue portrait, for characters who speak.
    pub bust: Option<Bust>,
}

/// The player: a kawaii mage bunny with two long ears, big round eyes, a violet
/// robe with white lace, and a star-tipped wand.
const MAGE: CharacterArt = CharacterArt {
    heads: [
        [" ^ ^ ", " ~~~ "],
        ["  ^^ ", "( :o)"],
        [" ^ ^ ", "(o:o)"],
        [" ^^  ", "(o: )"],
    ],
    bodies: ["/#+#\\", "\\#+#/"],
    hems: ["/###\\", "\\###/"],
    held: Some(["*", "|"]),
    bust: None,
};

/// Ysolde: a deep cowl with no face at all, only two cold points of light in a
/// void, robed to the ground.
const VEILED: CharacterArt = CharacterArt {
    heads: [
        [" ___ ", " %%% "],
        [" ___ ", " (@o)"],
        [" ___ ", "(o@o)"],
        [" ___ ", "(o@ )"],
    ],
    bodies: ["/%%%\\", "\\%%%/"],
    hems: ["/###\\", "\\###/"],
    held: None,
    bust: Some(Bust {
        rows: [
            "   _____    ",
            "  /     \\   ",
            " |       |  ",
            " |  o o  |  ",
            " |       |  ",
            "  \\     /   ",
            " /#######\\  ",
            "|#########| ",
        ],
        eyes: [
            " |  o o  |  ",
            " |  v v  |  ",
            " |  @ o  |  ",
            " |  @ @  |  ",
        ],
    }),
};

/// Rin: a small, bright keeper under a patched hood, carrying a lantern whose
/// light burns above it.
const LANTERN: CharacterArt = CharacterArt {
    heads: [
        [" _^_ ", " ### "],
        [" _^_ ", " (:o)"],
        [" _^_ ", "(o.o)"],
        [" _^_ ", "(o: )"],
    ],
    bodies: ["/#+#\\", "\\#+#/"],
    hems: ["/%#%\\", "\\%#%/"],
    held: Some(["*", "="]),
    bust: Some(Bust {
        rows: [
            "   ,^,      ",
            "  /___\\     ",
            " |     |    ",
            " | o o |    ",
            "  \\___/     ",
            " /#+#+#\\    ",
            "|##+#+##|   ",
            " \\#+#+#/    ",
        ],
        eyes: [
            " | o o |    ",
            " | v v |    ",
            " | @ o |    ",
            " | @ @ |    ",
        ],
    }),
};

/// Cassen: hunched and weathered, with a bundle on his back, a squinting face,
/// a patched dark cloak, and a walking stick.
const SCAVENGER: CharacterArt = CharacterArt {
    heads: [
        [" _,_ ", " %%% "],
        [" _,_ ", " (_o)"],
        [" _,_ ", "(o_o)"],
        [" _,_ ", "(o_ )"],
    ],
    bodies: ["/%#%\\", "\\%#%/"],
    hems: ["/%%%\\", "\\%%%/"],
    held: Some(["|", "|"]),
    bust: Some(Bust {
        rows: [
            "  ,___,     ",
            " /     \\    ",
            " |     |    ",
            " | o_o |    ",
            "  \\___/     ",
            " /%%%=\\     ",
            "|%%=%#%|    ",
            " \\%%%%%/    ",
        ],
        eyes: [
            " | o_o |    ",
            " | v_v |    ",
            " | @_o |    ",
            " | ___ |    ",
        ],
    }),
};

/// Ioreth: tall and serene under a bright diadem, an indigo robe with silver,
/// robed to the floor.
const PRIESTESS: CharacterArt = CharacterArt {
    heads: [
        [" +_+ ", " ### "],
        [" +_+ ", " (:o)"],
        [" +_+ ", "(v:v)"],
        [" +_+ ", "(o: )"],
    ],
    bodies: ["/=#=\\", "\\=#=/"],
    hems: ["/###\\", "\\###/"],
    held: None,
    bust: Some(Bust {
        rows: [
            "   +_+      ",
            "  /===\\     ",
            " |     |    ",
            " | o o |    ",
            "  \\___/     ",
            " /=====\\    ",
            "|===+===|   ",
            " \\=====/    ",
        ],
        eyes: [
            " | o o |    ",
            " | v v |    ",
            " | @ o |    ",
            " | @ @ |    ",
        ],
    }),
};

/// Vesper: grave-wrappings, a bone face with two cold lights, and bands across
/// the body.
const REVENANT: CharacterArt = CharacterArt {
    heads: [
        [" _%_ ", " %%% "],
        [" _%_ ", " (@v)"],
        [" _%_ ", "(v@v)"],
        [" _%_ ", "(v@ )"],
    ],
    bodies: ["/=%=\\", "\\=%=/"],
    hems: ["/%+%\\", "\\%+%/"],
    held: None,
    bust: Some(Bust {
        rows: [
            "   _%_      ",
            "  /%%%\\     ",
            " |     |    ",
            " | o o |    ",
            "  \\_:_/     ",
            " /%=%=%\\    ",
            "|%=%=%|     ",
            " \\%_%_%/    ",
        ],
        eyes: [
            " | o o |    ",
            " | v v |    ",
            " | @ o |    ",
            " | @ @ |    ",
        ],
    }),
};

/// Nim, the shrine apprentice: magical ribbons in her hair, a big grin, a short
/// dress with a wide sash, and a charm hanging on a string.
const APPRENTICE: CharacterArt = CharacterArt {
    heads: [
        [" ^=^ ", " ~~~ "],
        [" ^=^ ", " (:o)"],
        [" ^=^ ", "(o,o)"],
        [" ^=^ ", "(o: )"],
    ],
    bodies: ["/=#=\\", "\\=#=/"],
    hems: ["/#=#\\", "\\#=#/"],
    held: Some(["+", "|"]),
    bust: Some(Bust {
        rows: [
            "  =^=^=     ",
            " /     \\    ",
            " |     |    ",
            " | o o |    ",
            "  \\_o_/     ",
            " /=====\\    ",
            "|==+==|     ",
            " \\=====/    ",
        ],
        eyes: [
            " | o o |    ",
            " | v v |    ",
            " | @ o |    ",
            " | @ @ |    ",
        ],
    }),
};

/// Maren, the wanderer: an enormous wide-brimmed pointed hat, a face mostly in
/// its shadow, a travelling cloak, and a long staff.
const WANDERER: CharacterArt = CharacterArt {
    heads: [
        [" /#\\ ", " ~~~ "],
        [" /#\\ ", " (@v)"],
        [" /#\\ ", "(v_v)"],
        [" /#\\ ", "(v@ )"],
    ],
    bodies: ["/%#%\\", "\\%#%/"],
    hems: ["/##%\\", "\\%##/"],
    held: Some(["|", "|"]),
    bust: Some(Bust {
        rows: [
            "   /^\\      ",
            "  /###\\     ",
            " ###=###    ",
            "  @o o@     ",
            "  \\###/     ",
            " /#####\\    ",
            "|#######|   ",
            " \\#####/    ",
        ],
        eyes: [
            "  @o o@     ",
            "  @v v@     ",
            "  @o  @     ",
            "  @@ @@     ",
        ],
    }),
};

/// Wren, the forest spirit: a rounded, spotted mushroom cap, hands clasped low,
/// a mossy hem, and a small face with wide, cautious eyes.
const SPIRIT: CharacterArt = CharacterArt {
    heads: [
        [" _+_ ", " %%% "],
        [" _+_ ", " (:o)"],
        [" _+_ ", "(o o)"],
        [" _+_ ", "(o: )"],
    ],
    bodies: ["/#:#\\", "\\#:#/"],
    hems: ["/%#%\\", "\\%#%/"],
    held: None,
    bust: Some(Bust {
        rows: [
            "   ___      ",
            "  /_+_\\     ",
            " /%%%%%\\    ",
            "  o   o     ",
            "   |:|      ",
            " /%%%%%\\    ",
            "|%%%%%%%|   ",
            " \\%%%%%/    ",
        ],
        eyes: [
            "  o   o     ",
            "  v   v     ",
            "  o   @     ",
            "  @   @     ",
        ],
    }),
};

/// Tamsin, the catacomb warden: a plumed helmet, a narrow visor showing two
/// eyes, steel pauldrons, and a spear.
const WARDEN: CharacterArt = CharacterArt {
    heads: [
        [" ~^~ ", " %%% "],
        [" ~^~ ", " (@o)"],
        [" ~^~ ", "(o=o)"],
        [" ~^~ ", "(o@ )"],
    ],
    bodies: ["/=#=\\", "\\=#=/"],
    hems: ["/###\\", "\\###/"],
    held: Some(["+", "|"]),
    bust: Some(Bust {
        rows: [
            "   ,^,      ",
            "  /###\\     ",
            " |=====|    ",
            " | o o |    ",
            "  \\___/     ",
            " /=====\\    ",
            "|===+===|   ",
            " \\=====/    ",
        ],
        eyes: [
            " | o o |    ",
            " | v v |    ",
            " | @ o |    ",
            " | @ @ |    ",
        ],
    }),
};

/// The art for a figure.
///
/// Total by construction: every [`Figure`] has an entry, so a character the model
/// can name is always a character the renderer can draw.
#[must_use]
pub const fn art(figure: Figure) -> &'static CharacterArt {
    match figure {
        Figure::Mage => &MAGE,
        Figure::Veiled => &VEILED,
        Figure::Lantern => &LANTERN,
        Figure::Scavenger => &SCAVENGER,
        Figure::Priestess => &PRIESTESS,
        Figure::Revenant => &REVENANT,
        Figure::Apprentice => &APPRENTICE,
        Figure::Wanderer => &WANDERER,
        Figure::Spirit => &SPIRIT,
        Figure::Warden => &WARDEN,
    }
}

/// The inks a figure is drawn in, for this palette.
#[must_use]
pub const fn inks(palette: &Palette, figure: Figure) -> Inks {
    match figure {
        Figure::Mage => palette.mage,
        Figure::Veiled => palette.veiled,
        Figure::Lantern => palette.lantern,
        Figure::Scavenger => palette.scavenger,
        Figure::Priestess => palette.priestess,
        Figure::Revenant => palette.revenant,
        Figure::Apprentice => palette.apprentice,
        Figure::Wanderer => palette.wanderer,
        Figure::Spirit => palette.spirit,
        Figure::Warden => palette.warden,
    }
}

/// Which of a [`Bust`]'s four eye rows an expression wears.
///
/// The cast distinguishes four faces — `[calm, warm, wary, sorry]` — but the
/// model carries more moods than that, so the extras fold onto the nearest one.
#[must_use]
pub const fn eye_row(mood: Mood) -> usize {
    match mood {
        Mood::Calm => 0,
        Mood::Warm | Mood::Wry => 1,
        Mood::Wary | Mood::Veiled => 2,
        Mood::Sorry | Mood::Shaken => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::{art, eye_row, inks};
    use crate::game::{Figure, Mood};
    use crate::render::glyph;
    use crate::ui::theme::Palette;

    /// Every row of a figure's art, held item and bust included.
    fn all_rows(figure: Figure) -> Vec<&'static str> {
        let art = art(figure);
        let mut rows: Vec<&'static str> = art
            .heads
            .iter()
            .flat_map(|facing| facing.iter().copied())
            .chain(art.bodies.iter().copied())
            .chain(art.hems.iter().copied())
            .collect();
        if let Some(held) = art.held {
            rows.extend(held.iter().copied());
        }
        if let Some(bust) = &art.bust {
            rows.extend(bust.rows.iter().copied());
            rows.extend(bust.eyes.iter().copied());
        }
        rows
    }

    /// The columns of a row that are not an eye, for alignment checks.
    fn is_eye(ch: char) -> bool {
        matches!(ch, 'o' | 'v' | '●' | '-')
    }

    #[test]
    fn every_head_is_two_rows_of_five_cells() {
        for figure in Figure::ALL {
            for facing in art(figure).heads {
                assert_eq!(
                    2,
                    facing.len(),
                    "{figure:?} has a {}-row head",
                    facing.len()
                );
                for row in facing {
                    assert_eq!(
                        5,
                        row.chars().count(),
                        "{figure:?} has a head row that is not five cells: {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_body_and_hem_is_five_cells() {
        for figure in Figure::ALL {
            let art = art(figure);
            for row in art.bodies.iter().chain(art.hems.iter()) {
                assert_eq!(
                    5,
                    row.chars().count(),
                    "{figure:?} has a body or hem that is not five cells: {row:?}"
                );
            }
        }
    }

    #[test]
    fn every_held_item_is_two_single_cell_rows() {
        for figure in Figure::ALL {
            let Some(held) = art(figure).held else {
                continue;
            };
            for row in held {
                assert_eq!(1, row.chars().count(), "{figure:?} holds {row:?}");
            }
        }
    }

    #[test]
    fn every_bust_is_eight_rows_of_twelve_cells() {
        for figure in Figure::ALL {
            let Some(bust) = &art(figure).bust else {
                continue;
            };
            assert_eq!(8, bust.rows.len(), "{figure:?} has a broken bust");
            for row in bust.rows {
                assert_eq!(
                    12,
                    row.chars().count(),
                    "{figure:?} has a bust row that is not twelve cells: {row:?}"
                );
            }
        }
    }

    #[test]
    fn every_bust_has_four_eye_rows_of_twelve_cells() {
        for figure in Figure::ALL {
            let Some(bust) = &art(figure).bust else {
                continue;
            };
            assert_eq!(4, bust.eyes.len(), "{figure:?} has the wrong eye rows");
            for row in bust.eyes {
                assert_eq!(
                    12,
                    row.chars().count(),
                    "{figure:?} has an eye row that is not twelve cells: {row:?}"
                );
            }
        }
    }

    #[test]
    fn every_eye_row_lines_up_with_the_face_row() {
        // An eye row is a drop-in replacement for row 3, so every cell that is
        // not one of the four eye glyphs must be exactly what row 3 already has.
        for figure in Figure::ALL {
            let Some(bust) = &art(figure).bust else {
                continue;
            };
            let face: Vec<char> = bust.rows[3].chars().collect();
            for eyes in bust.eyes {
                let eyes: Vec<char> = eyes.chars().collect();
                assert_eq!(face.len(), eyes.len(), "{figure:?}");
                for (column, (placeholder, actual)) in face.iter().zip(&eyes).enumerate() {
                    if !is_eye(*placeholder) {
                        assert_eq!(
                            placeholder, actual,
                            "{figure:?} eye row moves column {column} off the face"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn all_art_is_drawable() {
        // A glyph two columns wide would desynchronise the cell grid, and a
        // zero-width one would silently vanish. Everything is plain ASCII, so
        // the only real risk is a glyph that is not in the legend at all.
        for figure in Figure::ALL {
            for row in all_rows(figure) {
                for ch in row.chars() {
                    assert!(
                        ch.is_ascii() && !ch.is_ascii_control(),
                        "{figure:?} draws non-ASCII {ch:?} in {row:?}"
                    );
                    assert!(
                        ch == ' ' || glyph::is_drawable(ch),
                        "{figure:?} draws undrawable {ch:?} in {row:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn characters_that_speak_have_a_bust() {
        for figure in Figure::ALL {
            if figure == Figure::Mage {
                assert!(
                    art(figure).bust.is_none(),
                    "the player never speaks, so has no bust"
                );
                continue;
            }
            assert!(
                art(figure).bust.is_some(),
                "{figure:?} speaks but has no bust"
            );
        }
    }

    #[test]
    fn every_figure_has_its_own_inks() {
        // Two characters sharing a palette would be a mistake that is easy to
        // make and hard to see, so it is checked rather than trusted.
        let palette = Palette::EXTENDED;
        for (index, figure) in Figure::ALL.iter().enumerate() {
            for other in &Figure::ALL[index + 1..] {
                assert_ne!(
                    inks(&palette, *figure),
                    inks(&palette, *other),
                    "{figure:?} and {other:?} look identical"
                );
            }
        }
    }

    #[test]
    fn the_faces_are_distinct_from_one_another() {
        // Two figures whose *full face* is identical would read as the same
        // character, however different their inks were.
        for (index, figure) in Figure::ALL.iter().enumerate() {
            for other in &Figure::ALL[index + 1..] {
                assert_ne!(
                    art(*figure).heads[2],
                    art(*other).heads[2],
                    "{figure:?} and {other:?} have the same face"
                );
            }
        }
    }

    #[test]
    fn every_figure_turns_in_four_directions() {
        // A figure whose north and south heads were the same would never look
        // turned; the profiles must differ too, and both from each other.
        for figure in Figure::ALL {
            let heads = art(figure).heads;
            for (index, facing) in heads.iter().enumerate() {
                for other in &heads[index + 1..] {
                    assert_ne!(
                        facing, other,
                        "{figure:?} has two facings that look the same"
                    );
                }
            }
        }
    }

    #[test]
    fn every_mood_maps_to_an_eye_row() {
        for mood in [
            Mood::Calm,
            Mood::Warm,
            Mood::Wary,
            Mood::Sorry,
            Mood::Wry,
            Mood::Veiled,
            Mood::Shaken,
        ] {
            assert!(eye_row(mood) < 4, "{mood:?} has no eye row");
        }
        assert_eq!(0, eye_row(Mood::Calm));
        assert_eq!(1, eye_row(Mood::Warm));
        assert_eq!(2, eye_row(Mood::Wary));
        assert_eq!(3, eye_row(Mood::Sorry));
    }
}
