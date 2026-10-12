//! The cast: how each figure is drawn.
//!
//! Every character in the world is a [`Figure`] — a role named by the game model
//! — and this module is the single place that decides what a role *looks* like.
//! One table holds each figure's ASCII art, one describes the inks it is drawn
//! in, and one holds its dialogue bust.
//!
//! Keeping it in one place is what makes the boundary real: the world model can
//! add a character by naming a role, the compiler then demands an entry here, and
//! nothing in the renderer has to learn a special case. Before this, every NPC
//! was drawn with the same hardcoded silhouette.
//!
//! # The legend
//!
//! A figure's rows are ASCII, and the characters in them are a *legend*: each
//! names an ink rather than a picture. `^` is hair, `o` is an eye, `#` is cloth,
//! and so on — see `ink_for` in [`crate::ui::scenery`]. That is what lets one
//! small figure have pink ears, a pale face, a violet robe and a bright wand at
//! once, and it is why a character can be recoloured without touching a glyph.

use super::theme::{Inks, Palette};
use crate::game::Figure;

/// One character's art.
#[derive(Debug)]
pub struct CharacterArt {
    /// Two head rows for `[north, east, south, west]`, five cells wide.
    ///
    /// The profiles are authored rather than mirrored: the eye sits toward the
    /// direction the character faces, which is the only cue that makes a two-row
    /// head read as turned rather than as missing an eye.
    pub heads: [[&'static str; 2]; 4],
    /// The hem, and the same hem swished, for the walk.
    pub hems: [&'static str; 2],
    /// Something held out to one side: a wand, a lantern, a stick.
    pub held: Option<[&'static str; 2]>,
    /// The dialogue bust, for characters who speak.
    ///
    /// A larger, front-on version of the same character, so a conversation shows
    /// the face of whoever is talking.
    pub bust: Option<[&'static str; 8]>,
}

/// The mage bunny: two long ears, big eyes, a laced robe, and a star wand.
const MAGE: CharacterArt = CharacterArt {
    heads: [
        [" ^ ^ ", " ~~~ "],
        ["  ^^ ", "( :●)"],
        [" ^ ^ ", "(●:●)"],
        [" ^^  ", "(●: )"],
    ],
    hems: ["/#+#\\", "\\#+#/"],
    held: Some(["*", "|"]),
    bust: None,
};

/// Ysolde: a deep cowl with no face, only two cold points of light.
const VEILED: CharacterArt = CharacterArt {
    heads: [
        [" ___ ", " ### "],
        [" ___ ", "( @o)"],
        [" ___ ", "(o@o)"],
        [" ___ ", "(o@ )"],
    ],
    hems: ["/###\\", "\\###/"],
    held: None,
    bust: Some([
        "   _____   ",
        "  /     \\  ",
        " |  o o  | ",
        "  \\_____/  ",
        " /#######\\ ",
        "|#########|",
        " \\#######/ ",
        "    | |    ",
    ]),
};

/// Rin: a small keeper under a patched hood, carrying a lantern.
const LANTERN: CharacterArt = CharacterArt {
    heads: [
        [" ,^, ", " ~~~ "],
        [" ,^, ", "( o.)"],
        [" ,^, ", "(o.o)"],
        [" ,^, ", "(.o )"],
    ],
    hems: ["/#+#\\", "\\#+#/"],
    held: Some(["+", "|"]),
    bust: Some([
        "   ,^,    ",
        "  /___\\   ",
        " | o o |  ",
        "  \\___/   ",
        " /#+#+#\\  ",
        "|##+#+##| ",
        " \\#+#+#/  ",
        "   | |    ",
    ]),
};

/// Cassen: a hunched scavenger with a bundle on his back and a stick.
const SCAVENGER: CharacterArt = CharacterArt {
    heads: [
        [" _,_ ", " %%% "],
        [" _,_ ", "(o_ )"],
        [" _,_ ", "(o_o)"],
        [" _,_ ", "( _o)"],
    ],
    hems: ["/%%%\\", "\\%%%/"],
    held: Some(["|", "'"]),
    bust: Some([
        "   _,_    ",
        "  /___\\   ",
        " | o_o |  ",
        "  \\___/   ",
        " /%%%%%\\  ",
        "|%%%=%%%| ",
        " \\%%%%%/  ",
        "  |   |   ",
    ]),
};

/// Ioreth: a tall priestess under a silver diadem, robed to the floor.
const PRIESTESS: CharacterArt = CharacterArt {
    heads: [
        [" +_+ ", " ~~~ "],
        [" +_+ ", "(o: )"],
        [" +_+ ", "(o:o)"],
        [" +_+ ", "( :o)"],
    ],
    hems: ["/===\\", "\\===/"],
    held: None,
    bust: Some([
        "   +_+    ",
        "  /=== \\  ",
        " | o o |  ",
        "  \\___/   ",
        " /=====\\  ",
        "|===+===| ",
        " \\=====/  ",
        "  |   |   ",
    ]),
};

/// Vesper: a revenant wound in grave-wrappings, two lights in a bone face.
const REVENANT: CharacterArt = CharacterArt {
    heads: [
        [" _%_ ", " %%% "],
        [" _%_ ", "(@: )"],
        [" _%_ ", "(@:@)"],
        [" _%_ ", "( :@)"],
    ],
    hems: ["/|%|\\", "\\|%|/"],
    held: None,
    bust: Some([
        "   _%_    ",
        "  /%%%\\   ",
        " | @ @ |  ",
        "  \\_:_/   ",
        " /|%|%|\\  ",
        "|%|%:%|%| ",
        " \\|%|%|/  ",
        "  |   |   ",
    ]),
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
    }
}

#[cfg(test)]
mod tests {
    use super::{art, inks};
    use crate::game::Figure;
    use crate::render::glyph;
    use crate::ui::theme::Palette;

    #[test]
    fn every_head_is_two_rows_of_five_cells() {
        for figure in Figure::ALL {
            for facing in art(figure).heads {
                assert_eq!(2, facing.len());
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
    fn every_hem_is_five_cells() {
        for figure in Figure::ALL {
            for hem in art(figure).hems {
                assert_eq!(
                    5,
                    hem.chars().count(),
                    "{figure:?} has a hem that is not five cells: {hem:?}"
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
    fn all_art_is_drawable() {
        // A glyph two columns wide would desynchronise the cell grid, and a
        // zero-width one would silently vanish.
        for figure in Figure::ALL {
            let art = art(figure);
            let rows = art
                .heads
                .iter()
                .flat_map(|facing| facing.iter())
                .chain(art.hems.iter())
                .chain(art.bust.iter().flat_map(|bust| bust.iter()));
            for row in rows {
                for ch in row.chars() {
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
        for figure in [
            Figure::Veiled,
            Figure::Lantern,
            Figure::Scavenger,
            Figure::Priestess,
            Figure::Revenant,
        ] {
            let bust = art(figure)
                .bust
                .unwrap_or_else(|| panic!("{figure:?} speaks but has no bust"));
            assert_eq!(8, bust.len(), "{figure:?} has a {}-row bust", bust.len());
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
}
