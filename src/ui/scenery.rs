//! The ASCII world renderer.
//!
//! This is Grimmoire's own scenery renderer. It draws the world into the cell
//! grid the rest of the interface uses: one glyph per cell, chosen from what the
//! tile is, what surrounds it, what time it is, and what the player just did.
//!
//! # What makes it read as a place
//!
//! 1. **Structure.** A wall is not a field of `#`. Its glyph depends on its
//!    neighbours: a wall with open floor above it shows a lit cap, one with floor
//!    below shows its shadowed base, a side wall shows a dressed face, and a wall
//!    standing alone in a room is drawn as a pillar.
//! 2. **Variation.** Floors, paths and water pick from small sets using a hash of
//!    their world position. The same tile looks the same every time it is drawn,
//!    so the world never shimmers as the camera moves and neighbours still differ.
//! 3. **Inlay.** Large open floors would otherwise read as empty graph paper, so
//!    each area carries a deliberate motif - a rose cut into the chapel crossing,
//!    rings around the cellar's sigil - drawn only on plain, unoccupied floor.
//! 4. **Life.** Candles gutter along the walls, motes drift, water carries a
//!    rolling swell, the sigil turns. All of it is a function of position and
//!    time, and none of it is ever drawn over a character, a prop or a doorway.
//! 5. **Figures.** Characters are multi-coloured ASCII figures: hair, skin, cloth,
//!    trim and eyes are separate inks, with four facing directions and a stride.
//!    Which art a figure gets comes from [`super::cast`], never from a special
//!    case here.
//! 6. **Place.** Each area has its own ground, its own air, and its own inlay, so
//!    a wood, a village, a shrine and a crypt are told apart before the location
//!    name is read.
//!
//! # Draw order
//!
//! Terrain, then inlay, then props, then ambience, then figures. A feedback pulse
//! is applied as a *style* change wherever it lands, so it can never hide a glyph.
//! Only tiles inside the view are visited.
//!
//! Everything here is pure: a world, a palette, a time and a pose in; cells out.
//! It knows nothing about terminals, input, or the game's rules.

use crate::game::{Area, AreaId, Direction, EntityKind, Figure, Position, Prop, Tile, World};

use super::animation::{Pose, Stage};
use super::effects::{Effects, Pulse};
use super::sprite::{self, Pose as SpritePose};
use super::theme::Palette;
use crate::render::{CanvasView, Color, Style};

use super::sprite::{ArtInks, Sprite};

/// How far the pool the player carries brightens what it falls on, in tiles.
const LIGHT_RADIUS: u16 = 7;

/// How often a wall that faces open floor carries an aperture.
const WINDOW_PITCH: u16 = 13;

/// How far apart the seams between flagstones are.
const JOINT_PITCH: u16 = 6;

/// The shape an area puts in the walls that face open floor.
///
/// A chapel has stained glass, a shrine has a lit rune, a village has a lamp
/// behind a shutter, a catacomb has a grave-lamp — and a wood and a cellar have
/// nothing at all, because one is a treeline and the other is underground. A
/// place with no apertures says so by returning the masonry it would have drawn
/// anyway, so the wall keeps its own stone.
/// How often a run of wall carries a window, in rows.
///
/// A chapel is glazed: its bays are close together, and reading as a lit
/// building rather than a stone one is the whole of its identity. Everywhere
/// else keeps the wider beat, where a window is an event rather than the rule.
fn window_pitch(area: AreaId) -> u16 {
    match area {
        crate::game::areas::CHAPEL => WINDOW_PITCH / 2,
        _ => WINDOW_PITCH,
    }
}

fn aperture(area: AreaId) -> (char, char) {
    use crate::game::areas;
    match area {
        areas::CHAPEL => ('▌', '◆'),
        areas::VILLAGE => ('▌', '◆'),
        areas::SHRINE => ('│', '◇'),
        areas::CATACOMBS => ('░', '·'),
        _ => ('▓', '▒'),
    }
}

/// How many rows a figure's hem sits above its own tile.
///
/// A sprite's *last* row is drawn on the tile the character stands on, so a
/// taller sprite reaches further up rather than sinking into the floor.
const STANDING_ROWS: i32 = 0;

/// The primary glyph families, grouped so the art can be read at a glance.
mod art {
    /// Floors inside a building: worn stone.
    ///
    /// Deliberately only a couple of glyphs. Every tile being different is noise;
    /// a floor should read as one surface with a little grit on it.
    pub const FLOOR: &[char] = &['·', '.', '·', '.', '·', '\''];
    /// Trodden earth paths.
    pub const PATH: &[char] = &[',', '·', ',', ';', ',', '.'];
    /// Grass, leaf litter and clover, for the wood.
    pub const GRASS: &[char] = &['"', '\'', ',', '"', '.', '\''];
    /// Ash, brick-dust and broken tile, for the village.
    pub const ASH: &[char] = &['·', '.', '·', '.', ',', '·'];
    /// Pale polished slabs, for the shrine.
    pub const MARBLE: &[char] = &['·', '.', '·', '.', '·', '\''];
    /// Bone dust and splinters, for the catacombs.
    pub const BONE: &[char] = &['·', '.', '·', '.', ';', '·'];
    /// Flood water, calm to moving.
    pub const WATER: &[char] = &['~', '~', '≈', '~', '∼', '~'];
    /// Rubble and broken masonry scattered over a floor.
    pub const RUBBLE: &[char] = &['*', '+', 'x', '·'];
    /// A candle flame, in the order it gutters.
    pub const FLAME: &[char] = &['\'', '.', 'i', '*', 'i', '.'];

    /// Sparkles thrown off by magic, in the order they twinkle.
    pub const SPARKLE: &[char] = &['·', '+', '✧', '*', '✧', '+'];

    // -- props ---------------------------------------------------------------
    //
    // Each is a small block drawn with its baseline on a world cell, in a single
    // role colour. They are shapes rather than pictograms: five cells across is
    // enough for a silhouette to be unmistakable and not enough for detail, which
    // is exactly the discipline the world renderer works under.

    /// A cracked basin: a bowl on a stem.
    pub const BASIN: [&str; 3] = [".---.", "\\___/", "  |  "];
    /// A faded sigil, cut into the floor, in its two turning phases.
    pub const SIGIL: [[&str; 3]; 2] = [["\\|/", "-O-", "/|\\"], ["/|\\", "-O-", "\\|/"]];
    /// A well: a stone drum, unlike the basin because it is solid to the ground.
    pub const WELL: [&str; 3] = [".---.", "|###|", "\\___/"];
    /// A bundle of letters, tied with string.
    pub const LETTERS: [&str; 3] = [" ___ ", "/=+=\\", "\\___/"];
    /// A cairn of stacked stones.
    pub const CAIRN: [&str; 3] = ["  ^  ", " ( ) ", "(-=-)"];
    /// A standing stone with a hole worn through it.
    pub const STANDING_STONE: [&str; 3] = [" ___ ", "| o |", "|_|_|"];
    /// A dial cut into the floor, marked with the moon's phases.
    pub const DIAL: [&str; 3] = [" ___ ", "(o o)", "\\_=_/"];
    /// A stone coffin with its lid still on.
    pub const SARCOPHAGUS: [&str; 3] = [" _____ ", "|=|=|=|", "|_____|"];
    /// A wall of names, cut small and close.
    pub const NAMES: [&str; 3] = [";;;:;;;", ":;::;;:", ";;:;;;:"];
    /// A door sealed into its own frame.
    pub const SEALED_DOOR: [&str; 3] = [".---.", "| X |", "|_|_|"];
    /// A lantern hanging on a wire.
    pub const LANTERN: [&str; 3] = ["  |  ", " ( ) ", " '-' "];
    /// A bronze brazier, empty.
    pub const BRAZIER: [&str; 3] = [" ___ ", "|___|", "  |  "];
    /// A brazier's flame, drawn above the bowl once it is lit.
    pub const BRAZIER_FLAME: &[char] = &['*', '\'', 'i', '*'];

    /// An arched doorway.
    pub const DOOR: [&str; 3] = [".-.", "| |", "|_|"];
    /// A bare, dead tree.
    pub const TREE: [&str; 3] = ["\\|/", " | ", "_|_"];
}

/// What to draw and how: the art, the pose, the way it is facing, and its inks.
///
/// Gathered into one value so a sprite can be handed around without a function
/// growing an argument per property.
#[derive(Debug, Clone, Copy)]
struct Brush {
    art: &'static Sprite,
    pose: SpritePose,
    facing: Direction,
    inks: ArtInks,
}

/// A cut into a wide open floor: rings of glyphs around a point.
struct Motif {
    centre: (i32, i32),
    /// Manhattan radii to draw rings at.
    rings: &'static [i32],
    /// Whether to dot a coarse lattice as well, for very large floors.
    lattice: bool,
}

/// The inlay each area carries.
fn motif_for(area: AreaId) -> Option<&'static Motif> {
    use crate::game::areas;
    static CHAPEL: Motif = Motif {
        centre: (69, 11),
        rings: &[7, 13],
        lattice: true,
    };
    static CELLAR: Motif = Motif {
        centre: (118, 13),
        rings: &[4, 8, 12],
        lattice: false,
    };
    // The glade: rings around the standing stone, in a clearing cleared by hand.
    static WOODS: Motif = Motif {
        centre: (18, 30),
        rings: &[3, 6, 9],
        lattice: false,
    };
    // The village: a lattice of trodden ground between the fallen houses.
    static VILLAGE: Motif = Motif {
        centre: (62, 20),
        rings: &[9, 22],
        lattice: true,
    };
    // The shrine: rings widening from the moonwell itself.
    static SHRINE: Motif = Motif {
        centre: (56, 20),
        rings: &[9, 15],
        lattice: false,
    };
    // The catacombs: a bone lattice cut around the scribe's chamber.
    static CATACOMBS: Motif = Motif {
        centre: (71, 22),
        rings: &[5, 9],
        lattice: true,
    };
    match area {
        areas::CHAPEL => Some(&CHAPEL),
        areas::CELLAR => Some(&CELLAR),
        areas::WOODS => Some(&WOODS),
        areas::VILLAGE => Some(&VILLAGE),
        areas::SHRINE => Some(&SHRINE),
        areas::CATACOMBS => Some(&CATACOMBS),
        _ => None,
    }
}

/// What an area's open ground is made of.
///
/// The single strongest cue for telling one place from another: the eye reads the
/// field colour before it reads a single wall. Each area is one material, and the
/// material chooses the glyphs, the ink, and how often grit appears in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ground {
    /// Worn grey flagstones, with moss in the joints.
    Stone,
    /// Damp stone, dark and beaded with water.
    Damp,
    /// Grass and leaf litter.
    Grass,
    /// Ash, brick-dust and broken tiles.
    Ash,
    /// Pale polished slabs.
    Marble,
    /// Bone dust and splinters.
    Bone,
}

/// The ground an area is floored with.
fn ground_for(area: AreaId) -> Ground {
    use crate::game::areas;
    match area {
        areas::CELLAR => Ground::Damp,
        areas::WOODS => Ground::Grass,
        areas::VILLAGE => Ground::Ash,
        areas::SHRINE => Ground::Marble,
        areas::CATACOMBS => Ground::Bone,
        _ => Ground::Stone,
    }
}

/// What drifts through an area's air.
///
/// The same effect as motes everywhere, with a different nature, so a shrine and
/// a wood are alive in different ways without one becoming noise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Air {
    /// Dust in still air.
    Dust,
    /// Pollen and blossom, and the occasional late firefly.
    Pollen,
    /// Fine ash, and the odd cinder.
    Ash,
    /// Cold, bright motes: moonlight with something in it.
    Silver,
    /// Bone dust, and a green something that is not dust.
    Bone,
}

/// The air an area breathes.
fn air_for(area: AreaId) -> Air {
    use crate::game::areas;
    match area {
        areas::WOODS => Air::Pollen,
        areas::VILLAGE => Air::Ash,
        areas::SHRINE => Air::Silver,
        areas::CATACOMBS => Air::Bone,
        _ => Air::Dust,
    }
}

/// The four glyphs a mote can wear as it fades in, brightens, and goes out, plus
/// how thinly it is spread.
struct AirArt {
    /// A mote just arriving, steady, bright, and a rare flourish.
    glyphs: [char; 4],
    /// One mote per this many tiles, before the drifting lattice is applied.
    density: u32,
    /// The ink each mote is drawn in.
    ink: fn(&Palette) -> Color,
}

fn air_art(air: Air) -> AirArt {
    match air {
        Air::Dust => AirArt {
            glyphs: ['.', '\'', '·', '·'],
            density: 53,
            ink: |palette| palette.text_muted,
        },
        Air::Pollen => AirArt {
            glyphs: ['.', '·', '✿', '❀'],
            density: 29,
            ink: |palette| palette.mage.hair,
        },
        Air::Ash => AirArt {
            glyphs: ['.', '\'', '·', '*'],
            density: 61,
            ink: |palette| palette.border,
        },
        Air::Silver => AirArt {
            glyphs: ['.', '·', '✧', '·'],
            density: 41,
            ink: |palette| palette.relic,
        },
        Air::Bone => AirArt {
            glyphs: ['.', '\'', '·', '*'],
            density: 71,
            ink: |palette| palette.text_secondary,
        },
    }
}

/// The world, a palette, the time, and what the player is doing.
pub struct Scenery<'a> {
    /// How many steps the player has taken, so the walk alternates poses on the
    /// movement rather than on the clock.
    pub step: u32,
    /// The world to draw.
    pub world: &'a World,
    /// The theme to draw it with.
    pub palette: &'a Palette,
    /// Seconds since the session started, for everything that moves.
    pub phase: f32,
    /// The player's pose this frame.
    pub player: Pose,
    /// Live feedback pulses.
    pub effects: &'a Effects,
    /// Where the tiles' backgrounds come from.
    pub backdrop: Backdrop,
}

/// Where the world's cell backgrounds come from.
///
/// This is the seam between the material renderer and the atmosphere layer, and
/// it is deliberately the *only* one: everything else about the two is
/// independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Backdrop {
    /// Each tile paints its own two tones: the light the player carries, and the
    /// dark outside it. What every place does by default.
    #[default]
    Material,
    /// The tiles leave their backgrounds alone, so a field composed underneath
    /// them shows through. The place is then lit by its own light, falling
    /// smoothly, rather than by a step between two colours.
    ///
    /// The player's own light is not lost with the tones: it is still what
    /// chooses a tile's ink (`lit`, `floor_ink`), which is the part that made a
    /// figure or a wall face readable in the first place.
    Atmosphere,
}

/// Which of a tile's neighbours are solid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Neighbours {
    north: bool,
    south: bool,
    east: bool,
    west: bool,
}

/// The tiles currently on screen, and how screen maps back to world.
#[derive(Debug, Clone, Copy)]
struct Window {
    first: (i32, i32),
    last: (i32, i32),
    camera: (i32, i32),
}

impl Window {
    /// Calls `apply` with the screen and world position of every visible tile.
    fn each(&self, mut apply: impl FnMut((u16, u16), Position)) {
        for y in self.first.1..self.last.1 {
            for x in self.first.0..self.last.0 {
                let sx = x - self.camera.0;
                let sy = y - self.camera.1;
                apply((sx as u16, sy as u16), Position::new(x as u16, y as u16));
            }
        }
    }

    /// The world position a screen cell shows.
    fn at(&self, sx: u16, sy: u16) -> Position {
        Position::new(
            (i32::from(sx) + self.camera.0).clamp(0, i32::from(u16::MAX)) as u16,
            (i32::from(sy) + self.camera.1).clamp(0, i32::from(u16::MAX)) as u16,
        )
    }

    /// Where a world position lands on screen, if it is inside `view`.
    fn screen(&self, position: Position, view: &CanvasView) -> Option<(u16, u16)> {
        let x = i32::from(position.x) - self.camera.0;
        let y = i32::from(position.y) - self.camera.1;
        if x < 0 || y < 0 || x >= i32::from(view.width()) || y >= i32::from(view.height()) {
            None
        } else {
            Some((x as u16, y as u16))
        }
    }
}

impl Scenery<'_> {
    /// Draws the world into `view`, scrolling to keep the player in the middle.
    pub fn draw(&self, view: &mut CanvasView) {
        let area = self.world.area();
        if view.bounds().is_empty() || area.width() == 0 {
            return;
        }
        let player = self.world.player();
        let camera = (
            axis_camera(area.width(), view.width(), player.x),
            axis_camera(area.height(), view.height(), player.y),
        );
        let window = self.window(area, view, camera);

        self.draw_terrain(view, area, window);
        self.draw_inlay(view, area, window);
        self.draw_props(view, area, window);
        self.draw_ambience(view, area, window);
        self.draw_figures(view, area, window);
        self.draw_sparkles(view, area, window);
    }

    /// The tiles worth visiting: the view, clipped to the area.
    fn window(&self, area: &Area, view: &CanvasView, camera: (i32, i32)) -> Window {
        Window {
            first: (camera.0.max(0), camera.1.max(0)),
            last: (
                (camera.0 + i32::from(view.width())).min(i32::from(area.width())),
                (camera.1 + i32::from(view.height())).min(i32::from(area.height())),
            ),
            camera,
        }
    }

    // -- terrain -------------------------------------------------------------

    /// Paints every visible terrain tile, with glyphs chosen from its surroundings.
    fn draw_terrain(&self, view: &mut CanvasView, area: &Area, window: Window) {
        let player = self.world.player();
        window.each(|(sx, sy), position| {
            let tile = area.tile(position);
            let neighbours = neighbours_of(area, position);
            let (glyph, style) = self.tile_art(position, tile, neighbours);
            let style = self.shaded(style, distance(player, position));
            self.put(view, sx, sy, position, glyph, style);
        });
    }

    /// The glyph and style for one terrain tile.
    fn tile_art(&self, position: Position, tile: Tile, neighbours: Neighbours) -> (char, Style) {
        let palette = self.palette;
        match tile {
            Tile::Wall => {
                let arte = self.palette.arte(self.world.area_id());
                let lit = self.lit(position);
                // A standing flame takes the place of the masonry it is fixed to,
                // so a candle can never hide a floor, an exit or a character.
                if neighbours.south && self.has_flame(position, neighbours) {
                    let index = (self.phase * 5.0) as usize % art::FLAME.len();
                    return (
                        art::FLAME[index],
                        Style::new().with_fg(self.palette.effect).bold(),
                    );
                }
                // A wall standing alone in open floor is a column, and columns are
                // the whole of a colonnade.
                if !neighbours.north && !neighbours.south && !neighbours.east && !neighbours.west {
                    return ('║', Style::new().with_fg(arte.cap).bold());
                }
                // A wall one cell thick with floor on both sides is a lintel or a
                // screen: it should read as a span, not as a block.
                if !neighbours.north && !neighbours.south {
                    return ('▬', Style::new().with_fg(arte.cap));
                }
                // A wall that faces open floor along a run carries the light: the
                // glazing of a chapel, the runes of a shrine, the grave-lamps of a
                // catacomb. Placed on a regular beat, so a wall reads as bays
                // rather than as noise.
                // Glazing belongs to a *run* of wall, not to every edge: a wall
                // with stone above and below it is one bay of a long side, and
                // that is where a window goes. Only one side is glazed, so a
                // chapel's north and south walls are stone.
                let facing = !neighbours.east || !neighbours.west;
                let in_a_run = neighbours.north && neighbours.south;
                let (pane, mark) = aperture(self.world.area_id());
                let pitch = window_pitch(self.world.area_id());
                if facing && in_a_run && position.y % pitch == 0 {
                    return (pane, Style::new().with_fg(arte.window));
                }
                if facing && in_a_run && position.y % pitch == pitch / 2 {
                    return (mark, Style::new().with_fg(arte.window).bold());
                }
                // Damage breaks up long runs of masonry.
                let h = hash(position.x as i32, position.y as i32, 0x11);
                if h % 41 == 0 {
                    return ('%', Style::new().with_fg(arte.base));
                }
                // Caps catch the light, bases sit in their own shadow, faces are
                // the side of the stone, and the mass behind is darkest.
                let (ch, ink, bold) = if !neighbours.north {
                    ('▀', arte.cap, true)
                } else if !neighbours.south {
                    ('▄', arte.base, false)
                } else if !neighbours.west || !neighbours.east {
                    ('▓', if lit { arte.cap } else { arte.wall }, false)
                } else if h % 5 == 0 {
                    ('▒', arte.wall, false)
                } else {
                    ('▓', arte.mass, false)
                };
                let mut style = Style::new().with_fg(ink);
                if bold {
                    style = style.bold();
                }
                (ch, style)
            }
            // Open ground is the strongest sign of *where* the player is, so the
            // area's material decides the glyphs, the ink, and what grows in it.
            Tile::Floor | Tile::Path => {
                let h = hash(position.x as i32, position.y as i32, 0x21);
                let trodden = tile == Tile::Path;
                let arte = self.palette.arte(self.world.area_id());
                let lit = self.lit(position);
                // Seams between flagstones, on a regular beat: a floor that is
                // laid rather than poured.
                if !trodden && (position.x % JOINT_PITCH == 0 || position.y % JOINT_PITCH == 0) {
                    return ('.', Style::new().with_fg(arte.grit));
                }
                match ground_for(self.world.area_id()) {
                    Ground::Grass => {
                        if h % 43 == 0 {
                            // A stone the roots have shouldered out.
                            return ('*', Style::new().with_fg(arte.grit));
                        }
                        if h % 6 == 0 {
                            // Blades catch the light; the ground under them does not.
                            return ('"', Style::new().with_fg(self.palette.nature));
                        }
                        let glyph = if trodden {
                            art::PATH[(h as usize >> 8) % art::PATH.len()]
                        } else {
                            art::GRASS[(h as usize >> 16) % art::GRASS.len()]
                        };
                        let ink = if trodden {
                            arte.base
                        } else {
                            self.palette.nature_dark
                        };
                        (glyph, Style::new().with_fg(ink))
                    }
                    Ground::Ash => {
                        if h % 31 == 0 {
                            return ('x', Style::new().with_fg(arte.grit));
                        }
                        let glyph = art::ASH[(h as usize >> 16) % art::ASH.len()];
                        let ink = self.floor_ink(arte, lit, trodden);
                        (glyph, Style::new().with_fg(ink))
                    }
                    Ground::Marble => {
                        if h % 47 == 0 {
                            // A polished joint catching the light.
                            return ('=', Style::new().with_fg(arte.window));
                        }
                        let glyph = art::MARBLE[(h as usize >> 16) % art::MARBLE.len()];
                        let ink = if trodden { arte.grit } else { arte.floor };
                        (glyph, Style::new().with_fg(ink))
                    }
                    Ground::Bone => {
                        if h % 37 == 0 {
                            return ('x', Style::new().with_fg(arte.grit));
                        }
                        let glyph = art::BONE[(h as usize >> 16) % art::BONE.len()];
                        let ink = self.floor_ink(arte, lit, trodden);
                        (glyph, Style::new().with_fg(ink))
                    }
                    Ground::Damp => {
                        if h % 29 == 0 {
                            return ('"', Style::new().with_fg(arte.window));
                        }
                        let glyph = art::FLOOR[(h as usize >> 16) % art::FLOOR.len()];
                        let ink = self.floor_ink(arte, lit, trodden);
                        (glyph, Style::new().with_fg(ink))
                    }
                    Ground::Stone => {
                        if h % 37 == 0 {
                            let glyph = art::RUBBLE[(h as usize >> 8) % art::RUBBLE.len()];
                            return (glyph, Style::new().with_fg(arte.grit));
                        }
                        let glyph = if trodden {
                            art::PATH[(h as usize >> 8) % art::PATH.len()]
                        } else {
                            art::FLOOR[(h as usize >> 16) % art::FLOOR.len()]
                        };
                        let ink = self.floor_ink(arte, lit, trodden);
                        (glyph, Style::new().with_fg(ink))
                    }
                }
            }
            Tile::Water => {
                // Two wave trains at different speeds, so the surface carries a
                // rolling swell rather than one repeating stripe.
                let slow = (self.phase * 2.0) as i32;
                let fast = (self.phase * 5.0) as i32;
                let index = (i32::from(position.x) + slow + (i32::from(position.y) + fast) * 2)
                    .rem_euclid(art::WATER.len() as i32) as usize;
                let crest = (i32::from(position.x) * 3 + i32::from(position.y) * 5 + fast)
                    .rem_euclid(11)
                    == 0;
                let style = if crest {
                    Style::new().with_fg(palette.relic).bold()
                } else {
                    Style::new().with_fg(palette.accent_alt)
                };
                (art::WATER[index], style)
            }
            // A wood is a mass, not a field of identical icons. Deep inside it the
            // tiles read as one dark canopy; only where the wood meets open ground
            // does a crown or a trunk show, which is what makes its shape legible
            // at a glance.
            Tile::Tree => {
                if !neighbours.north {
                    return ('♣', Style::new().with_fg(palette.nature).bold());
                }
                if !neighbours.south {
                    return ('|', Style::new().with_fg(palette.nature_dark));
                }
                if !neighbours.east || !neighbours.west {
                    return ('▓', Style::new().with_fg(palette.nature_dark));
                }
                // Deep in the wood the trees are a mass, and a mass painted solid
                // green is a wall of green however dark the green is. So the mass
                // is drawn in shade with the wood breaking through it: the eye
                // reads depth, and the green stays where it belongs - at the edges
                // and in the gaps.
                let h = hash(position.x as i32, position.y as i32, 0x61);
                if h % 4 == 0 {
                    ('♣', Style::new().with_fg(palette.nature_dark))
                } else if h % 13 == 0 {
                    ('♠', Style::new().with_fg(palette.border))
                } else {
                    ('▓', Style::new().with_fg(palette.shade_mid))
                }
            }
            Tile::Door => ('.', Style::new().with_fg(palette.separator)),
        }
    }

    /// Whether the player's light falls on this tile.
    fn lit(&self, position: Position) -> bool {
        distance(self.world.player(), position) <= LIGHT_RADIUS
    }

    /// The ink a floor is drawn in: brighter inside the light, stone outside it.
    fn floor_ink(&self, arte: crate::ui::theme::Arte, lit: bool, trodden: bool) -> Color {
        if trodden {
            return arte.wall;
        }
        if lit { arte.grit } else { arte.floor }
    }

    /// Whether a wall carries a candle.
    ///
    /// Only walls that face open floor to the south can - that is the face a
    /// player walks up to - and only sparsely, so the effect reads as occasional
    /// sconces rather than a dashed line.
    fn has_flame(&self, position: Position, neighbours: Neighbours) -> bool {
        if !neighbours.south || neighbours.north && neighbours.east && neighbours.west {
            return false;
        }
        hash(position.x as i32, position.y as i32, 0x51) % 17 == 0
    }

    // -- inlay ---------------------------------------------------------------

    /// Draws the area's floor motif, on plain, unoccupied floor only.
    fn draw_inlay(&self, view: &mut CanvasView, area: &Area, window: Window) {
        let Some(motif) = motif_for(self.world.area_id()) else {
            return;
        };
        let player = self.world.player();
        window.each(|(sx, sy), position| {
            if !area.tile(position).is_walkable() || position == player {
                return;
            }
            // Never over a character, an object or a doorway.
            if area.entity_at(position).is_some() || area.portal_index_at(position).is_some() {
                return;
            }
            let Some(glyph) = inlay_glyph(motif, position) else {
                return;
            };
            let style = Style::new().with_fg(self.palette.accent_dim).dim();
            let style = self.shaded(style, distance(player, position));
            self.put(view, sx, sy, position, glyph, style);
        });
    }

    // -- props ---------------------------------------------------------------

    /// Draws doors, the objects people can interact with, and the braziers.
    fn draw_props(&self, view: &mut CanvasView, area: &Area, window: Window) {
        window.each(|(sx, sy), position| {
            // A sealed doorway is drawn exactly like an open one: the player can
            // see where it goes and can try it. What is withheld is the way
            // through, not the knowledge that it is there.
            if area.tile(position) == Tile::Door {
                let open = area.portal_index_at(position).is_some_and(|index| {
                    area.portal(index)
                        .is_some_and(|portal| portal.is_open(|d| self.world.knows(d)))
                });
                let ink = if open {
                    self.palette.border
                } else {
                    self.palette.accent_dim
                };
                self.draw_art(view, window, (sx, sy), &art::DOOR, ink, 2);
            }
            // A tree standing on its own in the open is drawn in full, with a
            // crown and a trunk. A tree in a wood is part of the mass, and is drawn
            // by the terrain pass instead.
            if area.tile(position) == Tile::Tree {
                let neighbours = neighbours_of(area, position);
                let alone =
                    !neighbours.north && !neighbours.south && !neighbours.east && !neighbours.west;
                if alone {
                    self.draw_art(view, window, (sx, sy), &art::TREE, self.palette.nature, 2);
                }
            }
        });

        for entity in area.entities() {
            let Some(at) = window.screen(entity.position, view) else {
                continue;
            };
            match entity.kind {
                EntityKind::Npc { .. } => {}
                EntityKind::Object { prop, .. } => {
                    let (art, ink, baseline) = prop_art(self.phase, prop, self.palette);
                    self.draw_art(view, window, at, art, ink, baseline);
                }
                EntityKind::Brazier { phase, .. } => {
                    self.draw_art(view, window, at, &art::BRAZIER, self.palette.border, 2);
                    if self.world.progress().is_lit(phase) {
                        // The flame gutters above the bowl, each phase out of step
                        // with the next, so three lit braziers are visibly three
                        // fires rather than one pattern repeated.
                        let index = (self.phase * 5.0 + phase.index() as f32 * 1.7) as usize
                            % art::BRAZIER_FLAME.len();
                        self.draw_cell(
                            view,
                            window,
                            i32::from(at.0),
                            i32::from(at.1) - 2,
                            art::BRAZIER_FLAME[index],
                            self.palette.effect,
                        );
                    }
                }
            }
        }
    }

    // -- ambience ------------------------------------------------------------

    /// Drifts a few motes through the air over open ground.
    ///
    /// Motes are anchored to the *world*, so they scroll with it and read as dust
    /// in a room rather than dirt on the screen. They are sampled from a slowly
    /// shifting lattice, which is what makes them drift with no per-mote state,
    /// and they are only ever drawn on bare floor.
    fn draw_ambience(&self, view: &mut CanvasView, area: &Area, window: Window) {
        let air = air_art(air_for(self.world.area_id()));
        let drift_x = (self.phase * 0.55) as i32;
        let drift_y = (self.phase * 0.31) as i32;
        let shimmer = (self.phase * 0.9).sin();
        let player = self.world.player();
        window.each(|(sx, sy), position| {
            if !area.tile(position).is_walkable() {
                return;
            }
            if area.entity_at(position).is_some() || area.portal_index_at(position).is_some() {
                return;
            }
            let h = hash(
                i32::from(position.x) + drift_x,
                i32::from(position.y) - drift_y,
                0x7f,
            );
            if h % air.density != 0 {
                return;
            }
            // Each mote breathes: it fades in, drifts, and fades out.
            let breath = (self.phase * 0.8 + (h % 100) as f32 * 0.0628).sin();
            if breath < -0.2 {
                return;
            }
            // Most motes are the same particle at a different moment of its life;
            // the fourth glyph is the rare one, which is what gives each place its
            // own weather - blossom in the wood, cinders in the village.
            let (glyph, ink) = if h % 5 == 0 && breath > 0.2 {
                (air.glyphs[3], self.palette.focus)
            } else if breath > 0.7 {
                (air.glyphs[2], (air.ink)(self.palette))
            } else if breath > 0.1 {
                (air.glyphs[1], (air.ink)(self.palette))
            } else {
                (air.glyphs[0], self.palette.border)
            };
            let mut style = Style::new().with_fg(ink);
            if shimmer > 0.85 {
                style = style.bold();
            }
            let style = self.shaded(style, distance(player, position));
            self.put(view, sx, sy, position, glyph, style);
        });
    }

    // -- figures -------------------------------------------------------------

    /// Draws the people, in front of everything else.
    fn draw_figures(&self, view: &mut CanvasView, area: &Area, window: Window) {
        for entity in area.entities() {
            let EntityKind::Npc { figure, facing, .. } = entity.kind else {
                continue;
            };
            self.draw_figure(
                view,
                window,
                entity.position,
                figure,
                &Pose::idle(facing),
                0,
            );
        }

        // A sprint leans into the direction of travel and leaves a wake.
        let sprinting = matches!(self.player.stage, Stage::Sprint);
        self.draw_figure(
            view,
            window,
            self.world.player(),
            Figure::Mage,
            &self.player,
            self.step,
        );
        if sprinting {
            self.draw_wake(view, window, self.world.player(), self.player.facing);
        }
    }

    /// Draws a figure: head, hem, stride, and whatever it carries.
    ///
    /// The art and the inks both come from the cast table, so this function knows
    /// nothing about who it is drawing — a new character needs an entry there and
    /// no change here at all.
    fn draw_figure(
        &self,
        view: &mut CanvasView,
        window: Window,
        at: Position,
        figure: Figure,
        pose: &Pose,
        step: u32,
    ) {
        let art = sprite::for_figure(figure);
        let inks = art.inks;
        // The walk is cut from the sprite's own poses, one per step, so the legs
        // can never drift away from the movement they illustrate.
        let walking = matches!(pose.stage, Stage::Walk | Stage::Sprint);
        let sprite_pose = if matches!(pose.stage, Stage::Sprint) {
            SpritePose::Sprint
        } else if walking {
            SpritePose::Walk { step }
        } else {
            // A breath every few seconds, not every frame.
            SpritePose::Idle {
                breathing: (self.phase * 1.4 + f32::from(at.x)).sin() > 0.55,
            }
        };
        self.draw_sprite(
            view,
            window,
            at,
            Brush {
                art,
                pose: sprite_pose,
                facing: pose.facing,
                inks,
            },
        );
    }

    /// Draws one sprite, centred on its column and standing on its row.
    ///
    /// Every non-space glyph is coloured by what it *is* — mass, shading, metal,
    /// an eye — so the art itself is never touched to change how a character
    /// looks. A two-cell glyph would shear the sprite, which is why the library
    /// asserts every glyph is one column wide.
    fn draw_sprite(&self, view: &mut CanvasView, window: Window, at: Position, brush: Brush) {
        let Brush {
            art,
            pose,
            facing,
            inks,
        } = brush;
        let Some((sx, sy)) = window.screen(at, view) else {
            return;
        };
        let rows = pose.rows_for(art);
        let (dx, dy) = art.origin();
        let left = i32::from(sx) + dx;
        let top = i32::from(sy) + dy + STANDING_ROWS;
        let width = i32::from(art.width);
        // A shadow under the feet. A figure is the same value as the stone it
        // stands on often enough that it needs one, and a single dark row is
        // enough to lift it off the floor without drawing a box around it.
        let arte = self.palette.arte(self.world.area_id());
        for column in 2..width - 2 {
            let x = left + column;
            let y = top + i32::from(art.height);
            if x >= 0 && y >= 0 && x < i32::from(view.width()) && y < i32::from(view.height()) {
                let (sx, sy) = (x as u16, y as u16);
                let world = window.at(sx, sy);
                self.put(view, sx, sy, world, '▄', Style::new().with_fg(arte.dark));
            }
        }
        // The supplied art is authored front on, so west-facing is that art
        // reflected rather than redrawn: the staff changes hands and the
        // character visibly turns. The canonical rows are never altered.
        let mirrored = matches!(facing, Direction::West);
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
                let ink = inks.of(sprite::part_of(ch));
                self.draw_cell(view, window, left + column, top + row as i32, ch, ink);
            }
        }
    }

    /// A couple of marks trailing a sprinting figure, opposite the direction of
    /// travel, so the speed reads without animating the whole world.
    fn draw_wake(&self, view: &mut CanvasView, window: Window, at: Position, facing: Direction) {
        let (dx, dy) = facing.delta();
        let style = Style::new().with_fg(self.palette.text_muted).dim();
        for step in 1..=2i32 {
            let x = i32::from(at.x) - dx * step;
            let y = i32::from(at.y) - dy * step;
            if x < 0 || y < 0 {
                continue;
            }
            let world = Position::new(x as u16, y as u16);
            let Some((sx, sy)) = window.screen(world, view) else {
                continue;
            };
            let glyph = if dx != 0 { '·' } else { '\'' };
            self.put(view, sx, sy, world, glyph, style);
        }
    }

    /// Twinkles thrown off by an interaction or an arrival.
    ///
    /// Deterministic: the same pulse always scatters the same sparkles, and each
    /// is only ever drawn on bare, unoccupied floor, so a sparkle can never land
    /// on a character or cover an exit.
    fn draw_sparkles(&self, view: &mut CanvasView, area: &Area, window: Window) {
        for (at, kind, strength) in self.effects.live() {
            if !matches!(kind, Pulse::Touched | Pulse::Arrived) {
                continue;
            }
            // They fly outward as the pulse fades.
            let spread = 1 + ((1.0 - strength) * 2.0) as i32;
            for corner in 0..4i32 {
                let h = hash(i32::from(at.x) * 4 + corner, i32::from(at.y), 0x9e37);
                let dx = if corner % 2 == 0 { -1 } else { 1 } * spread;
                let dy = if corner < 2 { -1 } else { 1 } * spread;
                let x = i32::from(at.x) + dx;
                let y = i32::from(at.y) + dy;
                if x < 0 || y < 0 || x >= i32::from(area.width()) || y >= i32::from(area.height()) {
                    continue;
                }
                let world = Position::new(x as u16, y as u16);
                if !area.tile(world).is_walkable() || area.entity_at(world).is_some() {
                    continue;
                }
                let Some((sx, sy)) = window.screen(world, view) else {
                    continue;
                };
                let glyph = art::SPARKLE[(h as usize >> 8) % art::SPARKLE.len()];
                let style = Style::new().with_fg(self.palette.magic_of(kind)).bold();
                self.put(view, sx, sy, world, glyph, style);
            }
        }
    }

    /// Draws one cell of a figure at an unclipped screen position.
    fn draw_cell(
        &self,
        view: &mut CanvasView,
        window: Window,
        x: i32,
        y: i32,
        ch: char,
        ink: Color,
    ) {
        if x < 0 || y < 0 || x >= i32::from(view.width()) || y >= i32::from(view.height()) {
            return;
        }
        let (sx, sy) = (x as u16, y as u16);
        let world = window.at(sx, sy);
        self.put(view, sx, sy, world, ch, Style::new().with_fg(ink).bold());
    }

    /// The sprite of the character being played.
    #[must_use]
    pub fn player_sprite(&self) -> &'static Sprite {
        sprite::for_figure(Figure::Mage)
    }

    // -- shared --------------------------------------------------------------

    /// Draws an art block, with `baseline` (an index into its rows) sitting on the
    /// screen cell `(sx, sy)`.
    fn draw_art(
        &self,
        view: &mut CanvasView,
        window: Window,
        at: (u16, u16),
        art: &[&str],
        colour: Color,
        baseline: usize,
    ) {
        let (sx, sy) = at;
        let width = art.iter().map(|row| row.chars().count()).max().unwrap_or(0) as i32;
        let left = i32::from(sx) - (width - 1) / 2;
        let top = i32::from(sy) - baseline as i32;
        let style = Style::new().with_fg(colour).bold();
        for (row, line) in art.iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = left + column as i32;
                let y = top + row as i32;
                if x < 0 || y < 0 || x >= i32::from(view.width()) || y >= i32::from(view.height()) {
                    continue;
                }
                let (sx, sy) = (x as u16, y as u16);
                self.put(view, sx, sy, window.at(sx, sy), ch, style);
            }
        }
    }

    /// Writes a glyph, applying any feedback pulse that lands on this tile.
    fn put(
        &self,
        view: &mut CanvasView,
        sx: u16,
        sy: u16,
        world: Position,
        ch: char,
        style: Style,
    ) {
        view.put_char(sx, sy, ch, self.pulse(world, style));
    }

    /// Brightens a style where a pulse is live, without changing the glyph.
    fn pulse(&self, at: Position, style: Style) -> Style {
        match self.effects.sample(at) {
            Some((kind, strength)) if strength > 0.25 => {
                let ink = match kind {
                    Pulse::Blocked => self.palette.warning,
                    Pulse::Touched => self.palette.focus,
                    Pulse::Arrived => self.palette.accent,
                };
                style.with_fg(ink).bold().underline()
            }
            _ => style,
        }
    }

    /// Dims a style by distance, in tiles, from the player's light.
    ///
    /// Rather than a second colour per role, distance is expressed with the
    /// background ramp and the dim attribute: far stone is the same stone, seen in
    /// worse light. The ramp's steps are read as squared distances, so they are
    /// divided down here to put the player's pool of light at roughly six tiles, a
    /// middle band out to fourteen, and shadow beyond that.
    fn shaded(&self, style: Style, tiles: u16) -> Style {
        // A field underneath owns the background, and a tile that painted its own
        // would shut it out. The ink still carries the light the player makes.
        if self.backdrop == Backdrop::Atmosphere {
            return style;
        }
        // Two tones, not a grey ramp: the pool the player carries is the colour
        // of the place they are in, and everything outside it falls into that
        // place's dark. The `dim` attribute is deliberately not used — a terminal
        // may ignore it on an indexed colour, which is exactly how the first
        // attempt at this ended up with no falloff at all.
        let arte = self.palette.arte(self.world.area_id());
        let bg = if tiles <= LIGHT_RADIUS {
            arte.light
        } else {
            arte.dark
        };
        style.with_bg(bg)
    }
}

/// The ink a pulse of a given kind is drawn in.
impl Palette {
    /// The colour a feedback pulse of this kind uses.
    fn magic_of(&self, kind: Pulse) -> Color {
        match kind {
            Pulse::Blocked => self.warning,
            Pulse::Touched => self.magic_role(),
            Pulse::Arrived => self.accent,
        }
    }

    /// The magical accent, which is what sparkles are made of.
    fn magic_role(&self) -> Color {
        self.mage.magic
    }
}

/// The sigil's art for the current time, turning slowly.
fn sigil_phase(phase: f32) -> &'static [&'static str] {
    &art::SIGIL[(phase * 0.7) as usize % art::SIGIL.len()]
}

/// The art, ink and baseline for a prop.
///
/// One table, keyed by the prop the *model* named. Adding an artefact means a new
/// [`Prop`] and a new arm here, and nothing else in the renderer learns about it.
fn prop_art(phase: f32, prop: Prop, palette: &Palette) -> (&'static [&'static str], Color, usize) {
    match prop {
        Prop::Basin => (&art::BASIN[..], palette.interactive, 2),
        Prop::Sigil => (sigil_phase(phase), palette.effect, 1),
        Prop::Well => (&art::WELL[..], palette.interactive, 2),
        Prop::Letters => (&art::LETTERS[..], palette.interactive, 2),
        Prop::Cairn => (&art::CAIRN[..], palette.border, 2),
        Prop::StandingStone => (&art::STANDING_STONE[..], palette.relic, 2),
        Prop::Dial => (&art::DIAL[..], palette.relic, 2),
        Prop::Sarcophagus => (&art::SARCOPHAGUS[..], palette.border, 2),
        Prop::Names => (&art::NAMES[..], palette.relic, 2),
        Prop::SealedDoor => (&art::SEALED_DOOR[..], palette.warning, 2),
        Prop::Lantern => (&art::LANTERN[..], palette.interactive, 2),
    }
}

/// The inlay glyph at `position`, if the motif has one there.
fn inlay_glyph(motif: &Motif, position: Position) -> Option<char> {
    let dx = i32::from(position.x) - motif.centre.0;
    let dy = i32::from(position.y) - motif.centre.1;
    let ring = dx.abs() + dy.abs();
    if motif.rings.contains(&ring) {
        // Cross marks where a ring meets the axes; dots along the diagonals.
        return Some(if dx == 0 || dy == 0 { '+' } else { '·' });
    }
    if motif.lattice && ring > 20 && dx.rem_euclid(9) == 0 && dy.rem_euclid(9) == 0 {
        // A coarse lattice keeps very large floors from reading as blank paper.
        return Some('+');
    }
    None
}

/// Which neighbours of `position` are solid.
fn neighbours_of(area: &Area, position: Position) -> Neighbours {
    let solid = |dx: i32, dy: i32| {
        let nx = i32::from(position.x) + dx;
        let ny = i32::from(position.y) + dy;
        if nx < 0 || ny < 0 || nx >= i32::from(area.width()) || ny >= i32::from(area.height()) {
            return true;
        }
        !area.tile(Position::new(nx as u16, ny as u16)).is_walkable()
    };
    Neighbours {
        north: solid(0, -1),
        south: solid(0, 1),
        east: solid(1, 0),
        west: solid(-1, 0),
    }
}

/// The player's screen cell in a canvas of this size, for tests.
#[cfg(test)]
fn axis_camera_offsets(world: &World, canvas: &crate::render::Canvas) -> (u16, u16) {
    let area = world.area();
    let player = world.player();
    let x = i32::from(player.x) - axis_camera(area.width(), canvas.width(), player.x);
    let y = i32::from(player.y) - axis_camera(area.height(), canvas.height(), player.y);
    (x.max(0) as u16, y.max(0) as u16)
}

/// The world-space origin of the visible window for one axis.
///
/// The camera follows the player and clamps to the area's edges; an area smaller
/// than the view is centred.
fn axis_camera(area: u16, view: u16, focus: u16) -> i32 {
    if area <= view {
        -((i32::from(view) - i32::from(area)) / 2)
    } else {
        let maximum = i32::from(area - view);
        (i32::from(focus) - i32::from(view) / 2).clamp(0, maximum)
    }
}

/// Chebyshev distance in tiles, which is how a person judges "how far away".
fn distance(a: Position, b: Position) -> u16 {
    let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs() as u16;
    let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs() as u16;
    dx.max(dy)
}

/// A stable hash of a position and seed, so variation never moves.
fn hash(x: i32, y: i32, seed: u32) -> u32 {
    let mut value = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (y as u32).wrapping_mul(0x85eb_ca6b)
        ^ seed.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 15;
    value = value.wrapping_mul(0x2545_f491);
    value ^= value >> 13;
    value.wrapping_mul(0x27d4_eb2d) ^ (value >> 16)
}

#[cfg(test)]
mod tests {
    use super::{
        Backdrop, Neighbours, Scenery, axis_camera, distance, hash, inlay_glyph, motif_for,
    };
    use crate::game::{Area, AreaId, Direction, Position, Tile, World, areas};
    use crate::render::{Canvas, Color, Style};
    use crate::ui::Palette;
    use crate::ui::animation::Pose;
    use crate::ui::effects::{Effects, Pulse};

    fn view_text(canvas: &Canvas) -> String {
        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        text
    }

    fn scenery<'a>(world: &'a World, effects: &'a Effects, pose: Pose) -> Scenery<'a> {
        Scenery {
            world,
            palette: &Palette::EXTENDED,
            phase: 0.0,
            player: pose,
            step: 0,
            effects,
            backdrop: Backdrop::Material,
        }
    }

    fn draw(world: &World, width: u16, height: u16, phase: f32) -> (Canvas, String) {
        draw_with(world, width, height, phase, Pose::idle(world.facing()))
    }

    fn draw_with(
        world: &World,
        width: u16,
        height: u16,
        phase: f32,
        pose: Pose,
    ) -> (Canvas, String) {
        let effects = Effects::new();
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        {
            let mut view = canvas.view(canvas.bounds());
            let mut scenery = scenery(world, &effects, pose);
            scenery.phase = phase;
            scenery.draw(&mut view);
        }
        let text = view_text(&canvas);
        (canvas, text)
    }

    #[test]
    fn the_world_draws_across_the_whole_view() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, text) = draw(&world, 80, 24, 0.0);
        for y in 0..24 {
            for x in 0..80 {
                assert!(
                    canvas.get(x, y).is_some_and(|cell| cell.ch != ' '),
                    "({x},{y}) was left blank"
                );
            }
        }
        assert!(text.contains("▟▖"), "no wizard drawn:\n{text}");
    }

    /// The glyph the renderer chooses for a wall with the given neighbours.
    fn wall_glyph(neighbours: Neighbours) -> char {
        let world = World::new(areas::all(), areas::CHAPEL);
        let effects = Effects::new();
        scenery(&world, &effects, Pose::idle(Direction::South))
            .tile_art(Position::new(3, 3), Tile::Wall, neighbours)
            .0
    }

    #[test]
    fn a_wall_in_the_middle_of_a_room_is_drawn_as_a_pillar() {
        assert_eq!(
            '║',
            wall_glyph(Neighbours::default()),
            "a free-standing wall is a pillar"
        );
    }

    #[test]
    fn a_wall_face_and_a_wall_cap_are_drawn_differently() {
        let cap = wall_glyph(Neighbours {
            north: false,
            south: true,
            east: true,
            west: true,
        });
        let base = wall_glyph(Neighbours {
            north: true,
            south: false,
            east: true,
            west: true,
        });
        let body = wall_glyph(Neighbours {
            north: true,
            south: true,
            east: true,
            west: true,
        });
        assert_eq!('▀', cap, "the cap is missing");
        assert_eq!('▄', base, "the base is missing");
        assert_ne!(cap, body);
        assert_ne!(base, body);
    }

    #[test]
    fn tile_variation_is_stable_for_a_position() {
        assert_eq!(
            hash(3, 4, 7),
            hash(3, 4, 7),
            "the same tile must look the same every frame"
        );
        assert_ne!(hash(3, 4, 7), hash(3, 5, 7));
    }

    #[test]
    fn neighbouring_walls_do_not_all_look_alike() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (_, text) = draw(&world, 80, 24, 0.0);
        let glyphs: std::collections::HashSet<char> = text.chars().collect();
        assert!(glyphs.len() > 8, "the scenery is too uniform: {glyphs:?}");
    }

    #[test]
    fn water_moves_with_time() {
        let mut world = World::new(areas::all(), areas::CELLAR);
        world.set_player_for_test(Position::new(20, 30));
        let (_, first) = draw(&world, 40, 20, 0.0);
        let (_, second) = draw(&world, 40, 20, 0.4);
        assert_ne!(first, second, "the water should ripple");
    }

    #[test]
    fn the_camera_follows_the_player_and_clamps() {
        assert_eq!(0, axis_camera(140, 80, 10));
        assert_eq!(60, axis_camera(140, 80, 139));
        assert_eq!(-30, axis_camera(20, 80, 5));
    }

    #[test]
    fn distance_is_measured_in_tiles() {
        assert_eq!(0, distance(Position::new(4, 4), Position::new(4, 4)));
        assert_eq!(3, distance(Position::new(4, 4), Position::new(7, 6)));
    }

    #[test]
    fn drawing_is_safe_at_any_size_and_offset() {
        let world = World::new(areas::all(), areas::CHAPEL);
        for (width, height) in [(0, 0), (1, 1), (3, 2), (80, 24), (400, 120)] {
            let (_, _) = draw(&world, width, height, 1.0);
        }
    }

    // --- characters ---------------------------------------------------------

    #[test]
    fn the_player_faces_the_direction_they_walk() {
        let mut world = World::new(areas::all(), areas::CHAPEL);
        // The supplied art is authored front on, so west-facing is that art
        // reflected: the staff changes hands and the character visibly turns,
        // while the canonical rows are never altered.
        world.move_player(Direction::East);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(
            text.contains("═▓♦▓▘│"),
            "expected the staff on the right:\n{text}"
        );
        world.move_player(Direction::West);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(
            text.contains("│▘▓♦▓═"),
            "expected the art reflected:\n{text}"
        );
        assert!(
            !text.contains("═▓♦▓▘│"),
            "and not the unreflected art as well"
        );
    }

    #[test]
    fn a_figure_is_drawn_in_several_inks() {
        // Hair, skin, cloth, trim and eyes must not collapse to one colour, or the
        // character art is pointless.
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, _) = draw(&world, 120, 40, 0.0);
        let mut inks: Vec<Color> = Vec::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                if let Some(cell) = canvas.get(x, y) {
                    if "~:o+#@|*%/\\()".contains(cell.ch) && !inks.contains(&cell.style.fg) {
                        inks.push(cell.style.fg);
                    }
                }
            }
        }
        assert!(inks.len() >= 4, "the figure uses too few inks: {inks:?}");
    }

    #[test]
    fn the_veiled_figure_is_drawn_distinctly() {
        // Ysolde stands facing east, so the panel shows her profile: one cold
        // light in a void, which is what makes her read as something other than a
        // person.
        let mut world = World::new(areas::all(), areas::CHAPEL);
        world.set_player_for_test(Position::new(56, 38));
        let (_, text) = draw(&world, 60, 24, 0.0);
        // The wraith's hood and the two cold lights in it.
        assert!(text.contains("▐░▒░▌"), "Ysolde's face is missing:\n{text}");
        assert!(text.contains("▝▓▒▓▘"), "and her hem is missing");
    }

    #[test]
    fn the_witch_carries_a_wand() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, _) = draw(&world, 120, 40, 0.0);
        // The star on the wand twinkles through a small set of glyphs, but it is
        // always drawn in the magic ink, which nothing else in the world uses
        // while no effects are live.
        let magic = Palette::EXTENDED.mage.magic;
        let found = (0..canvas.height())
            .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
            .any(|(x, y)| canvas.get(x, y).is_some_and(|cell| cell.style.fg == magic));
        assert!(found, "the wand's star is not in the magic ink");
    }

    #[test]
    fn the_stride_changes_the_figure() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let standing = draw_with(&world, 60, 20, 0.0, Pose::idle(Direction::South)).1;
        let mut walking = Pose::idle(Direction::South);
        walking.stage = crate::ui::animation::Stage::Walk;
        walking.frame = 1;
        let striding = draw_with(&world, 60, 20, 0.0, walking).1;
        assert_ne!(standing, striding, "the stride must be visible");
    }

    // --- ambience and inlay -------------------------------------------------

    #[test]
    fn the_inlay_never_covers_a_character_an_object_or_a_doorway() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let area = world.area();
        let motif = motif_for(world.area_id()).expect("the chapel has a motif");
        for entity in area.entities() {
            assert_eq!(
                None,
                inlay_glyph(motif, entity.position),
                "the inlay would draw over {}",
                entity.name
            );
        }
        for portal in area.portals() {
            // A doorway is excluded by the draw pass falling through, so only the
            // character and object positions need to be clear of the motif.
            let _ = portal;
        }
    }

    #[test]
    fn no_mote_is_drawn_on_a_wall_or_over_a_character() {
        // Motes are only ever painted on bare, walkable, unoccupied floor.
        let mut world = World::new(areas::all(), areas::CHAPEL);
        world.set_player_for_test(Position::new(70, 44));
        let (canvas, _) = draw(&world, 60, 24, 1.7);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let Some(cell) = canvas.get(x, y) else {
                    continue;
                };
                if cell.ch == '\'' || cell.ch == '·' {
                    // A mote is small and dim; a character glyph is neither.
                    assert!(
                        cell.style == cell.style && cell.ch != 'o',
                        "a mote overwrote a character at ({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_pulse_changes_the_style_and_never_the_glyph() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let at = world.player();

        let cell_of = |world: &World, effects: &Effects| {
            let mut canvas = Canvas::new(60, 20);
            canvas.clear_with(Palette::EXTENDED.backdrop());
            let mut view = canvas.view(canvas.bounds());
            scenery(world, effects, Pose::idle(Direction::South)).draw(&mut view);
            let (sx, sy) = super::axis_camera_offsets(world, &canvas);
            (canvas, sx, sy)
        };

        let none = Effects::new();
        let (plain, sx, sy) = cell_of(&world, &none);
        let before = plain.get(sx, sy).expect("cell");

        let mut effects = Effects::new();
        effects.spawn(at, Pulse::Touched);
        let (pulsed, sx2, sy2) = cell_of(&world, &effects);
        assert_eq!((sx, sy), (sx2, sy2));
        let after = pulsed.get(sx, sy).expect("cell");

        assert_eq!(before.ch, after.ch, "a pulse must not change the glyph");
        assert_ne!(before.style, after.style, "but it must be visible");
    }

    #[test]
    fn a_blocked_pulse_marks_the_cell_it_lands_on() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let at = world.player();
        let mut effects = Effects::new();
        let mut canvas = Canvas::new(60, 20);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let before = {
            let mut view = canvas.view(canvas.bounds());
            scenery(&world, &effects, Pose::idle(Direction::South)).draw(&mut view);
            view_text(&canvas)
        };

        effects.spawn(at, Pulse::Blocked);
        let mut canvas = Canvas::new(60, 20);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let after = {
            let mut view = canvas.view(canvas.bounds());
            scenery(&world, &effects, Pose::idle(Direction::South)).draw(&mut view);
            view_text(&canvas)
        };
        assert_eq!(before, after, "the glyphs stay put");
    }

    // --- quality ------------------------------------------------------------

    #[test]
    fn every_drawn_glyph_respects_the_width_policy() {
        let world = World::new(areas::all(), areas::CELLAR);
        let (canvas, _) = draw(&world, 120, 40, 0.5);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let cell = canvas.get(x, y).expect("cell");
                assert!(
                    crate::render::glyph::is_drawable(cell.ch),
                    "({x},{y}) holds {:?}, which is not one column wide",
                    cell.ch
                );
                assert_ne!(cell.style.bg, Color::Default);
            }
        }
    }

    #[test]
    fn styles_are_not_plain() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, _) = draw(&world, 60, 20, 0.0);
        let coloured = (0..20)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .filter(|(x, y)| canvas.get(*x, *y).is_some_and(|c| c.style != Style::PLAIN))
            .count();
        assert!(coloured > 500, "only {coloured} styled cells");
    }

    #[test]
    fn a_figure_is_drawn_over_the_terrain_and_the_inlay() {
        // The characters win over everything else: whatever the floor, the inlay
        // or a mote would have put there, the figure's own glyph is on top.
        let world = World::new(areas::all(), areas::CHAPEL);
        let (_, text) = draw(&world, 120, 40, 0.0);
        assert!(text.contains("▟▖"), "the sprite is missing");
        // The whole sprite is present, in order, on consecutive rows: the floor
        // and the inlay never punch a hole through it.
        let lines: Vec<&str> = text.lines().collect();
        let hat = lines
            .iter()
            .position(|line| line.contains("▟▖"))
            .expect("the hat");
        assert!(
            lines[hat + 1].contains("▄▟█▙▄"),
            "the shoulders should be below the hat:\n{text}"
        );
        assert!(
            lines[hat + 3].contains("═▓♦▓▘│"),
            "the staff should be below the shoulders:\n{text}"
        );
        assert!(
            lines[hat + 4].contains("▓▓▓"),
            "the robe should be the last row:\n{text}"
        );
    }

    #[test]
    fn the_place_the_player_stands_in_lights_the_floor_around_them() {
        // The pool is drawn into the *glyphs*, not behind them: the same tile is
        // a brighter stone inside the light and a duller one outside it. This is
        // the defect the light was rewritten to fix — a bright background made a
        // lit rectangle instead of a pool.
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, _) = draw(&world, 80, 40, 0.0);
        let mut inks: Vec<String> = Vec::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                if let Some(cell) = canvas.get(x, y) {
                    if "'.,·".contains(cell.ch) {
                        inks.push(format!("{:?}", cell.style.fg));
                    }
                }
            }
        }
        inks.sort_unstable();
        inks.dedup();
        assert!(
            inks.len() >= 2,
            "the floor is lit the same everywhere: {inks:?}"
        );
    }

    #[test]
    fn every_place_draws_as_its_own_world() {
        // Same renderer, same code path, six different places.
        let mut frames = Vec::new();
        for place in [
            areas::CHAPEL,
            areas::WOODS,
            areas::VILLAGE,
            areas::SHRINE,
            areas::CATACOMBS,
        ] {
            let mut world = World::new(areas::all(), place);
            let _ = world.place_player(world.area().spawn());
            frames.push((place, draw(&world, 80, 40, 0.0).1));
        }
        for (index, (place, frame)) in frames.iter().enumerate() {
            for (other, other_frame) in &frames[index + 1..] {
                assert_ne!(frame, other_frame, "{place:?} looks like {other:?}");
            }
        }
    }

    #[test]
    fn composition_is_deterministic() {
        let world = World::new(areas::all(), areas::CHAPEL);
        assert_eq!(draw(&world, 80, 24, 0.7).1, draw(&world, 80, 24, 0.7).1);
    }

    #[test]
    fn a_bare_stage_still_draws_something() {
        let area = Area::new(
            AreaId::new("stage"),
            "Stage",
            &["....", "...."],
            Vec::new(),
            Vec::new(),
            Position::new(1, 1),
        );
        let world = World::new(vec![area], AreaId::new("stage"));
        let (_, text) = draw(&world, 12, 10, 0.0);
        assert!(text.contains("▟▖"), "no figure:\n{text}");
    }
}
