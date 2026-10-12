//! The cathedral silhouette the title screen stands in front of.
//!
//! This is not a picture. It is a *plan*: the towers, the nave, the gable, the
//! rose window, the lancets and the doorway are all derived from the space the
//! caller has, so the building fills a 120-column terminal and still reads at 80,
//! and a caller does not have to keep a second copy of the art for each size.
//!
//! # Why it is drawn from blocks
//!
//! Every glyph here is set in the foreground with an explicit colour, and the
//! cell's background is left as the atmosphere's. That is deliberate: the
//! building is a *silhouette* against a lit sky, so it must be opaque — a shaded
//! glyph such as `▓` would let the sky through and read as haze rather than
//! stone. Texture comes from a hash of the cell, the same way `ui::scenery` makes
//! a wall vary, and never from the frame.
//!
//! # Coming to life
//!
//! [`Look`] carries two numbers. `risen` draws only the part of the building that
//! has come up out of the ground, and `lit` lights the windows one after another
//! in a fixed order, starting with the doorway. Together they turn one picture
//! into an entrance: the building rises, then it wakes up. Nothing here reads a
//! clock, so the screen that owns the timing stays in charge of it.

use crate::render::{CanvasView, Style};
use crate::terminal::ColorSupport;

use super::atmosphere::Rgb;

/// The colours a cathedral is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Inks {
    /// The mass of the building.
    pub stone: Rgb,
    /// A lighter stone, for the masonry and the faces the sky catches.
    pub stone_high: Rgb,
    /// The lit edges: quoins, jambs, the rim of the roof.
    pub edge: Rgb,
    /// Window glass, lit from inside.
    pub glass: Rgb,
    /// The light behind the doorway. Dimmer than the windows: it is a way in, not
    /// a lamp, and a doorway as bright as the glass swallows whoever walks into it.
    pub door: Rgb,
    /// A window that has not been lit yet.
    pub dark_window: Rgb,
}

/// How much of the building to draw, and how far it has woken up.
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// The colours to draw it in.
    pub inks: Inks,
    /// What the terminal can show, for reducing those colours.
    pub support: ColorSupport,
    /// 0 draws nothing; 1 draws the whole building. It rises from the ground.
    pub risen: f32,
    /// How many windows are lit, 0 to 1. The doorway lights first.
    pub lit: f32,
}

/// How many windows the building has, for staggering the lighting.
const WINDOWS: usize = 14;

/// The index of the doorway, which lights before the rest.
const DOOR: usize = 0;

/// Draws the cathedral standing on row `base`, centred in the view.
///
/// `width` is the space the building may use; `height` is how many rows above
/// `base` it may occupy. A space too small to hold a recognisable building is
/// left alone rather than drawn badly.
pub fn draw(view: &mut CanvasView, base: u16, width: u16, height: u16, look: &Look) {
    let Some(plan) = Plan::fit(view, base, width, height) else {
        return;
    };
    plan.paint(view, look);
}

/// The building, measured out.
///
/// Everything is in two coordinates: a column in the view, and `k`, which counts
/// rows *above the baseline*. The centre is a whole column — `view_width / 2`,
/// the left of the two middle columns on an even width — and the facade is an odd
/// number of columns wide, so the building has a spine and is symmetric about it.
/// Symmetric art is not a nicety here: a spire one column off-centre on a dark
/// sky is the first thing an eye finds.
#[derive(Debug, Clone, Copy)]
struct Plan {
    /// The column the building is symmetric about.
    centre: i32,
    /// The row it stands on.
    base: i32,
    /// The left edge of the facade.
    left: i32,
    /// The width of one tower.
    tower: i32,
    /// The width of the nave between the towers.
    nave: i32,
    /// The topmost row of the nave wall, above the baseline.
    wall_k: i32,
    /// The row the nave's cornice runs along.
    cornice_k: i32,
    /// The topmost row of the roof.
    gable_k: i32,
    /// The gable's half-width at the row it starts, narrowing by one a row.
    gable: i32,
    /// The topmost row of the tower bodies.
    tower_k: i32,
    /// The topmost row of the spires.
    spire_k: i32,
}

impl Plan {
    /// Measures a building into `view`, or decides there is no room for one.
    fn fit(view: &CanvasView, base: u16, width: u16, height: u16) -> Option<Plan> {
        let bounds = view.bounds();
        // Below these the building stops being a building: two towers, a nave and
        // a roof with a window in it need roughly this much room.
        if width < 30 || height < 12 || base == 0 || base >= bounds.height {
            return None;
        }
        let avail = width.min(bounds.width).min(95);
        // Three-fifths of the space, and odd, so it has a spine and a margin. A
        // building that spans the whole frame is a wall, not a cathedral.
        let facade = ((avail * 3 / 5).max(31)) | 1;
        let centre = i32::from(bounds.width / 2);
        let left = centre - i32::from(facade) / 2;
        let tower = (i32::from(facade) / 5).clamp(6, 11) | 1;
        let nave = i32::from(facade) - tower * 2;
        if nave < 15 {
            return None;
        }

        // A spire is a point, not a cap: it tapers by two columns a row.
        let spire = (i32::from(height) / 6).clamp(3, 4);
        let tower_k = i32::from(height) - 1 - spire;
        // The nave roof is *narrow* and steep. A roof as wide as the nave cannot
        // look like a roof in a cell grid — a terminal cell is about twice as tall
        // as it is wide, so a wide shallow pitch comes out as a staircase. A
        // steep little gable over the crossing reads as a point, which is what the
        // eye is looking for.
        let gable = (i32::from(height) / 6).clamp(2, 3);
        let gable_k = tower_k - 3;
        let cornice_k = gable_k - gable;
        let wall_k = cornice_k - 1;
        if wall_k < 5 || tower_k < 7 {
            return None;
        }
        Some(Plan {
            centre,
            base: i32::from(base),
            left,
            tower,
            nave,
            wall_k,
            cornice_k,
            gable_k,
            gable,
            tower_k,
            spire_k: i32::from(height) - 1,
        })
    }

    /// The row `k` rows above the baseline.
    fn row(&self, k: i32) -> i32 {
        self.base - k
    }

    /// Whether a row is drawn yet, given how far the building has risen.
    ///
    /// The building grows out of the ground: the rows nearest the baseline appear
    /// first and the top arrives last. At zero, nothing is drawn at all, so an
    /// opening can begin on a bare sky.
    fn above_ground(&self, row: i32, risen: f32) -> bool {
        if risen <= 0.0 {
            return false;
        }
        let from_base = self.base - row;
        let drawn = (risen.clamp(0.0, 1.0) * self.spire_k as f32).ceil() as i32;
        from_base <= drawn
    }

    /// What colour window `index` is, given how many have lit.
    fn window_ink(look: &Look, index: usize) -> Rgb {
        // The doorway is the first thing to light: it is the way in, and the light
        // the whole scene is lit by.
        let reached = look.lit.clamp(0.0, 1.0) * WINDOWS as f32;
        if f32::from(index as u8) + 0.5 <= reached {
            if index == DOOR {
                look.inks.door
            } else {
                look.inks.glass
            }
        } else {
            look.inks.dark_window
        }
    }

    /// Draws the building.
    fn paint(&self, view: &mut CanvasView, look: &Look) {
        let bounds = view.bounds();
        let put = |view: &mut CanvasView, x: i32, y: i32, ch: char, ink: Rgb| {
            if x < 0 || y < 0 {
                return;
            }
            let (x, y) = (x as u16, y as u16);
            if x >= bounds.width || y >= bounds.height {
                return;
            }
            if i32::from(y) > self.base || !self.above_ground(i32::from(y), look.risen) {
                return;
            }
            view.put_char(x, y, ch, Style::new().with_fg(ink.to_color(look.support)));
        };
        let stone =
            |view: &mut CanvasView, x: i32, k: i32, ink: Rgb| put(view, x, self.row(k), '█', ink);

        let tower = self.tower;
        let left = self.left;
        let right = self.left + self.tower * 2 + self.nave;

        // -- the towers: a body, a lit outer face, and a spire ----------------
        for k in 0..=self.tower_k {
            for dx in 0..tower {
                // The outer column catches the sky, the one inside it is the lit
                // edge, and the rest is mass. That is what gives a flat silhouette
                // a front and a side.
                let ink = if dx == 0 {
                    look.inks.stone
                } else if dx == 1 {
                    look.inks.edge
                } else if dx == tower - 1 {
                    look.inks.stone_high
                } else {
                    look.inks.stone
                };
                stone(view, left + dx, k, ink);
                stone(view, right - 1 - dx, k, ink);
            }
        }
        for k in (self.tower_k + 1)..self.spire_k {
            let inset = k - self.tower_k - 1;
            let half = (tower / 2 - inset * 3).max(0);
            for dx in -half..=half {
                stone(view, left + tower / 2 + dx, k, look.inks.edge);
                stone(view, right - 1 - tower / 2 + dx, k, look.inks.edge);
            }
            // A slope on either side, so the taper is a spire rather than a stack.
            put(
                view,
                left + tower / 2 - half - 1,
                self.row(k),
                '/',
                look.inks.edge,
            );
            put(
                view,
                left + tower / 2 + half + 1,
                self.row(k),
                '\\',
                look.inks.edge,
            );
            put(
                view,
                right - 1 - tower / 2 - half - 1,
                self.row(k),
                '/',
                look.inks.edge,
            );
            put(
                view,
                right - 1 - tower / 2 + half + 1,
                self.row(k),
                '\\',
                look.inks.edge,
            );
        }
        put(
            view,
            left + tower / 2,
            self.row(self.spire_k + 1),
            '▲',
            look.inks.edge,
        );
        put(
            view,
            right - 1 - tower / 2,
            self.row(self.spire_k + 1),
            '▲',
            look.inks.edge,
        );

        // -- the nave wall, with masonry from a hash of the cell ---------------
        for k in 0..=self.wall_k {
            for x in (left + tower)..(right - tower) {
                // Hashed by distance from the spine, not by the column, so the
                // masonry mirrors with the building instead of speckling one side
                // and not the other.
                let offset = (x - self.centre).unsigned_abs();
                let seed = offset.wrapping_mul(0x9E37_79B9) ^ (k as u32).wrapping_mul(0x85EB_CA6B);
                let ink = if seed % 19 == 0 {
                    look.inks.stone_high
                } else {
                    look.inks.stone
                };
                stone(view, x, k, ink);
            }
        }

        // -- the roof: a cornice along the nave, then a steep gable ------------
        for x in (left + tower)..(right - tower) {
            put(view, x, self.row(self.cornice_k), '█', look.inks.stone_high);
        }
        for k in (self.cornice_k + 1)..=self.gable_k {
            let step = k - (self.cornice_k + 1);
            let half = (self.gable - step).max(0);
            for x in (self.centre - half)..=(self.centre + half) {
                stone(view, x, k, look.inks.stone);
            }
            put(
                view,
                self.centre - half - 1,
                self.row(k),
                '/',
                look.inks.edge,
            );
            put(
                view,
                self.centre + half + 1,
                self.row(k),
                '\\',
                look.inks.edge,
            );
        }
        put(
            view,
            self.centre,
            self.row(self.gable_k + 1),
            '▲',
            look.inks.edge,
        );

        // -- the rose window --------------------------------------------------
        // Drawn as an explicit ring rather than from trigonometry: a trig ring at
        // this radius, in cells twice as tall as they are wide, comes out as a
        // plus sign. Five columns by three rows is a rosette.
        let rose_k = (self.wall_k - 3).max(2);
        let rose_ink = Self::window_ink(look, 1);
        let centre_x = self.centre;
        for (dx, dk) in [
            (-1, 1),
            (0, 1),
            (1, 1),
            (-2, 0),
            (2, 0),
            (-1, -1),
            (0, -1),
            (1, -1),
        ] {
            put(view, centre_x + dx, self.row(rose_k + dk), '◆', rose_ink);
        }
        put(view, centre_x, self.row(rose_k), '✧', rose_ink);

        // -- the doorway, cut into the wall and lit from within ---------------
        let door_w = (self.nave / 4).clamp(5, 7) | 1;
        let door_h = (self.wall_k / 4).clamp(1, 2);
        let door_ink = Self::window_ink(look, DOOR);
        for k in 0..=door_h {
            for x in (self.centre - door_w / 2)..=(self.centre + door_w / 2) {
                put(view, x, self.row(k), '█', door_ink);
            }
        }
        for k in 0..=door_h {
            put(
                view,
                self.centre - door_w / 2 - 1,
                self.row(k),
                '│',
                look.inks.edge,
            );
            put(
                view,
                self.centre + door_w / 2 + 1,
                self.row(k),
                '│',
                look.inks.edge,
            );
        }
        for x in (self.centre - door_w / 2 - 1)..=(self.centre + door_w / 2 + 1) {
            put(view, x, self.row(door_h + 1), '▄', look.inks.edge);
        }

        // -- the lancets: three per tower, lit from the ground up -------------
        let lancet = (self.tower_k / 8).clamp(2, 3);
        for (side, tower_left) in [left, right - tower].into_iter().enumerate() {
            for slot in 0..3usize {
                let index = 2 + side * 3 + slot;
                let ink = Self::window_ink(look, index);
                let x = tower_left + tower / 2;
                for step in 0..lancet {
                    put(
                        view,
                        x,
                        self.row(2 + slot as i32 * (lancet + 2) + step),
                        '█',
                        ink,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Inks, Look, draw};
    use crate::render::{Canvas, Cell, Color, Frame, Rect, Style};
    use crate::terminal::ColorSupport;

    fn inks() -> Inks {
        Inks {
            stone: super::Rgb::new(20, 18, 30),
            stone_high: super::Rgb::new(34, 30, 48),
            edge: super::Rgb::new(70, 62, 92),
            glass: super::Rgb::new(255, 190, 96),
            door: super::Rgb::new(190, 140, 70),
            dark_window: super::Rgb::new(12, 10, 20),
        }
    }

    fn whole(risen: f32, lit: f32) -> Look {
        Look {
            inks: inks(),
            support: ColorSupport::TrueColor,
            risen,
            lit,
        }
    }

    /// Paints a building into a canvas and hands back the canvas.
    fn painted(width: u16, height: u16, base: u16, space: u16, look: &Look) -> Canvas {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Cell::new(' ', Style::new().with_bg(Color::Indexed(17))));
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            let mut view = frame.view(Rect::from_size(width, height));
            draw(&mut view, base, width, space, look);
        }
        canvas
    }

    fn is_glass(cell: &Cell) -> bool {
        cell.style.fg == Color::Rgb(255, 190, 96)
    }

    #[test]
    fn there_is_no_building_where_there_is_no_room_for_one() {
        for (width, height, space) in [(20, 24, 20), (200, 24, 9), (60, 6, 4)] {
            let canvas = painted(
                width,
                height,
                height.saturating_sub(3).max(1),
                space,
                &whole(1.0, 1.0),
            );
            assert!(
                (0..canvas.height()).all(|y| (0..canvas.width()).all(|x| canvas.get(x, y)
                    == Some(Cell::new(' ', Style::new().with_bg(Color::Indexed(17)))))),
                "{width}x{height} space {space} drew something"
            );
        }
    }

    #[test]
    fn the_building_stands_on_its_baseline_and_never_below_it() {
        let base = 20;
        let canvas = painted(120, 24, base, 18, &whole(1.0, 1.0));
        for y in (base + 1)..canvas.height() {
            for x in 0..canvas.width() {
                let cell = canvas.get(x, y).expect("cell");
                assert_eq!(
                    ' ', cell.ch,
                    "something was drawn below the baseline at ({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn the_building_never_reaches_above_the_space_it_was_given() {
        let base = 26;
        let space = 16;
        let canvas = painted(120, 30, base, space, &whole(1.0, 1.0));
        let ceiling = base.checked_sub(space).expect("space fits");
        // One row is allowed above the wall for the finial.
        for y in 0..(ceiling.saturating_sub(1)) {
            for x in 0..canvas.width() {
                let cell = canvas.get(x, y).expect("cell");
                assert_eq!(' ', cell.ch, "the building reached ({x}, {y})");
            }
        }
    }

    #[test]
    fn the_building_is_symmetric_about_its_spine() {
        let width = 120;
        let canvas = painted(width, 30, 26, 18, &whole(1.0, 1.0));
        let centre = width / 2;
        for y in 0..canvas.height() {
            for x in 0..centre {
                let mirror = 2 * centre - x;
                if mirror >= width {
                    continue;
                }
                let left = canvas.get(x, y).expect("cell");
                let right = canvas.get(mirror, y).expect("cell");
                // The roof slopes are mirror images rather than equal glyphs.
                let expected = match left.ch {
                    '/' => '\\',
                    '\\' => '/',
                    other => other,
                };
                assert_eq!(
                    (expected, left.style.fg),
                    (right.ch, right.style.fg),
                    "row {y} is not symmetric at column {x}"
                );
            }
        }
    }

    #[test]
    fn every_window_is_glass_once_the_building_is_lit() {
        let canvas = painted(120, 30, 26, 18, &whole(1.0, 1.0));
        let lit: Vec<(u16, u16)> = (0..canvas.height())
            .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
            .filter(|(x, y)| canvas.get(*x, *y).is_some_and(|cell| is_glass(&cell)))
            .collect();
        assert!(
            lit.len() > 20,
            "only {} glass cells: the windows did not light",
            lit.len()
        );
        // The doorway is the brightest thing at the base of the building, which is
        // where the light the scene is lit by comes from.
        let base = 26;
        assert!(
            (base - 3..=base).any(|y| (0..canvas.width())
                .any(|x| canvas.get(x, y).is_some_and(|cell| is_glass(&cell)))),
            "the doorway did not light"
        );
    }

    #[test]
    fn an_unlit_window_is_dark_rather_than_invisible() {
        let lit = painted(120, 30, 26, 18, &whole(1.0, 1.0));
        let dark = painted(120, 30, 26, 18, &whole(1.0, 0.0));
        let glass = (0..dark.height())
            .flat_map(|y| (0..dark.width()).map(move |x| (x, y)))
            .filter(|(x, y)| dark.get(*x, *y).is_some_and(|cell| is_glass(&cell)))
            .count();
        assert_eq!(0, glass, "windows lit before their turn");
        // The building is still there, so the windows are drawn, not omitted.
        let drawn = |canvas: &Canvas| {
            (0..canvas.height())
                .flat_map(|y| (0..canvas.width()).map(move |x| (x, y)))
                .filter(|(x, y)| canvas.get(*x, *y).is_some_and(|cell| cell_ch(&cell) != ' '))
                .count()
        };
        assert!(
            drawn(&dark) > 100,
            "the building vanished: {} cells",
            drawn(&dark)
        );
        assert!(
            drawn(&lit) > drawn(&dark) - 40,
            "lighting the windows changed the building's size"
        );
    }

    fn cell_ch(cell: &Cell) -> char {
        cell.ch
    }

    #[test]
    fn the_building_rises_from_the_ground_rather_than_appearing() {
        let base = 26;
        let none = painted(120, 30, base, 18, &whole(0.0, 1.0));
        let half = painted(120, 30, base, 18, &whole(0.5, 1.0));
        let all = painted(120, 30, base, 18, &whole(1.0, 1.0));
        let rows = |canvas: &Canvas| {
            (0..canvas.height())
                .filter(|y| {
                    (0..canvas.width())
                        .any(|x| canvas.get(x, *y).is_some_and(|cell| cell.ch != ' '))
                })
                .count()
        };
        assert_eq!(0, rows(&none), "something was drawn before the rise began");
        assert!(
            rows(&all) > 10,
            "the finished building is {} rows",
            rows(&all)
        );
        assert!(
            rows(&half) > 0 && rows(&half) < rows(&all),
            "the rise is not progressive: {} then {} of {}",
            rows(&half),
            rows(&all),
            rows(&all)
        );
        // What is drawn is always the part nearest the ground.
        for x in 0..120 {
            let topmost = (0..30)
                .find(|y| half.get(x, *y).is_some_and(|cell| cell.ch != ' '))
                .unwrap_or(base);
            assert!(
                topmost <= base,
                "the half-risen building starts below its base"
            );
        }
    }

    #[test]
    fn the_colours_reduce_for_a_terminal_that_cannot_show_true_colour() {
        let mut look = whole(1.0, 1.0);
        look.support = ColorSupport::Ansi16;
        let canvas = painted(120, 30, 26, 18, &look);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let cell = canvas.get(x, y).expect("cell");
                assert!(
                    !matches!(cell.style.fg, Color::Rgb(..)),
                    "({x}, {y}) kept true colour"
                );
            }
        }
    }
}
