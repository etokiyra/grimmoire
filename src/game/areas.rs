//! The authored region: its maps, its cast, and its points of interest.
//!
//! **Content, not logic.** Every map here is an ASCII grid plus a list of things
//! placed on it; every character is a name, a [`Speech`], and a [`Figure`]; every
//! doorway is a position and a destination. Nothing in this file decides how any
//! of it looks — the interface maps figures and props to art — and nothing in it
//! runs a rule: the world model does that.
//!
//! # The region
//!
//! ```text
//!                    Hollow Village ─────┐
//!                          │             │
//!   Ruined Chapel ── Whispering Woods ── Moonwell Shrine
//!        │                                   │ (sealed until answered)
//!   Sunken Cellar ── Forgotten Catacombs ─────┘
//! ```
//!
//! Every area is reachable without solving anything. The moonwell does not gate a
//! place: it opens a *shortcut* between the shrine and the catacombs, which is a
//! reward for exploring rather than a toll on it.
//!
//! # Maps
//!
//! The `&[...]` grids below are **generated**, not hand-edited: see
//! `tools/generate-areas.py` and `tools/apply-areas.py`, and
//! [`docs/map-design.md`](../../docs/map-design.md) for the rules they enforce —
//! above all that no passage the player can stand in is narrower than
//! [`MIN_PASSAGE`] cells, because the character sprite is five cells wide.

use super::area::{Area, AreaId, Direction, Portal, Position};
use super::dialogue::{Dialogue, Speech};
use super::discovery::Discovery;
use super::entity::{Entity, Figure, Prop};
use super::progress::MoonPhase;

/// The narrowest a passage may be, in cells.
///
/// **Every map must obey this.** The character sprite is five cells wide and
/// three tall, so a passage narrower than three leaves her shoulders through the
/// wall on both sides — which is exactly what the first maps looked like. Three
/// is the sprite width minus two cells of acceptable overlap; the routes a player
/// uses constantly are drawn wider still (the chapel nave is nine cells, its
/// crossings and both stair doorways five).
///
/// The same rule covers anything the player passes *through*: doorways, chamber
/// mouths, corridors, and the gaps between pillars and pews. `tools/generate-areas.py`
/// enforces it when it draws a map, and
/// [`tests::every_passage_fits_the_character`] enforces it here, so a hand-edited
/// map cannot quietly regress. See `docs/map-design.md`.
pub const MIN_PASSAGE: u16 = 3;

/// How wide a doorway in an outer wall is drawn, in cells.
pub const DOORWAY: u16 = 5;

/// The id of the area the game starts in.
pub const CHAPEL: AreaId = AreaId::new("ruined-chapel");
/// The flooded crypt beneath the chapel.
pub const CELLAR: AreaId = AreaId::new("sunken-cellar");
/// The wood the chapel stands in.
pub const WOODS: AreaId = AreaId::new("whispering-woods");
/// The village north of the wood, empty for a generation.
pub const VILLAGE: AreaId = AreaId::new("hollow-village");
/// The shrine east of the wood, and the moonwell inside it.
pub const SHRINE: AreaId = AreaId::new("moonwell-shrine");
/// The burial galleries beneath the shrine.
pub const CATACOMBS: AreaId = AreaId::new("forgotten-catacombs");

// -- the cast ----------------------------------------------------------------
//
// Names and lines are content and nothing else: the renderer reads
// `Entity::name` and `Dialogue::speaker` without knowing what either says, and
// the figure is a role the interface maps to art. Rewriting the cast touches
// this section and nothing else.
//
// They are still, like everything in the region, provisional.

/// Ysolde, who has no face and has been here longer than the chapel.
const YSOLDE: Speech = Speech::reacting(
    Dialogue::new(
        "Ysolde",
        &[
            "You walk where the stones still remember.",
            "The stair below has not been swept in a long while. Mind the water.",
        ],
    ),
    &[
        (
            Discovery::MoonwellAnswered,
            Dialogue::new(
                "Ysolde",
                &[
                    "You answered the moon. Nobody has done that since the village was alive.",
                    "The door under the shrine remembers you now. It will let you both ways.",
                ],
            ),
        ),
        (
            Discovery::ChapelBasin,
            Dialogue::new(
                "Ysolde",
                &[
                    "You looked in the basin. Most people only look at it.",
                    "It fills itself. I have never decided whether that is a kindness.",
                ],
            ),
        ),
    ],
);

/// Rin, who keeps the lanterns on the wood's path and remembers very little.
const RIN: Speech = Speech::reacting(
    Dialogue::new(
        "Rin",
        &[
            "Oh — a person. I have not seen one since, um. Since a while.",
            "I keep the lanterns lit along the path. Someone has to, or the path forgets where it goes.",
        ],
    ),
    &[(
        Discovery::WoodsGlade,
        Dialogue::new(
            "Rin",
            &[
                "You found the glade! I keep that clear too. I do not remember being told to.",
                "Do not mention it to the man in the village. He takes things, and that is the one thing he cannot have.",
            ],
        ),
    )],
);

/// Cassen, who scavenges the village and would rather you did not look closely.
const CASSEN: Speech = Speech::reacting(
    Dialogue::new(
        "Cassen",
        &[
            "Mind the houses. They come down if you lean on them, and then so do you.",
            "You would not have anything worth trading. No. Nobody ever does.",
        ],
    ),
    &[
        (
            Discovery::VillageLetters,
            Dialogue::new(
                "Cassen",
                &[
                    "Those letters were hers, and now they are chapel business. That is what the address says.",
                    "You are not going to read them out loud. I would rather you did not.",
                ],
            ),
        ),
        (
            Discovery::VillageWell,
            Dialogue::new(
                "Cassen",
                &[
                    "So you found the well. Dry, and the rope cut at the top, not the bottom.",
                    "I did not cut it. I only said that I would, which is a different thing, legally.",
                ],
            ),
        ),
    ],
);

/// Ioreth, who tends the moonwell and tells the truth in an inconvenient order.
const IORETH: Speech = Speech::reacting(
    Dialogue::new(
        "Ioreth",
        &[
            "The moon drank here before the village had a name. It still drinks, if it is asked properly.",
            "Very little of this is mine to explain. The floor explains it better.",
        ],
    ),
    &[
        (
            Discovery::MoonwellAnswered,
            Dialogue::new(
                "Ioreth",
                &[
                    "The seal is open. Whatever is below was buried on purpose, by people who could have destroyed it.",
                    "I will not tell you not to go. I will tell you to take your own light.",
                ],
            ),
        ),
        (
            Discovery::MoonwellDial,
            Dialogue::new(
                "Ioreth",
                &[
                    "You have read the dial, then. Waning, full, waxing: dark, filled, and let go.",
                    "Light the braziers in that order and the water will remember what it is for.",
                ],
            ),
        ),
    ],
);

/// Vesper, who was buried standing up and resents it.
const VESPER: Speech = Speech::reacting(
    Dialogue::new(
        "Vesper",
        &[
            "Do not stand in the doorway. The draught takes the warmth out of the living.",
            "I am not a ghost. I am the last thing that was left of one.",
        ],
    ),
    &[(
        Discovery::CatacombScribe,
        Dialogue::new(
            "Vesper",
            &[
                "So you found the scribe. She wrote the same name until her hands stopped doing it.",
                "It was the moon's name. She got as far as the second letter.",
            ],
        ),
    )],
);

// -- areas -------------------------------------------------------------------

/// The starting area: a long ruined chapel, its nave running north to the apse.
#[must_use]
pub fn chapel() -> Area {
    Area::new(
        CHAPEL,
        "Ruined Chapel",
        &[
            "############################################################################################################################################",
            "#.......#...........#...........#...........#...........#...........#...........#...........#...........#...........#...........#..........#",
            "#.......#...........#...........#...........#...........#...........#...........#...........#...........#...........#...........#..........#",
            "#...,,,.,,.,,.....,,.....,,.....#,.....,,.....,,.....,,...,,,,,,,,,,#,,,,,,,,,,,#,,.....,,.....,,.....,,.....,,.....#,.....,,.....,,,,.,,..#",
            "#..,.,##.,................................................#,,,,,,,,,,,,,,,,,,,,,,,#...............................................,.,##.,..#",
            "#..,,.,#,.................................................#,,,,,,,,,,,####,,,,,,,,#...............................................#,.,##...#",
            "#..,#,.,##............................................#..,#,,,,,,,,#,,,,,#,,,,,,,,##..,...........................................,#,.,#,..#",
            "#...##,.,#............................................#..,#,,,,,,,,###,,,#,,,,,,,,##..,............................................##,.,,..#",
            "#..,.##,.,............................................#..,#,,,,,,,,,,,,,,,,,,,,,,,##..,...........................................,.##,.,..#",
            "#..,,.##,.................................................#########,,,,,,,#########...............................................#,.,#,...#",
            "#...................................................................,,,,,..................................................................#",
            "#..,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,..#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#.....................................................#..,.#,.,,.,#.,,,,,.#,.,,.,#.,..#....................................................#",
            "#.....................................................#..,..........,,,,,..........#..,....................................................#",
            "#.....................................................#..,..........,,,,,..........#..,....................................................#",
            "#..........................................................,,.,#.,,.,,,,,.,,.,#.,,.........................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...............,#############......................................,,,,,.......................................,#############.............#",
            "#...............,.,#...##....#.............................##.,,.##.,,,,,.##.,,.##..............................,.,#...##....#.............#",
            "#...............,............#......................................,,,,,.......................................,............#.............#",
            "#...............#............#........................#..,..........,,,,,..........#..,.........................#............#.............#",
            "+...............#............#........................#..,.,,.##.,,.,,,,,.,,.##.,,.#..,.........................#............#.............#",
            "+...............,............,........................#..,..........,,,,,..........#..,.........................,............,.............#",
            "+...................................................................,,,,,..................................................................#",
            "+...............,............,......................................,,,,,.......................................,............,.............#",
            "+...............#............#......................................,,,,,.......................................#............#.............#",
            "#...............#............#...............................#...#..,,,,,..#...#................................#............#.............#",
            "#...............#............#......................................,,,,,.......................................#............#.............#",
            "#...............,.,#...##....#........................#..,..........,,,,,..........#..,.........................,.,#...##....#.............#",
            "#...............,#############........................#..,..........,,,,,..........#..,.........................,#############.............#",
            "#.....................................................#..,...#...#..,,,,,..#...#...#..,....................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#............................................................#...#..,,,,,..#...#...........................................................#",
            "#...,#,.,#..........................................................,,,,,..........................................................,#,.,,..#",
            "#..,.,##.,............................................#..,..........,,,,,..........#..,...........................................,.,##.,..#",
            "#..,,.,##.............................................#..,..........,,,,,..........#..,...........................................#,.,##...#",
            "#..,#,.,,,............................................#..,..........,,,,,..........#..,...........................................,#,.,#,..#",
            "#..,##,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,##,,,,..#",
            "#..,.##,.,..........................................................,,,,,.........................................................,.##,.,..#",
            "#..,,.~~~~~~~.......................................................,,,,,.........................................................#,.,#,...#",
            "#.....~~~~~~~.......T...............................................,,,,,..................................................T...............#",
            "#.....~~~~~~~...............................................T.......,,,,,.....................................T.............T..............#",
            "#.....~~~~~~~.,...................................T.................,,,,,......................T...........................................#",
            "#..............T........T.....T.....................................,,,,,.......,...................T.................T....................#",
            "#...,,.....,,.....,,.....,,.....#,.....,,.....,,.....,,.....,,.....,,,,,,.,,.....,,.....,,.....,,.....,,.....,,.....#,.....,,.....,,.......#",
            "#.......,...........#...........#...........#...........#...........,,,,,.......,...........#...........#...........#...........#..........#",
            "#.......#...........#...........#...........#...........#...........,...........#...........#...........#...........#...........#..........#",
            "####################################################################+++++###################################################################",
        ],
        vec![
            Entity::npc(
                Position::new(56, 33),
                "Ysolde",
                YSOLDE,
                Figure::Veiled,
                Direction::East,
            ),
            Entity::relic(
                Position::new(76, 6),
                "a cracked basin",
                Prop::Basin,
                "Still water, cold and dark; it has not rained in a long while.",
                Discovery::ChapelBasin,
            ),
            Entity::object(
                Position::new(60, 44),
                "a worn sigil",
                Prop::Sigil,
                "Cut into the flagstones, and worn almost away by feet that are long gone.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(70, 51),
                CELLAR,
                Position::new(20, 12),
                "the Sunken Cellar",
            ),
            Portal::new(
                Position::new(0, 25),
                WOODS,
                Position::new(64, 40),
                "the Whispering Woods",
            ),
        ],
        Position::new(70, 44),
    )
}

/// The crypt beneath the chapel: four chambers, one of them under water.
#[must_use]
pub fn cellar() -> Area {
    Area::new(
        CELLAR,
        "Sunken Cellar",
        &[
            "##################+++++#################################################################################################################",
            "#..................,,,.................................................................................................................#",
            "#..................,,,.................................................................................................................#",
            "#..................,,,.................................................................................................................#",
            "#..................,,,..................................,..............................................................................#",
            "#..................,,,...................................................................................................#.............#",
            "#.....,############,,,#################################,,,##,.............,#######################################################.....#",
            "#.....,.,#....##...,,,....##....#,....##....##....##....##..,.............,.,#....##....##....##...##,....##....##....##....##...#.....#",
            "#.....,............,,,...........,..,.......................,.............,..........................,...........................#.....#",
            "#.....#............,,,...,.......,..........................#.............#..........................,...........................#.....#",
            "#.....#............,,,...........,..........................#.............#..........................,..........,,####,,,###,....#.....#",
            "#.....#....................#.....,..........................#.............#....#.....................,..........#,,,,,,,,,,,#....#.....#",
            "#...............#................,..........................#.....,,,,,,,,...........................,..........#,,,,,,,,,,,#....#.....#",
            "#....#.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,#,,,,#.....#",
            "#................................,..........................#.....,,,,,,,,...........................,..........#,,,,,,,,,,,#....#.....#",
            "#.....##.........................,.........................##.............#..........................,..........#,,,,,,,,,,,#....#.....#",
            "#.....#..........................,..........................#.............#..........................,..........####,,,,,####....#.....#",
            "#.....#..........................,..........................##............#..........................,...........,...............#.....#",
            "#.....#..........................,................#.........#.............#..........................,#..........................#.....#",
            "#.....,.,#....##....##....##....#,....##....##....##....##..,.............,.,#....##....##....##....#,....##....,,.,..##....,#...#.....#",
            "#.....,#####################################################,.............,##########################,,,##########################.....#",
            "#............................................................................................#.........................................#",
            "#.............................,...................................,,,,,,,,.............................................................#",
            "#..................,..............................................,,,,,,,,.............................................................#",
            "#.....#######################################################.....,,,,,,,,,#################,,,###################################.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#............#,.,#....##....##....##....#,....##....##....##....##...#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.,...........,..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#...#.........#..........................,..............#............#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,...........................,................#..........#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,......................#....,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#.......................#..,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#............#.............,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#....#........,.,#....##....##....##....#,....##....##....##....##...#.....#",
            "#.....#######################################################.............,###########,,,############################...###########....#",
            "#..................................................................#.................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "####################################################################################################################+++++###############",
        ],
        vec![Entity::object(
            Position::new(118, 13),
            "a faded sigil",
            Prop::Sigil,
            "The carving is worn smooth, but it hums faintly under your fingers.",
        )],
        vec![
            Portal::new(
                Position::new(20, 0),
                CHAPEL,
                Position::new(70, 49),
                "the Ruined Chapel",
            ),
            Portal::new(
                Position::new(118, 43),
                CATACOMBS,
                Position::new(8, 21),
                "the Forgotten Catacombs",
            ),
        ],
        Position::new(20, 12),
    )
}

/// The wood: a trodden trail between the chapel and the village, and a glade.
#[must_use]
pub fn woods() -> Area {
    Area::new(
        WOODS,
        "Whispering Woods",
        &[
            "############################################################+++++###############################################################",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,T,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,T,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,TT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,TT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTT,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTT,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,T,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,T,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTT,,,TT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "##############################################################+++++#############################################################",
        ],
        vec![
            Entity::npc(
                Position::new(63, 17),
                "Rin",
                RIN,
                Figure::Lantern,
                Direction::South,
            ),
            Entity::relic(
                Position::new(18, 30),
                "a standing stone",
                Prop::StandingStone,
                "A hole worn right through it, and a floor around it swept clean of leaves.",
                Discovery::WoodsGlade,
            ),
            Entity::object(
                Position::new(52, 26),
                "a waymarker cairn",
                Prop::Cairn,
                "Stones stacked by someone with more patience than sense.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(64, 43),
                CHAPEL,
                Position::new(5, 25),
                "the Ruined Chapel",
            ),
            Portal::new(
                Position::new(62, 0),
                VILLAGE,
                Position::new(62, 38),
                "the Hollow Village",
            ),
            Portal::new(
                Position::new(127, 20),
                SHRINE,
                Position::new(5, 20),
                "the Moonwell Shrine",
            ),
        ],
        Position::new(64, 40),
    )
}

/// The village: a crossroad of fallen houses around a dry well.
#[must_use]
pub fn village() -> Area {
    Area::new(
        VILLAGE,
        "Hollow Village",
        &[
            "############################################################################################################################",
            "#..........................................................................................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#.............,#########################,.......,#########################,.......,############################............#",
            "#.............,.,#...##...##...##...##..,.......,.,#...##...##...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,.........................,.......,.........................,.......,...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#..........#.,............#.......#..........#.,............#.......#............,.#............#............#",
            "#.............#...........#.............#.......#...........#.............#.......#.............#.............#............#",
            "#.............#..........,.#............#.......#..........,.#............#.......#............#.,............#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............,.,#...##...#,...##...##..,.......,.,#...##...#,...##...##..,.......,.,#...##...#,...##...##....#............#",
            "#.............,###########...###########,.......,###########...###########,.......,############...#############............#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#.............,###########...###########,.......,###########...###########,.......,############...#############............#",
            "#.............,.,#...##...##...##...##..,.......,.,#...##...##...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,.........................,.......,.........................,.......,...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#..........#.,............#.......#..........#.,............#.......#............#.,............#............#",
            "#.............#...........#.............#.......#...........#.............#.......#.............#.............#............#",
            "#.............#..........,.,............#.......#..........,.,............#.......#............,.#............#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............,.,#...##...,,...##...##..,.......,.,#...##...,,...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,#########################,.......,#########################,.......,#############,,,############............#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "############################################################+++++###########################################################",
        ],
        vec![
            Entity::npc(
                Position::new(40, 20),
                "Cassen",
                CASSEN,
                Figure::Scavenger,
                Direction::East,
            ),
            Entity::relic(
                Position::new(62, 20),
                "the village well",
                Prop::Well,
                "Dry to the bottom. The rope is cut at the top, not the bottom.",
                Discovery::VillageWell,
            ),
            Entity::relic(
                Position::new(20, 10),
                "a bundle of letters",
                Prop::Letters,
                "Tied with string and addressed to the chapel. None of them were opened.",
                Discovery::VillageLetters,
            ),
        ],
        vec![
            Portal::new(
                Position::new(62, 41),
                WOODS,
                Position::new(64, 40),
                "the Whispering Woods",
            ),
            Portal::new(
                Position::new(0, 20),
                SHRINE,
                Position::new(56, 5),
                "the Moonwell Shrine",
            ),
        ],
        Position::new(62, 38),
    )
}

/// The shrine: a pale hall around the moonwell, with three braziers and a dial.
#[must_use]
pub fn shrine() -> Area {
    Area::new(
        SHRINE,
        "Moonwell Shrine",
        &[
            "######################################################+++++#############################################################",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....#.,,,,#.....#.....#.....#.....#.....#.....#.....#.....#............#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....,.,,,,,.....,.....#.....#.....#.....#.....#.....#.....#............#",
            "#.....................................................,,~,,............................................................#",
            "#.................................................~~~~~~~~~~~~~........................................................#",
            "#..............................................~~~~~~~~~~~~~~~~~~~.....................................................#",
            "#.............................................~~~~~~~~~~~~~~~~~~~~~....................................................#",
            "#............................................~~~~~~~~~~~~~~~~~~~~~~~...................................................#",
            "+.........#.....#.....#.....#.....#.....#...~~~~~~~~~~~~~~~~~~~~~~~~~.,.....#.....#.....#.....#.....#.....#............+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,..~~~~~~~~~~~~~~~~~~~~~~~~~~~.................................................+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "#............................................~~~~~~~~~~~~~~~~~~~~~~~...................................................#",
            "#.........#.....#.....#.....#.....#.....#.....~~~~~~~~~~~~~~~~~~~~~...#.....#.....#.....#.....#.....#.....#............#",
            "#..............................................~~~~~~~~~~~~~~~~~~~.....................................................#",
            "#.................................................~~~~~~~~~~~~~........................................................#",
            "#.......................................................~..............................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#............#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "########################################################################################################################",
        ],
        vec![
            Entity::npc(
                Position::new(30, 14),
                "Ioreth",
                IORETH,
                Figure::Priestess,
                Direction::East,
            ),
            Entity::relic(
                Position::new(40, 20),
                "the moon dial",
                Prop::Dial,
                "A waning crescent, a full moon, and a waxing one — in that order.",
                Discovery::MoonwellDial,
            ),
            Entity::brazier(
                Position::new(78, 16),
                "the first brazier",
                MoonPhase::Waning,
                "A bronze bowl on a stem, marked with a closing crescent. Cold.",
            ),
            Entity::brazier(
                Position::new(78, 20),
                "the second brazier",
                MoonPhase::Full,
                "A bronze bowl on a stem, marked with a full moon. Cold.",
            ),
            Entity::brazier(
                Position::new(78, 24),
                "the third brazier",
                MoonPhase::Waxing,
                "A bronze bowl on a stem, marked with an opening crescent. Cold.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(56, 0),
                VILLAGE,
                Position::new(62, 38),
                "the Hollow Village",
            ),
            Portal::new(
                Position::new(0, 20),
                WOODS,
                Position::new(124, 20),
                "the Whispering Woods",
            ),
            Portal::sealed(
                Position::new(119, 20),
                CATACOMBS,
                Position::new(121, 21),
                "the Forgotten Catacombs",
                Discovery::MoonwellAnswered,
            ),
        ],
        Position::new(56, 5),
    )
}

/// The catacombs: a long gallery of burials, and something still awake in it.
#[must_use]
pub fn catacombs() -> Area {
    Area::new(
        CATACOMBS,
        "Forgotten Catacombs",
        &[
            "####################################################################################################################################",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#.....##.......##..,,,..##.......##.......##,,.....##.......##.......##.......##.......##......,,,.......##.......##.,,,...##......#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,............############,,,############..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#.....#........#...,,,..#........#........#,,,.....#......#,,,,,,,,,,,,,,,,,,,,,,,,,#..,.......,,,.......#........#..,,,...#.......#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#.....#........#...,,,..#........#........#,,,.....#......#,,,,,,,,,,,,,,,,,,,,,,,,,#..,.......,,,.......#........#..,,,...#.......#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............############,,,############..........,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#.....##.......##..,,,..##.......##.......##,,.....##.......,,.......,#.......,,.......##......,,,.......##.......##.,,,...##......#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "####################################################################################################################################",
        ],
        vec![
            Entity::npc(
                Position::new(52, 21),
                "Vesper",
                VESPER,
                Figure::Revenant,
                Direction::West,
            ),
            Entity::relic(
                Position::new(71, 22),
                "the bone scribe's tomb",
                Prop::Sarcophagus,
                "The lid is carved with a name that stops after two letters, over and over.",
                Discovery::CatacombScribe,
            ),
            Entity::object(
                Position::new(20, 30),
                "a heap of grave goods",
                Prop::Cairn,
                "Wax, a stylus, and a bowl. Whoever was buried here meant to keep working.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(0, 21),
                CELLAR,
                Position::new(118, 40),
                "the Sunken Cellar",
            ),
            Portal::sealed(
                Position::new(131, 21),
                SHRINE,
                Position::new(115, 20),
                "the Moonwell Shrine",
                Discovery::MoonwellAnswered,
            ),
        ],
        Position::new(8, 21),
    )
}

/// Every area in the region, in a stable order.
#[must_use]
pub fn all() -> Vec<Area> {
    vec![
        chapel(),
        cellar(),
        woods(),
        village(),
        shrine(),
        catacombs(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        CATACOMBS, CELLAR, CHAPEL, all, catacombs, cellar, chapel, shrine, village, woods,
    };
    use crate::game::area::{Area, Direction, Position, Tile};
    use crate::game::discovery::Discovery;
    use crate::game::entity::EntityKind;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    /// The viewport the world is designed to fill, from `ui::viewport`.
    const REFERENCE: (u16, u16) = (120, 40);

    /// Every walkable tile reachable from `start`, ignoring entities (a single
    /// NPC or object cannot seal a region, and blocking on them would give a
    /// misleading picture of the layout).
    fn reachable(area: &Area, start: Position) -> BTreeSet<Position> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        if !area.tile_walkable(start) {
            return seen;
        }
        seen.insert(start);
        queue.push_back(start);
        while let Some(position) = queue.pop_front() {
            for direction in Direction::ALL {
                if let Some(next) = position.checked_step(direction, area.width(), area.height()) {
                    if area.tile_walkable(next) && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        seen
    }

    fn walkable_count(area: &Area) -> usize {
        (0..area.height())
            .flat_map(|y| (0..area.width()).map(move |x| Position::new(x, y)))
            .filter(|position| area.tile_walkable(*position))
            .count()
    }

    fn adjacent_to_reachable(area: &Area, position: Position, seen: &BTreeSet<Position>) -> bool {
        Direction::ALL.iter().any(|direction| {
            position
                .checked_step(*direction, area.width(), area.height())
                .is_some_and(|neighbour| seen.contains(&neighbour))
        })
    }

    /// How many walkable cells lie in a straight line through `position`.
    fn run(area: &Area, position: Position, dx: i32, dy: i32) -> u16 {
        let mut length = 1;
        for sign in [1i32, -1] {
            let mut x = i32::from(position.x) + dx * sign;
            let mut y = i32::from(position.y) + dy * sign;
            while (0..i32::from(area.width())).contains(&x)
                && (0..i32::from(area.height())).contains(&y)
                && area.tile(Position::new(x as u16, y as u16)).is_walkable()
            {
                length += 1;
                x += dx * sign;
                y += dy * sign;
            }
        }
        length
    }

    /// The narrowest cross-section through a tile: the smaller of the horizontal
    /// and vertical runs, which is the one the character has to fit through.
    fn clearance(area: &Area, position: Position) -> u16 {
        run(area, position, 1, 0).min(run(area, position, 0, 1))
    }

    #[test]
    fn every_passage_fits_the_character() {
        for area in all() {
            let area = &area;
            let mut worst = u16::MAX;
            let mut where_at = Position::new(0, 0);
            for y in 0..area.height() {
                for x in 0..area.width() {
                    let position = Position::new(x, y);
                    if !area.tile_walkable(position) {
                        continue;
                    }
                    let clearance = clearance(area, position);
                    if clearance < worst {
                        worst = clearance;
                        where_at = position;
                    }
                }
            }
            assert!(
                worst >= super::MIN_PASSAGE,
                "{} has a {worst}-cell passage at {},{}; the character sprite is \
                 five cells wide, so no passage may be narrower than {}",
                area.name(),
                where_at.x,
                where_at.y,
                super::MIN_PASSAGE,
            );
        }
    }

    #[test]
    fn the_main_routes_are_wider_than_the_minimum() {
        // The route a player walks constantly is drawn to the character's full
        // width rather than to the floor.
        let chapel = chapel();
        assert!(
            clearance(&chapel, Position::new(70, 44)) >= super::DOORWAY,
            "the nave should be a full character wide"
        );
    }

    #[test]
    fn every_doorway_is_a_full_character_wide() {
        for area in all() {
            let area = &area;
            for portal in area.portals() {
                // The door is a run of door tiles in the wall; measure it along
                // the wall it sits in.
                // A door on an edge has no neighbour outside it: `checked_sub`
                // rather than `saturating_sub`, or the door would be compared
                // with itself and read as a one-cell horizontal opening.
                let left = portal
                    .at
                    .x
                    .checked_sub(1)
                    .map(|x| area.tile(Position::new(x, portal.at.y)));
                let right = portal
                    .at
                    .x
                    .checked_add(1)
                    .map(|x| area.tile(Position::new(x, portal.at.y)));
                let horizontal = left == Some(Tile::Door) || right == Some(Tile::Door);
                let (dx, dy) = if horizontal { (1, 0) } else { (0, 1) };
                let mut width = 1;
                for sign in [1i32, -1] {
                    let mut x = i32::from(portal.at.x) + dx * sign;
                    let mut y = i32::from(portal.at.y) + dy * sign;
                    while (0..i32::from(area.width())).contains(&x)
                        && (0..i32::from(area.height())).contains(&y)
                        && area.tile(Position::new(x as u16, y as u16)) == Tile::Door
                    {
                        width += 1;
                        x += dx * sign;
                        y += dy * sign;
                    }
                }
                assert!(
                    width >= super::MIN_PASSAGE,
                    "{}: the doorway at {},{} is only {width} cells wide",
                    area.name(),
                    portal.at.x,
                    portal.at.y
                );
            }
        }
    }

    #[test]
    fn every_area_has_a_distinct_id_and_name() {
        let areas = all();
        let ids: BTreeSet<_> = areas.iter().map(crate::game::Area::id).collect();
        let names: BTreeSet<_> = areas.iter().map(crate::game::Area::name).collect();
        assert_eq!(areas.len(), ids.len(), "two areas share an id");
        assert_eq!(areas.len(), names.len(), "two areas share a name");
        assert!(areas.len() >= 6, "the region lost a place");
    }

    #[test]
    fn each_area_fills_the_reference_viewport() {
        for area in all() {
            assert!(
                area.width() >= REFERENCE.0 && area.height() >= REFERENCE.1,
                "{} is {}x{}, smaller than the reference viewport {:?}",
                area.name(),
                area.width(),
                area.height(),
                REFERENCE
            );
        }
    }

    #[test]
    fn each_area_spawns_the_player_on_walkable_ground() {
        for area in all() {
            assert!(
                area.tile_walkable(area.spawn()),
                "{} spawns inside terrain",
                area.name()
            );
            assert!(area.entity_at(area.spawn()).is_none());
        }
    }

    #[test]
    fn every_entity_stands_on_floor_and_can_be_reached() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            for entity in area.entities() {
                assert!(
                    area.tile_walkable(entity.position),
                    "{} stands on terrain in {}",
                    entity.name,
                    area.name()
                );
                assert!(
                    adjacent_to_reachable(area, entity.position, &seen),
                    "{} at {:?} cannot be walked up to in {}",
                    entity.name,
                    entity.position,
                    area.name()
                );
            }
        }
    }

    #[test]
    fn every_doorway_is_on_a_door_and_reachable_from_both_sides() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            for portal in area.portals() {
                assert_eq!(
                    Tile::Door,
                    area.tile(portal.at),
                    "{} has a doorway that is not a door",
                    area.name()
                );
                assert!(
                    adjacent_to_reachable(area, portal.at, &seen),
                    "the doorway at {:?} in {} cannot be reached",
                    portal.at,
                    area.name()
                );

                let destination = all()
                    .into_iter()
                    .find(|candidate| candidate.id() == portal.to)
                    .unwrap_or_else(|| panic!("{} leads to a missing area", area.name()));
                assert!(
                    destination.tile_walkable(portal.arrive),
                    "{} arrives inside terrain in {}",
                    area.name(),
                    destination.name()
                );
                assert!(
                    destination.entity_at(portal.arrive).is_none(),
                    "{} arrives on top of an entity",
                    area.name()
                );
                let destination_seen = reachable(&destination, portal.arrive);
                assert!(
                    !destination_seen.is_empty(),
                    "arriving in {} leaves the player stuck",
                    destination.name()
                );
            }
        }
    }

    #[test]
    fn every_doorway_runs_both_ways() {
        let areas = all();
        for area in &areas {
            for portal in area.portals() {
                let destination = areas
                    .iter()
                    .find(|candidate| candidate.id() == portal.to)
                    .unwrap_or_else(|| panic!("{} leads nowhere", area.name()));
                assert!(
                    destination
                        .portals()
                        .iter()
                        .any(|back| back.to == area.id()),
                    "{} leads to {} but there is no way back",
                    area.name(),
                    destination.name()
                );
            }
        }
    }

    #[test]
    fn the_whole_region_is_reachable_from_where_the_game_starts() {
        // A breadth-first walk over the *area graph*, following only doorways that
        // are open from the start. This is the test that would have caught a new
        // place being drawn, populated, and then walled off from the world.
        let areas = all();
        let start = areas
            .iter()
            .find(|area| area.id() == CHAPEL)
            .expect("the chapel exists");
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        seen.insert(start.id());
        queue.push_back(start.id());
        while let Some(id) = queue.pop_front() {
            let area = areas
                .iter()
                .find(|area| area.id() == id)
                .expect("a known area");
            for portal in area.portals() {
                // A sealed doorway is still a connection: it is reachable, visible
                // and interactive, it simply does not open yet.
                if seen.insert(portal.to) {
                    queue.push_back(portal.to);
                }
            }
        }
        assert_eq!(
            areas.len(),
            seen.len(),
            "unreachable areas: {:?}",
            areas
                .iter()
                .map(crate::game::Area::id)
                .filter(|id| !seen.contains(id))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn at_least_one_doorway_stays_open_into_every_hidden_place() {
        // A whole area behind a puzzle would be a toll on exploration rather than
        // a reward for it. Only the shortcut may be sealed.
        for area in all() {
            let sealed = area
                .portals()
                .iter()
                .filter(|portal| portal.requires.is_some())
                .count();
            assert!(
                sealed < area.portals().len() || sealed == 0,
                "{} is only reachable through a sealed door",
                area.name()
            );
        }
    }

    #[test]
    fn most_of_each_area_is_one_connected_space() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            let walkable = walkable_count(area);
            let fraction = seen.len() * 100 / walkable.max(1);
            assert!(
                fraction >= 80,
                "{}: only {fraction}% of {walkable} walkable tiles are reachable from the spawn",
                area.name()
            );
        }
    }

    #[test]
    fn the_region_has_every_voice_and_somewhere_to_use_it() {
        let voices: BTreeSet<&str> = all()
            .iter()
            .flat_map(|area| area.entities())
            .filter_map(|entity| match entity.kind {
                EntityKind::Npc { .. } => Some(entity.name),
                _ => None,
            })
            .collect();
        assert!(voices.len() >= 5, "the cast shrank to {voices:?}");
        assert!(voices.contains("Ysolde"), "the first voice is gone");
    }

    #[test]
    fn each_area_has_something_to_look_at() {
        for area in &all() {
            let landmarks = area
                .entities()
                .iter()
                .filter(|entity| !matches!(entity.kind, EntityKind::Npc { .. }))
                .count();
            assert!(
                landmarks >= 1,
                "{} has nothing in it to examine",
                area.name()
            );
        }
    }

    #[test]
    fn every_discovery_can_actually_be_made() {
        // Each discovery is either granted by examining a landmark or by answering
        // the moonwell. An unreachable journal entry would be content that exists
        // and can never be read.
        let mut granted: BTreeSet<Discovery> = BTreeSet::new();
        let mut braziers = 0;
        for area in all() {
            for entity in area.entities() {
                if let EntityKind::Object {
                    discovery: Some(discovery),
                    ..
                } = entity.kind
                {
                    granted.insert(discovery);
                }
                if matches!(entity.kind, EntityKind::Brazier { .. }) {
                    braziers += 1;
                }
            }
        }
        let phases: BTreeSet<crate::game::MoonPhase> = all()
            .iter()
            .flat_map(|area| area.entities())
            .filter_map(|entity| match entity.kind {
                EntityKind::Brazier { phase, .. } => Some(phase),
                _ => None,
            })
            .collect();
        if braziers > 0 {
            assert_eq!(
                crate::game::progress::MOON_ORDER.len(),
                phases.len(),
                "the moonwell needs one brazier per phase, and no repeats"
            );
            granted.insert(Discovery::MoonwellAnswered);
        }
        for discovery in Discovery::ALL {
            assert!(
                granted.contains(&discovery),
                "{} can never be discovered",
                discovery.title()
            );
        }
    }

    #[test]
    fn the_moonwell_braziers_are_all_in_one_place() {
        let areas: BTreeMap<_, _> = all()
            .into_iter()
            .map(|area| {
                let count = area
                    .entities()
                    .iter()
                    .filter(|entity| matches!(entity.kind, EntityKind::Brazier { .. }))
                    .count();
                (area.id(), count)
            })
            .collect();
        let with_braziers: Vec<_> = areas
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(id, count)| (*id, *count))
            .collect();
        assert_eq!(
            vec![(super::SHRINE, 3)],
            with_braziers,
            "the braziers belong together, in the shrine"
        );
    }

    #[test]
    fn every_area_has_a_reachable_exit() {
        for area in all() {
            assert!(!area.portals().is_empty(), "{} is a dead end", area.name());
        }
        // And the two places below ground connect to each other, not only upwards.
        let cellar = cellar();
        assert!(
            cellar.portals().iter().any(|portal| portal.to == CATACOMBS),
            "the cellar has no way down to the catacombs"
        );
        assert!(catacombs().portals().iter().any(|p| p.to == CELLAR));
        assert!(woods().portals().iter().any(|p| p.to == CHAPEL));
        assert!(village().portals().iter().any(|p| p.to == super::WOODS));
        assert!(shrine().portals().iter().any(|p| p.to == super::VILLAGE));
    }

    #[test]
    fn the_sealed_way_is_the_shortcut_and_needs_the_moonwell() {
        let sealed: Vec<_> = all()
            .into_iter()
            .flat_map(|area| {
                area.portals()
                    .iter()
                    .filter(|portal| portal.requires.is_some())
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(2, sealed.len(), "the shortcut has two ends");
        for portal in sealed {
            assert_eq!(
                Some(Discovery::MoonwellAnswered),
                portal.requires,
                "only the moonwell opens anything"
            );
        }
    }
}
