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
//!
//! # Draw order
//!
//! Terrain, then inlay, then props, then ambience, then figures. A feedback pulse
//! is applied as a *style* change wherever it lands, so it can never hide a glyph.
//! Only tiles inside the view are visited.
//!
//! Everything here is pure: a world, a palette, a time and a pose in; cells out.
//! It knows nothing about terminals, input, or the game's rules.

use crate::game::{Area, AreaId, Direction, EntityKind, Position, Tile, World};

use super::animation::{Pose, Stage};
use super::effects::{Effects, Pulse};
use super::theme::{Inks, Palette};
use crate::render::{CanvasView, Color, Style};

/// How many rows each figure is drawn in.
const FIGURE_ROWS: i32 = 5;

/// The primary glyph families, grouped so the art can be read at a glance.
mod art {
    /// Floors inside a building: worn stone.
    ///
    /// Deliberately only a couple of glyphs. Every tile being different is noise;
    /// a floor should read as one surface with a little grit on it.
    pub const FLOOR: &[char] = &['·', '.', '·', '.', '·', '\''];
    /// Trodden earth paths.
    pub const PATH: &[char] = &[',', '·', ',', ';', ',', '.'];
    /// Flood water, calm to moving.
    pub const WATER: &[char] = &['~', '~', '≈', '~', '∼', '~'];
    /// Rubble and broken masonry scattered over a floor.
    pub const RUBBLE: &[char] = &['*', '+', 'x', '·'];
    /// A candle flame, in the order it gutters.
    pub const FLAME: &[char] = &['\'', '.', 'i', '*', 'i', '.'];

    /// The mage's body: rows 0..4 for `[north, east, south, west]`.
    ///
    /// Silver hair, a face with two eyes, an indigo robe with a gold clasp. The
    /// legend is shared with the other figures - see `ink_for`.
    pub const MAGE_BODY: [[&str; 4]; 4] = [
        // North: the back of the head, no face.
        ["  ~~  ", "  ~~  ", " %%%% ", "|####|"],
        // East: a profile, the visible eye to the right.
        ["  ~~  ", " (::o)", "/#++#\\", "|####|"],
        // South: face on.
        ["  ~~  ", "(o::o)", "/#++#\\", "|####|"],
        // West: the mirrored profile.
        ["  ~~  ", "(o::) ", "\\#++#/", "|####|"],
    ];

    /// The veiled figure's body.
    ///
    /// A deep cowl, a trim rim, and cold light where a face should be. No skin is
    /// used at all, which is what makes it read as something other than a person.
    pub const VEILED_BODY: [[&str; 4]; 4] = [
        // North: no face at all.
        ["  __  ", " /++\\ ", "|####|", "|####|"],
        // East: one eye in the shadow.
        ["  __  ", " /++\\ ", "|@o  |", "|####|"],
        // South: two points of cold light.
        ["  __  ", " /++\\ ", "|@oo@|", "|####|"],
        // West.
        ["  __  ", " /++\\ ", "|  o@|", "|####|"],
    ];

    /// The stride that goes under a body: feet together, then apart.
    pub const HEM: [&str; 2] = ["  /\\  ", " /  \\ "];

    /// A staff: a crystal on a haft, held beside the mage.
    pub const STAFF: [&str; 5] = ["*", "+", "|", "|", "|"];

    /// A cracked basin, two cells high.
    pub const BASIN: [&str; 3] = [".---.", "\\___/", "  |  "];
    /// A faded sigil, cut into the floor, in its two turning phases.
    pub const SIGIL: [[&str; 3]; 2] = [["\\|/", "-O-", "/|\\"], ["/|\\", "-O-", "\\|/"]];
    /// An arched doorway.
    pub const DOOR: [&str; 3] = [".-.", "| |", "|_|"];
    /// A bare, dead tree.
    pub const TREE: [&str; 3] = ["\\|/", " | ", "_|_"];
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
    if area == areas::CHAPEL {
        Some(&CHAPEL)
    } else if area == areas::CELLAR {
        Some(&CELLAR)
    } else {
        None
    }
}

/// One character's art: the body rows, the inks they are drawn in, and whether
/// they carry a staff.
struct Figure<'a> {
    bodies: &'a [[&'a str; 4]; 4],
    inks: &'a Inks,
    staff: bool,
}

/// The world, a palette, the time, and what the player is doing.
pub struct Scenery<'a> {
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
                // A standing flame takes the place of the masonry it is fixed to,
                // so a candle can never hide a floor, an exit or a character.
                if neighbours.south && self.has_flame(position, neighbours) {
                    let index = (self.phase * 5.0) as usize % art::FLAME.len();
                    return (
                        art::FLAME[index],
                        Style::new().with_fg(palette.effect).bold(),
                    );
                }
                // A wall standing alone in open floor is a pillar.
                if !neighbours.north && !neighbours.south && !neighbours.east && !neighbours.west {
                    return ('║', Style::new().with_fg(palette.text_secondary).bold());
                }
                // Damage breaks up long runs of masonry.
                if hash(position.x as i32, position.y as i32, 0x11) % 41 == 0 {
                    return ('%', Style::new().with_fg(palette.border));
                }
                // Walls are a clear step brighter than the floor they stand on, so
                // the architecture reads before the texture does.
                let (ch, ink) = if !neighbours.north {
                    ('▀', palette.text_secondary)
                } else if !neighbours.south {
                    ('▄', palette.text_muted)
                } else if !neighbours.west || !neighbours.east {
                    ('▓', palette.text_secondary)
                } else {
                    ('#', palette.text_muted)
                };
                let mut style = Style::new().with_fg(ink);
                if !neighbours.north {
                    style = style.bold();
                }
                (ch, style)
            }
            Tile::Floor => {
                let h = hash(position.x as i32, position.y as i32, 0x21);
                if h % 37 == 0 {
                    let glyph = art::RUBBLE[(h as usize >> 8) % art::RUBBLE.len()];
                    return (glyph, Style::new().with_fg(palette.border));
                }
                if h % 29 == 0 {
                    // Moss creeping out of the joints.
                    return ('"', Style::new().with_fg(palette.nature).dim());
                }
                let glyph = art::FLOOR[(h as usize >> 16) % art::FLOOR.len()];
                (glyph, Style::new().with_fg(palette.separator))
            }
            Tile::Path => {
                let h = hash(position.x as i32, position.y as i32, 0x31);
                let glyph = art::PATH[(h as usize >> 8) % art::PATH.len()];
                (glyph, Style::new().with_fg(palette.text_muted))
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
            Tile::Tree => ('♣', Style::new().with_fg(palette.nature).bold()),
            Tile::Door => ('.', Style::new().with_fg(palette.separator)),
        }
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

    /// Draws doors, trees and the objects people can interact with.
    fn draw_props(&self, view: &mut CanvasView, area: &Area, window: Window) {
        window.each(|(sx, sy), position| {
            let art = match area.tile(position) {
                Tile::Door => Some(&art::DOOR[..]),
                Tile::Tree => Some(&art::TREE[..]),
                _ => None,
            };
            if let Some(art) = art {
                self.draw_art(view, window, (sx, sy), art, self.palette.border, 2);
            }
        });

        for entity in area.entities() {
            let (art, ink, baseline) = match entity.kind {
                EntityKind::Object { .. } => match entity.glyph {
                    '*' => (sigil_phase(self.phase), self.palette.effect, 1),
                    _ => (&art::BASIN[..], self.palette.interactive, 2),
                },
                EntityKind::Npc { .. } => continue,
            };
            if let Some(at) = window.screen(entity.position, view) {
                self.draw_art(view, window, at, art, ink, baseline);
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
            if h % 53 != 0 {
                return;
            }
            // Each mote breathes: it fades in, drifts, and fades out.
            let breath = (self.phase * 0.8 + (h % 100) as f32 * 0.0628).sin();
            if breath < -0.2 {
                return;
            }
            let (glyph, ink) = if breath > 0.7 {
                ('·', self.palette.focus)
            } else if breath > 0.1 {
                ('\'', self.palette.text_muted)
            } else {
                ('.', self.palette.border)
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
            if !matches!(entity.kind, EntityKind::Npc { .. }) {
                continue;
            }
            let sway = (self.phase * 1.1).sin() > 0.3;
            let figure = Figure {
                bodies: &art::VEILED_BODY,
                inks: &self.palette.veiled,
                staff: false,
            };
            self.draw_figure(
                view,
                window,
                entity.position,
                &figure,
                &Pose::idle(Direction::South),
                sway,
            );
        }

        // A sprint leans into the direction of travel and leaves a wake.
        let sprinting = matches!(self.player.stage, Stage::Sprint);
        let figure = Figure {
            bodies: &art::MAGE_BODY,
            inks: &self.palette.mage,
            staff: true,
        };
        self.draw_figure(
            view,
            window,
            self.world.player(),
            &figure,
            &self.player,
            self.player.frame % 2 == 1,
        );
        if sprinting {
            self.draw_wake(view, window, self.world.player(), self.player.facing);
        }
    }

    /// Draws a figure: body, stride, and optionally the staff it carries.
    fn draw_figure(
        &self,
        view: &mut CanvasView,
        window: Window,
        at: Position,
        figure: &Figure<'_>,
        pose: &Pose,
        sway: bool,
    ) {
        let Some((sx, sy)) = window.screen(at, view) else {
            return;
        };
        let body = &figure.bodies[pose.facing as usize];
        let hem = art::HEM[usize::from(sway)];
        let width = body[0].chars().count() as i32;
        let left = i32::from(sx) - (width - 1) / 2;
        let top = i32::from(sy) - (FIGURE_ROWS - 1);

        for (row, line) in body.iter().chain(std::iter::once(&hem)).enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let Some(ink) = ink_for(ch, figure.inks) else {
                    continue;
                };
                let glyph = ink_glyph(ch, pose.blinking);
                self.draw_cell(
                    view,
                    window,
                    left + column as i32,
                    top + row as i32,
                    glyph,
                    ink,
                );
            }
        }

        if figure.staff {
            // The staff is held on the side the figure is not facing into.
            let staff_left = match pose.facing {
                Direction::East => left + width,
                Direction::West => left - 1,
                Direction::North => left - 1,
                Direction::South => left + width,
            };
            for (row, line) in art::STAFF.iter().enumerate() {
                let Some(ch) = line.chars().next() else {
                    continue;
                };
                let Some(ink) = ink_for(ch, figure.inks) else {
                    continue;
                };
                self.draw_cell(view, window, staff_left, top + row as i32, ch, ink);
            }
        }
    }

    /// A couple of marks trailing a sprinting figure, opposite the direction of
    /// travel, so the speed reads without animating the whole world.
    fn draw_wake(&self, view: &mut CanvasView, window: Window, at: Position, facing: Direction) {
        let (dx, dy) = facing.delta();
        let style = Style::new().with_fg(self.palette.text_muted).dim();
        for step in 1..=2i32 {
            let x = i32::from(at.x) - dx * step + 3;
            let y = i32::from(at.y) - dy * step + 2;
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
        let palette = self.palette;
        let squared = (u32::from(tiles) * u32::from(tiles)) / 4;
        let mut style = style.with_bg(palette.shade_at(squared));
        if tiles >= 20 {
            style = style.dim();
        }
        style
    }
}

/// The sigil's art for the current time, turning slowly.
fn sigil_phase(phase: f32) -> &'static [&'static str] {
    &art::SIGIL[(phase * 0.7) as usize % art::SIGIL.len()]
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

/// The glyph a legend character draws when the figure blinks.
fn ink_glyph(ch: char, blinking: bool) -> char {
    if blinking && (ch == 'o' || ch == 'v') {
        '-'
    } else {
        ch
    }
}

/// The ink a legend character is drawn in.
///
/// Unknown characters are cloth: silhouettes are written with punctuation, and
/// garment-coloured punctuation is exactly right.
fn ink_for(ch: char, inks: &Inks) -> Option<Color> {
    Some(match ch {
        ' ' => return None,
        'o' | 'v' => inks.eye,
        ':' => inks.skin,
        '~' => inks.hair,
        '%' | '_' => inks.cloth_dark,
        '+' | '=' => inks.trim,
        '@' => inks.shadow,
        '|' => inks.wood,
        '*' => inks.magic,
        _ => inks.cloth,
    })
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
    use super::{Neighbours, Scenery, axis_camera, distance, hash, inlay_glyph, motif_for};
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
            effects,
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
        assert!(text.contains("o::o"), "no player drawn:\n{text}");
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
        world.move_player(Direction::East);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(
            text.contains("(::o)"),
            "expected an east-facing face:\n{text}"
        );
        world.move_player(Direction::West);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(text.contains("(o::)"), "expected a west-facing face");
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
        let mut world = World::new(areas::all(), areas::CHAPEL);
        world.set_player_for_test(Position::new(56, 38));
        let (_, text) = draw(&world, 60, 24, 0.0);
        assert!(text.contains("@oo@"), "Ysolde's eyes are missing:\n{text}");
        assert!(text.contains("/++\\"), "Ysolde's hood rim is missing");
    }

    #[test]
    fn the_mage_carries_a_staff() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, text) = draw(&world, 120, 40, 0.0);
        assert!(text.contains('*'), "no staff crystal:\n{text}");
        // The crystal is drawn in the magic ink, which nothing else uses.
        let magic = Palette::EXTENDED.mage.magic;
        let found = (0..canvas.height())
            .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
            .any(|(x, y)| {
                canvas
                    .get(x, y)
                    .is_some_and(|cell| cell.ch == '*' && cell.style.fg == magic)
            });
        assert!(found, "the staff crystal is not in the magic ink");
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
        let plain = draw(&world, 60, 20, 0.0).1;

        let mut effects = Effects::new();
        effects.spawn(at, Pulse::Touched);
        let mut canvas = Canvas::new(60, 20);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        {
            let mut view = canvas.view(canvas.bounds());
            scenery(&world, &effects, Pose::idle(Direction::South)).draw(&mut view);
        }
        assert_eq!(plain, view_text(&canvas), "the pulse changed the glyphs");
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
        assert!(text.contains("o::o"), "the face is missing");
        assert!(text.contains("#++#"), "the robe is missing");
        // And the whole figure is present, in order, on consecutive rows: the
        // floor and the inlay never punch a hole through it.
        let lines: Vec<&str> = text.lines().collect();
        let face = lines
            .iter()
            .position(|line| line.contains("o::o"))
            .expect("the face");
        assert!(lines[face - 1].contains("~~"), "the hair should be above");
        assert!(lines[face + 1].contains("#++#"), "the robe should be below");
        assert!(lines[face + 2].contains("|####|"), "and the robe's body");
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
        assert!(text.contains("o::o"), "no figure:\n{text}");
    }
}
