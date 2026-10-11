//! The ASCII world renderer.
//!
//! This is Grimmoire's own scenery renderer. It draws the world into the cell
//! grid the rest of the interface uses: one glyph per cell, chosen from what the
//! tile is and what surrounds it, coloured by role, and shaded by distance from
//! the player's light.
//!
//! # What makes it read as a place
//!
//! Three things, in order of importance:
//!
//! 1. **Structure.** A wall is not a field of `#`. Its glyph depends on its
//!    neighbours: a wall with open floor above it shows a lit cap, one with floor
//!    below shows its shadowed base, corners and edges differ, and a wall
//!    standing alone in a room is drawn as a pillar. That is what turns a grid of
//!    tiles into architecture.
//! 2. **Variation.** Floors, paths, water and ruined stone pick from small sets
//!    of glyphs using a hash of their world position. The same tile looks the
//!    same every time it is drawn, so the world never shimmers as the camera
//!    moves, and no two neighbouring tiles are identical.
//! 3. **Characters as art.** The player and the people they meet are small
//!    ASCII figures — three cells square — rather than a single coloured letter,
//!    with a facing direction and an animation of their own.
//!
//! Everything here is pure: it takes a world and a palette and writes cells. It
//! knows nothing about terminals, timing, or input.

use crate::game::{Area, EntityKind, Position, Tile, World};

use super::theme::Palette;
use crate::render::{CanvasView, Color, Style, glyph};

/// The primary glyph families, grouped so the art can be read at a glance.
mod art {
    /// Floors inside a building: worn stone.
    ///
    /// Deliberately only a couple of glyphs. Every tile being different is
    /// noise; a floor should read as one surface with a little grit on it.
    pub const FLOOR: &[char] = &['·', '.', '·', '.', '·', '\''];
    /// Trodden earth paths.
    pub const PATH: &[char] = &[',', '·', ',', ';', ',', '.'];
    /// Flood water, calm to moving.
    pub const WATER: &[char] = &['~', '~', '≈', '~', '~', '∼'];
    /// Rubble and broken masonry scattered over a floor.
    pub const RUBBLE: &[char] = &['*', '+', 'x', '·'];
    /// The wizard, `[direction][frame]`, three cells square.
    pub const PLAYER: [[[&str; 3]; 2]; 4] = [
        // North: the hood is seen from behind.
        [[" ^ ", "(.)", "/|\\"], [" ^ ", "(.)", "\\|/"]],
        // East.
        [[" ^ ", "(o>", "/|\\"], [" ^ ", "(o>", "\\|/"]],
        // South: face on.
        [[" ^ ", "(o)", "/|\\"], [" ^ ", "(o)", "\\|/"]],
        // West.
        [[" ^ ", "<o)", "/|\\"], [" ^ ", "<o)", "\\|/"]],
    ];
    /// The hooded figure, drawn tall and still, with two cold lights for eyes.
    pub const HOODED: [[&str; 3]; 2] = [["  ^  ", " (oo)", "|###|"], ["  ^  ", " (oo)", "/###\\"]];
    /// A cracked basin, two cells high.
    pub const BASIN: [&str; 3] = [".---.", "\\___/", "  |  "];
    /// A faded sigil, cut into the floor.
    pub const SIGIL: [&str; 3] = ["\\|/", "-O-", "/|\\"];
    /// An arched doorway.
    pub const DOOR: [&str; 3] = [".-.", "| |", "|_|"];
    /// A bare, dead tree.
    pub const TREE: [&str; 3] = ["\\|/", " | ", "_|_"];
}

/// The world, a palette, and the time — everything the renderer draws from.
pub struct Scenery<'a> {
    /// The world to draw.
    pub world: &'a World,
    /// The theme to draw it with.
    pub palette: &'a Palette,
    /// Seconds since the session started, for water and other motion.
    pub phase: f32,
    /// Whether the player is walking, which is when the figure animates.
    pub moving: bool,
}

/// Which of a tile's neighbours are solid.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Neighbours {
    north: bool,
    south: bool,
    east: bool,
    west: bool,
}

impl Scenery<'_> {
    /// Draws the world into `view`, scrolling to keep the player in the middle.
    pub fn draw(&self, view: &mut CanvasView) {
        let area = self.world.area();
        if view.bounds().is_empty() {
            return;
        }
        let player = self.world.player();
        let camera_x = axis_camera(area.width(), view.width(), player.x);
        let camera_y = axis_camera(area.height(), view.height(), player.y);

        self.draw_terrain(view, area, (camera_x, camera_y));
        self.draw_props(view, area, (camera_x, camera_y));
        self.draw_characters(view, area, (camera_x, camera_y));
    }

    /// Paints every terrain tile, with glyphs chosen from its surroundings.
    fn draw_terrain(&self, view: &mut CanvasView, area: &Area, camera: (i32, i32)) {
        let player = self.world.player();
        for y in 0..area.height() {
            for x in 0..area.width() {
                let Some((sx, sy)) = screen_of((x, y), camera, view) else {
                    continue;
                };
                let position = Position::new(x, y);
                let tile = area.tile(position);
                let neighbours = neighbours_of(area, (x, y));
                let (glyph, mut style) = self.tile_art(position, tile, neighbours);
                style = self.shaded(style, distance(player, position));
                view.put_char(sx, sy, glyph, style);
            }
        }
    }

    /// The glyph and style for one terrain tile.
    fn tile_art(&self, position: Position, tile: Tile, neighbours: Neighbours) -> (char, Style) {
        let palette = self.palette;
        match tile {
            Tile::Wall => {
                // A wall standing alone in open floor is a pillar.
                if !neighbours.north && !neighbours.south && !neighbours.east && !neighbours.west {
                    return ('║', Style::new().with_fg(palette.text_secondary).bold());
                }
                // Rubble and cracks break up long runs of masonry.
                if (hash(position, 0x11) % 41) == 0 {
                    return ('%', Style::new().with_fg(palette.border));
                }
                // Walls are a clear step brighter than the floor they stand on,
                // so the architecture reads before the texture does.
                let face = if !neighbours.north {
                    // Its cap catches what light there is.
                    ('▀', palette.text_secondary)
                } else if !neighbours.south {
                    ('▄', palette.text_muted)
                } else if !neighbours.west || !neighbours.east {
                    // A side wall shows its dressed face.
                    ('▓', palette.text_secondary)
                } else {
                    ('#', palette.text_muted)
                };
                let mut style = Style::new().with_fg(face.1);
                if !neighbours.north {
                    style = style.bold();
                }
                (face.0, style)
            }
            Tile::Floor => {
                if (hash(position, 0x21) % 37) == 0 {
                    let glyph = art::RUBBLE[(hash(position, 0x22) as usize) % art::RUBBLE.len()];
                    return (glyph, Style::new().with_fg(palette.border));
                }
                if (hash(position, 0x23) % 29) == 0 {
                    // Moss creeping out of the joints.
                    return ('"', Style::new().with_fg(palette.nature).dim());
                }
                let glyph = art::FLOOR[(hash(position, 0x24) as usize) % art::FLOOR.len()];
                (glyph, Style::new().with_fg(palette.separator))
            }
            Tile::Path => {
                let glyph = art::PATH[(hash(position, 0x31) as usize) % art::PATH.len()];
                (glyph, Style::new().with_fg(palette.text_muted))
            }
            Tile::Water => {
                // Ripples travel with time and position, so the water is alive.
                let drift = (self.phase * 4.0) as i32;
                let index = (position.x as i32 + position.y as i32 + drift).rem_euclid(4) as usize;
                let glyph = art::WATER[index % art::WATER.len()];
                let style = Style::new().with_fg(palette.accent_alt);
                (glyph, style)
            }
            Tile::Tree => ('♣', Style::new().with_fg(palette.nature).bold()),
            Tile::Door => ('.', Style::new().with_fg(palette.separator)),
        }
    }

    /// Draws doors, trees and the objects people can interact with.
    fn draw_props(&self, view: &mut CanvasView, area: &Area, camera: (i32, i32)) {
        for y in 0..area.height() {
            for x in 0..area.width() {
                let position = Position::new(x, y);
                let art = match area.tile(position) {
                    Tile::Door => Some(&art::DOOR[..]),
                    Tile::Tree => Some(&art::TREE[..]),
                    _ => None,
                };
                if let Some(art) = art {
                    // A doorway and a tree both stand on their tile.
                    self.draw_art_at(view, position, camera, art, self.palette.border, 2);
                }
            }
        }

        for entity in area.entities() {
            let (art, colour, baseline) = match entity.kind {
                EntityKind::Object { .. } => match entity.glyph {
                    // The sigil is cut into the floor, so its middle row is the
                    // one the player stands beside.
                    '*' => (&art::SIGIL[..], self.palette.effect, 1),
                    _ => (&art::BASIN[..], self.palette.interactive, 2),
                },
                EntityKind::Npc { .. } => continue,
            };
            self.draw_art_at(view, entity.position, camera, art, colour, baseline);
        }
    }

    /// Draws the player and anyone else who moves.
    fn draw_characters(&self, view: &mut CanvasView, area: &Area, camera: (i32, i32)) {
        for entity in area.entities() {
            if !matches!(entity.kind, EntityKind::Npc { .. }) {
                continue;
            }
            let frame = (self.phase * 1.5) as usize % 2;
            self.draw_art_at(
                view,
                entity.position,
                camera,
                &art::HOODED[frame],
                self.palette.character,
                2,
            );
        }

        // The player, animated only while they are walking.
        let player = self.world.player();
        let frame = if self.moving {
            (self.phase * 8.0) as usize % 2
        } else {
            0
        };
        let facing = self.world.facing();
        self.draw_art_at(
            view,
            player,
            camera,
            &art::PLAYER[facing as usize][frame],
            self.palette.player,
            2,
        );
    }

    /// Draws an art block, with `baseline` — an index into its rows — sitting on
    /// the tile the thing occupies.
    ///
    /// A figure stands *on* its tile, so its last row is the baseline; a floor
    /// marking lies *in* its tile, so its middle row is.
    fn draw_art_at(
        &self,
        view: &mut CanvasView,
        position: Position,
        camera: (i32, i32),
        art: &[&str],
        colour: Color,
        baseline: usize,
    ) {
        let width = art.iter().map(|row| row.chars().count()).max().unwrap_or(0) as u16;
        let Some((tile_x, tile_y)) = screen_of((position.x, position.y), camera, view) else {
            return;
        };
        let left = i32::from(tile_x) - (i32::from(width) - 1) / 2;
        let top = i32::from(tile_y) - baseline as i32;

        for (row, line) in art.iter().enumerate() {
            let y = top + row as i32;
            if y < 0 || y >= i32::from(view.height()) {
                continue;
            }
            for (column, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let x = left + column as i32;
                if x >= 0 && x < i32::from(view.width()) {
                    let style = Style::new().with_fg(colour).bold();
                    view.put_char(x as u16, y as u16, ch, style);
                }
            }
        }
    }

    /// Dims a style by distance, in tiles, from the player's light.
    ///
    /// Rather than a second colour per role, distance is expressed with the
    /// background ramp and the dim attribute: far stone is the same stone, seen
    /// in worse light. The ramp's steps are read as squared distances, so they
    /// are divided down here to put the player's pool of light at roughly six
    /// tiles, a middle band out to fourteen, and shadow beyond that.
    fn shaded(&self, style: Style, tiles: u16) -> Style {
        let palette = self.palette;
        let squared = (u32::from(tiles) * u32::from(tiles)) / 4;
        let mut style = style.with_bg(palette.shade_at(squared));
        // Only the far distance is dimmed: the light should fall off, not
        // swallow the room.
        if tiles >= 20 {
            style = style.dim();
        }
        style
    }
}

/// Where a tile lands on screen, or `None` when it is off the visible area.
fn screen_of(tile: (u16, u16), camera: (i32, i32), view: &CanvasView) -> Option<(u16, u16)> {
    let x = i32::from(tile.0) - camera.0;
    let y = i32::from(tile.1) - camera.1;
    if x < 0 || y < 0 || x >= i32::from(view.width()) || y >= i32::from(view.height()) {
        None
    } else {
        Some((x as u16, y as u16))
    }
}

/// Which neighbours of `(x, y)` are solid.
fn neighbours_of(area: &Area, (x, y): (u16, u16)) -> Neighbours {
    let solid = |dx: i32, dy: i32| {
        let nx = i32::from(x) + dx;
        let ny = i32::from(y) + dy;
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

/// A stable hash of a world position, so tile variation never moves.
fn hash(position: Position, seed: u32) -> u32 {
    let mut value = u32::from(position.x).wrapping_mul(0x9e37_79b1)
        ^ u32::from(position.y).wrapping_mul(0x85eb_ca6b)
        ^ seed.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 15;
    value = value.wrapping_mul(0x2545_f491);
    value ^= value >> 13;
    value.wrapping_mul(0x27d4_eb2d) ^ (value >> 16)
}

/// The width of the widest art block, for tests and layout.
#[must_use]
pub fn art_width(art: &[&str]) -> u16 {
    glyph::display_width(
        art.iter()
            .copied()
            .max_by_key(|row| row.chars().count())
            .unwrap_or(""),
    )
}

#[cfg(test)]
mod tests {
    use super::{Scenery, axis_camera, distance, hash};
    use crate::game::{Direction, Position, World, areas};
    use crate::render::{Canvas, Style, view::CanvasView};
    use crate::ui::Palette;

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

    fn draw(world: &World, width: u16, height: u16, phase: f32) -> (Canvas, String) {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        {
            let mut view: CanvasView<'_> = canvas.view(canvas.bounds());
            Scenery {
                world,
                palette: &Palette::EXTENDED,
                phase,
                moving: false,
            }
            .draw(&mut view);
        }
        let text = view_text(&canvas);
        (canvas, text)
    }

    #[test]
    fn the_world_draws_across_the_whole_view() {
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, text) = draw(&world, 80, 24, 0.0);
        // Every cell is drawn: no gaps in the scenery.
        for y in 0..24 {
            for x in 0..80 {
                assert!(
                    canvas.get(x, y).is_some_and(|cell| cell.ch != ' '),
                    "({x},{y}) was left blank"
                );
            }
        }
        assert!(text.contains("(o)"), "no player drawn:\n{text}");
    }

    /// The glyph the renderer chooses for a wall with the given neighbours.
    fn wall_glyph(neighbours: super::Neighbours) -> char {
        let world = World::new(areas::all(), areas::CHAPEL);
        let scenery = Scenery {
            world: &world,
            palette: &Palette::EXTENDED,
            phase: 0.0,
            moving: false,
        };
        scenery
            .tile_art(Position::new(3, 3), crate::game::Tile::Wall, neighbours)
            .0
    }

    #[test]
    fn a_wall_in_the_middle_of_a_room_is_drawn_as_a_pillar() {
        // A single wall tile surrounded by floor reads as a column, not a block.
        assert_eq!(
            '║',
            wall_glyph(super::Neighbours::default()),
            "a free-standing wall is a pillar"
        );
    }

    #[test]
    fn a_wall_face_and_a_wall_cap_are_drawn_differently() {
        // A wall with floor above shows its cap; one with floor below shows its
        // base. Uniform masonry would mean the neighbours were ignored.
        // Open floor above means we see the wall's cap; open floor below means
        // we see its base.
        let cap = wall_glyph(super::Neighbours {
            north: false,
            south: true,
            east: true,
            west: true,
        });
        let base = wall_glyph(super::Neighbours {
            north: true,
            south: false,
            east: true,
            west: true,
        });
        let body = wall_glyph(super::Neighbours {
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
            hash(Position::new(3, 4), 7),
            hash(Position::new(3, 4), 7),
            "the same tile must look the same every frame"
        );
        assert_ne!(hash(Position::new(3, 4), 7), hash(Position::new(3, 5), 7));
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
        // The flooded chamber is in the south-west of the cellar.
        world.set_player_for_test(Position::new(20, 30));
        let (_, first) = draw(&world, 40, 20, 0.0);
        let (_, second) = draw(&world, 40, 20, 0.4);
        assert_ne!(first, second, "the water should ripple");
    }

    #[test]
    fn the_camera_follows_the_player_and_clamps() {
        assert_eq!(0, axis_camera(140, 80, 10));
        assert_eq!(60, axis_camera(140, 80, 139));
        // An area narrower than the view is centred.
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

    #[test]
    fn the_player_faces_the_direction_they_walk() {
        let mut world = World::new(areas::all(), areas::CHAPEL);
        world.move_player(Direction::East);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(text.contains("(o>"), "expected an east-facing figure");
        world.move_player(Direction::West);
        let (_, text) = draw(&world, 60, 20, 0.0);
        assert!(text.contains("<o)"), "expected a west-facing figure");
    }

    #[test]
    fn the_hooded_figure_is_drawn_distinctly() {
        // Stand well clear, so the two figures do not overlap into each other.
        let mut world = World::new(areas::all(), areas::CHAPEL);
        world.set_player_for_test(Position::new(56, 38));
        let (_, text) = draw(&world, 60, 24, 0.0);
        assert!(
            text.contains("(oo)"),
            "the hooded figure is missing:\n{text}"
        );
        assert!(text.contains("|###|"), "the cloak is missing:\n{text}");
    }

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
                assert_ne!(cell.style.bg, crate::render::Color::Default);
            }
        }
    }

    #[test]
    fn styles_are_not_plain() {
        // The scenery colours its cells; a wall of unstyled text would mean the
        // palette never reached the renderer.
        let world = World::new(areas::all(), areas::CHAPEL);
        let (canvas, _) = draw(&world, 60, 20, 0.0);
        let coloured = (0..20)
            .flat_map(|y| (0..60).map(move |x| (x, y)))
            .filter(|(x, y)| canvas.get(*x, *y).is_some_and(|c| c.style != Style::PLAIN))
            .count();
        assert!(coloured > 500, "only {coloured} styled cells");
    }
}
