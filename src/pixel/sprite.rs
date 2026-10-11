//! Sprites: pixel art authored as ASCII, drawn with transparency.
//!
//! A sprite is written in the source as rows of characters and a small palette
//! mapping characters to colours, so the art is **source-controlled and
//! editable** — you can see the shape in the code and change it with the editor
//! you already have. `.` is always transparent, which is what lets characters be
//! drawn over the floor without a mask image.
//!
//! This is where Grimmoire's two visual languages meet: the shapes are authored
//! in ASCII, and the result is pixel art rather than terminal cells.

use super::canvas::{PixelCanvas, Rgb};

/// A bitmap with a transparent background.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sprite {
    width: u16,
    height: u16,
    pixels: Vec<Option<Rgb>>,
}

impl Sprite {
    /// Builds a sprite from ASCII rows and a character palette.
    ///
    /// The width is the longest row; shorter rows are padded with transparency,
    /// and `.` or any character absent from the palette is transparent.
    #[must_use]
    pub fn from_rows(rows: &[&str], palette: &[(char, Rgb)]) -> Self {
        let width = rows
            .iter()
            .map(|row| row.chars().count())
            .max()
            .unwrap_or(0) as u16;
        let height = rows.len() as u16;
        let mut pixels = vec![None; usize::from(width) * usize::from(height)];
        for (y, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '.' {
                    continue;
                }
                if let Some((_, colour)) = palette.iter().find(|(key, _)| *key == ch) {
                    pixels[y * usize::from(width) + x] = Some(*colour);
                }
            }
        }
        Sprite {
            width,
            height,
            pixels,
        }
    }

    /// Builds a sprite by recolouring every opaque pixel of `source` with
    /// `colour`, keeping the shape. Used for silhouettes, shadows, and the
    /// player's ambient light.
    #[must_use]
    pub fn silhouette(source: &Sprite, colour: Rgb) -> Self {
        let pixels = source
            .pixels
            .iter()
            .map(|pixel| pixel.map(|_| colour))
            .collect();
        Sprite {
            width: source.width,
            height: source.height,
            pixels,
        }
    }

    /// The width in pixels.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// The height in pixels.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// Whether `(x, y)` is inside the sprite.
    #[must_use]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    /// The pixel at `(x, y)`, or `None` for transparent.
    #[must_use]
    pub fn pixel(&self, x: i32, y: i32) -> Option<Rgb> {
        if !self.contains(x, y) {
            return None;
        }
        self.pixels[(y as usize) * usize::from(self.width) + x as usize]
    }

    /// Draws the sprite with its top-left corner at `(x, y)`.
    pub fn draw(&self, canvas: &mut PixelCanvas, x: i32, y: i32) {
        for row in 0..self.height as i32 {
            for column in 0..self.width as i32 {
                if let Some(colour) = self.pixel(column, row) {
                    canvas.set(x + column, y + row, colour);
                }
            }
        }
    }

    /// Draws the sprite multiplied by `factor`, for shading.
    pub fn draw_shaded(&self, canvas: &mut PixelCanvas, x: i32, y: i32, factor: f32) {
        for row in 0..self.height as i32 {
            for column in 0..self.width as i32 {
                if let Some(colour) = self.pixel(column, row) {
                    canvas.set(x + column, y + row, colour.shaded(factor));
                }
            }
        }
    }

    /// Draws the sprite at partial opacity, for dust and faded surroundings.
    pub fn draw_blended(&self, canvas: &mut PixelCanvas, x: i32, y: i32, alpha: u8) {
        for row in 0..self.height as i32 {
            for column in 0..self.width as i32 {
                if let Some(colour) = self.pixel(column, row) {
                    canvas.blend(x + column, y + row, colour, alpha);
                }
            }
        }
    }

    /// The sprite mirrored left-to-right.
    ///
    /// Facing west is the mirror of facing east, and mirroring at runtime keeps
    /// one authoring copy of each pose instead of two that can drift apart.
    #[must_use]
    pub fn flipped_x(&self) -> Sprite {
        let mut pixels = vec![None; self.pixels.len()];
        for y in 0..self.height as usize {
            for x in 0..self.width as usize {
                pixels[y * self.width as usize + x] =
                    self.pixels[y * self.width as usize + (self.width as usize - 1 - x)];
            }
        }
        Sprite {
            width: self.width,
            height: self.height,
            pixels,
        }
    }

    /// The sprite scaled up by an integer factor, nearest-neighbour.
    ///
    /// Nearest-neighbour is the whole point: smoothing pixel art turns crisp
    /// sprites into mush. Used for the dialogue portrait, which is a character
    /// bust at a size the scene never shows.
    #[must_use]
    pub fn scaled(&self, factor: u16) -> Sprite {
        let factor = factor.max(1);
        let width = self.width * factor;
        let height = self.height * factor;
        let mut pixels = vec![None; usize::from(width) * usize::from(height)];
        for y in 0..height as i32 {
            for x in 0..width as i32 {
                pixels[(y as usize) * usize::from(width) + x as usize] =
                    self.pixel(x / i32::from(factor), y / i32::from(factor));
            }
        }
        Sprite {
            width,
            height,
            pixels,
        }
    }

    /// The tallest column of opaque pixels, used to anchor sprites on the ground.
    #[must_use]
    pub fn bottom_opaque_row(&self) -> u16 {
        for row in (0..self.height as i32).rev() {
            for column in 0..self.width as i32 {
                if self.pixel(column, row).is_some() {
                    return row as u16;
                }
            }
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::Sprite;
    use crate::pixel::canvas::{PixelCanvas, Rgb};

    const KEY: &[(char, Rgb)] = &[('x', Rgb::hex(0xff_00_00)), ('o', Rgb::hex(0x00_ff_00))];

    #[test]
    fn rows_become_pixels_and_dots_are_transparent() {
        let sprite = Sprite::from_rows(&["x.o", ".oo"], KEY);
        assert_eq!(3, sprite.width());
        assert_eq!(2, sprite.height());
        assert_eq!(Some(Rgb::hex(0xff_00_00)), sprite.pixel(0, 0));
        assert_eq!(None, sprite.pixel(1, 0));
        assert_eq!(Some(Rgb::hex(0x00_ff_00)), sprite.pixel(2, 1));
    }

    #[test]
    fn ragged_rows_are_padded_not_wrapped() {
        let sprite = Sprite::from_rows(&["xx", "x", "xxxx"], KEY);
        assert_eq!(4, sprite.width());
        assert_eq!(None, sprite.pixel(3, 0));
        assert_eq!(None, sprite.pixel(2, 1));
        assert_eq!(Some(Rgb::hex(0xff_00_00)), sprite.pixel(3, 2));
    }

    #[test]
    fn drawing_respects_transparency_and_clips() {
        let sprite = Sprite::from_rows(&["x.", ".x"], KEY);
        let mut canvas = PixelCanvas::new(3, 3, Rgb::BLACK);
        sprite.draw(&mut canvas, -1, -1);
        // (0,0) holds source (1,1) => red; source (0,0) fell off the canvas.
        assert_eq!(Rgb::hex(0xff_00_00), canvas.get(0, 0));
        assert_eq!(Rgb::BLACK, canvas.get(1, 0));
        assert_eq!(Rgb::BLACK, canvas.get(2, 2));
    }

    #[test]
    fn flipping_mirrors_the_columns() {
        let sprite = Sprite::from_rows(&["xo.", "..o"], KEY);
        let flipped = sprite.flipped_x();
        assert_eq!(sprite.width(), flipped.width());
        assert_eq!(sprite.pixel(0, 0), flipped.pixel(2, 0));
        assert_eq!(Some(Rgb::hex(0x00_ff_00)), flipped.pixel(0, 1));
    }

    #[test]
    fn flipping_twice_is_the_identity() {
        let sprite = Sprite::from_rows(&["xo.", "..o", "ooo"], KEY);
        assert_eq!(sprite, sprite.flipped_x().flipped_x());
    }

    #[test]
    fn scaling_is_nearest_neighbour() {
        let sprite = Sprite::from_rows(&[".xo", "oxo"], KEY);
        let big = sprite.scaled(3);
        assert_eq!(9, big.width());
        assert_eq!(6, big.height());
        // Every source pixel becomes a solid 3x3 block, and holes stay holes.
        assert_eq!(sprite.pixel(1, 0), big.pixel(3, 0));
        assert_eq!(sprite.pixel(1, 0), big.pixel(5, 2));
        assert_eq!(None, big.pixel(0, 0));
        assert_eq!(None, big.pixel(2, 1));
        assert_eq!(sprite.pixel(2, 1), big.pixel(6, 3));
    }

    #[test]
    fn a_silhouette_keeps_the_shape_and_changes_every_colour() {
        let sprite = Sprite::from_rows(&["xo", ".o"], KEY);
        let ghost = Sprite::silhouette(&sprite, Rgb::hex(0x10_20_30));
        assert_eq!(Some(Rgb::hex(0x10_20_30)), ghost.pixel(0, 0));
        assert_eq!(None, ghost.pixel(0, 1));
        assert_eq!(sprite.width(), ghost.width());
    }

    #[test]
    fn the_bottom_opaque_row_finds_the_lowest_pixel() {
        let sprite = Sprite::from_rows(&["xx", "..", "ox"], KEY);
        assert_eq!(2, sprite.bottom_opaque_row());
    }
}
