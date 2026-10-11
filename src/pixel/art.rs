//! Grimmoire's pixel art.
//!
//! Every sprite is authored here as ASCII rows plus a small character palette,
//! so the art is text: it reads in the source, it diffs, and it is edited with
//! the same tooling as the rest of the project. Nothing is downloaded and nothing
//! is baked into a binary blob outside version control.
//!
//! # Adding art
//!
//! 1. Add the rows and, if it needs new colours, extend a palette below.
//! 2. Add a field to [`Artwork`] and build it in [`Artwork::build`].
//! 3. `cargo run --example art_sheet` renders every sprite to a PNG contact
//!    sheet, which is the quickest way to see whether a change actually reads.
//!
//! # Palette characters
//!
//! The legend is shared across sprites so a colour means one thing everywhere:
//!
//! * `.` transparent, `K` outline
//! * `H`/`h` hood cloth, `S`/`s` skin, `E` eye glow
//! * `R`/`r` robe, `G`/`g` gold trim, `F` boots
//! * `W`/`w` water, `C`/`c` cloak, `D`/`d` cloth shadow
//! * `T`/`t` wood, `M`/`m` magic, `Y`/`y` bone, `N`/`n` moss

use std::sync::OnceLock;

use super::canvas::Rgb;
use super::sprite::Sprite;

/// The hooded figure: a dark cloak and two points of cold light.
const SHADE: &[(char, Rgb)] = &[
    ('K', Rgb::hex(0x0b0a10)),
    ('H', Rgb::hex(0x4b4560)),
    ('h', Rgb::hex(0x322e40)),
    ('E', Rgb::hex(0xb98cff)),
    ('C', Rgb::hex(0x2b2736)),
    ('c', Rgb::hex(0x1e1b28)),
    ('D', Rgb::hex(0x3a3548)),
];

/// The portrait used by optional interface art.
///
/// One picture, drawn at a size the cell grid could never show: the dialogue
/// overlay uses it when pixel interface art is enabled. Everything else the game
/// draws is ASCII.
pub struct Artwork {
    portrait: Sprite,
}

/// The shared artwork, built on first use.
#[must_use]
pub fn art() -> &'static Artwork {
    static ART: OnceLock<Artwork> = OnceLock::new();
    ART.get_or_init(Artwork::build)
}

impl Artwork {
    /// The dialogue portrait.
    #[must_use]
    pub const fn portrait(&self) -> &Sprite {
        &self.portrait
    }

    /// Builds the artwork from the tables in this module.
    fn build() -> Artwork {
        Artwork {
            portrait: Sprite::from_rows(&PORTRAIT, SHADE),
        }
    }
}

/// The hooded figure as a dialogue portrait: a bust, at a size the world never
/// shows.
const PORTRAIT: [&str; 30] = [
    "..........KKKKKKKK..........",
    ".......KKKHHHHHHHHKKK.......",
    ".....KKHHHHHHHHHHHHHHKK.....",
    "....KHHHHHHHHHHHHHHHHHHK....",
    "...KHHHHHHHHHHHHHHHHHHHHK...",
    "..KHHHHHHHHHHHHHHHHHHHHHHK..",
    "..KHHHhhhhhhhhhhhhhhhHHHHK..",
    ".KHHHh...........h....hHHHK.",
    ".KHHh.....................HK",
    ".KHh.......EE....EE.......hK",
    ".KHh.......EE....EE.......hK",
    ".KHHh......E......E......hHK",
    ".KHHHh...................hHK",
    "..KHHHh...eeeeeeee.....hHHK.",
    "..KHHHHh..............hHHHK.",
    "...KHHHHh............hHHHK..",
    "...KHHHHHHh........hHHHHHK..",
    "....KHHHHHHHHHHHHHHHHHHHK...",
    "....KHHHHHHHHHHHHHHHHHHHK...",
    "...KCCCCCCCCCCCCCCCCCCCCCK..",
    "..KcCCCCCCCCCCCCCCCCCCCCCcK.",
    ".KcCCCCCCCCCCCCCCCCCCCCCCCcK",
    ".KcCCCCCCCCCGGGGCCCCCCCCCcK.",
    ".KcCCCCCCCCGggggGCCCCCCCCcK.",
    ".KcCCCCCCCCGgMMgGCCCCCCCCcK.",
    ".KcCCCCCCCCGggggGCCCCCCCCcK.",
    ".KcCCCCCCCCCGGGGCCCCCCCCCcK.",
    "KcCCCCCCCCCCCCCCCCCCCCCCCCCK",
    "KcCCCCCCCCCCCCCCCCCCCCCCCCCK",
    "KKKKKKKKKKKKKKKKKKKKKKKKKKKK",
];

#[cfg(test)]
mod tests {
    use super::art;

    #[test]
    fn the_portrait_is_a_real_picture() {
        let portrait = art().portrait();
        assert!(portrait.width() > 16 && portrait.height() > 16);
        let opaque = (0..portrait.width() as i32)
            .flat_map(|x| (0..portrait.height() as i32).map(move |y| (x, y)))
            .filter(|(x, y)| portrait.pixel(*x, *y).is_some())
            .count();
        let area = usize::from(portrait.width()) * usize::from(portrait.height());
        assert!(opaque * 2 > area, "the portrait is mostly empty");
    }

    #[test]
    fn the_artwork_is_built_only_once() {
        assert!(std::ptr::eq(art(), art()));
    }
}
