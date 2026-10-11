//! The software pixel canvas.
//!
//! This is Grimmoire's own drawing surface, in the same spirit as
//! [`crate::render::Canvas`] but one level down: instead of a grid of character
//! cells it is a flat, opaque RGB bitmap that Kitty displays through its
//! graphics protocol.
//!
//! It is deliberately small and total. Coordinates are signed so the camera can
//! place things partly off-screen, every write clips, and no operation can panic
//! on out-of-range input. There are no layers, no blend modes beyond a single
//! un-premultiplied alpha blend, and no transforms: the scene renderer composes
//! by drawing in order, exactly like the cell canvas does.

/// An opaque 24-bit colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Black, the usual base for a scene that is filled before drawing.
    pub const BLACK: Rgb = Rgb::new(0, 0, 0);

    /// Creates a colour from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Creates a colour from `0xRRGGBB`, so art tables read like hex swatches.
    #[must_use]
    pub const fn hex(value: u32) -> Self {
        Rgb {
            r: ((value >> 16) & 0xff) as u8,
            g: ((value >> 8) & 0xff) as u8,
            b: (value & 0xff) as u8,
        }
    }

    /// The colour as `[r, g, b]`, ready for the PNG encoder.
    #[must_use]
    pub const fn to_array(self) -> [u8; 3] {
        [self.r, self.g, self.b]
    }

    /// Multiplies the colour by `factor`, clamping at the ends.
    ///
    /// Used for shading: a wall's face is the same masonry a step darker, so the
    /// palette stays coherent without a second hand-picked colour per surface.
    #[must_use]
    pub fn shaded(self, factor: f32) -> Rgb {
        let clamp = |v: u8| (v as f32 * factor).round().clamp(0.0, 255.0) as u8;
        Rgb::new(clamp(self.r), clamp(self.g), clamp(self.b))
    }

    /// Moves the colour `amount` of the way towards white (`amount >= 0`).
    #[must_use]
    pub fn lighten(self, amount: f32) -> Rgb {
        let t = amount.clamp(0.0, 1.0);
        Rgb::new(
            (255.0 - (255.0 - f32::from(self.r)) * (1.0 - t)).round() as u8,
            (255.0 - (255.0 - f32::from(self.g)) * (1.0 - t)).round() as u8,
            (255.0 - (255.0 - f32::from(self.b)) * (1.0 - t)).round() as u8,
        )
    }

    /// Linear interpolation towards `other`.
    ///
    /// Float interpolation is for setup code. The per-pixel path uses
    /// [`Rgb::mix`], which is integer maths: `f32::round` is a library call on
    /// targets without a round instruction, and forty thousand of them per frame
    /// is the difference between a smooth game and a stutter.
    #[must_use]
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t + 0.5) as u8;
        Rgb::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    /// Mixes towards `other` by `amount` out of 255, in integer maths.
    #[must_use]
    pub fn mix(self, other: Rgb, amount: u8) -> Rgb {
        let a = u32::from(amount);
        let inv = 255 - a;
        let mix = |from: u8, to: u8| div255(u32::from(to) * a + u32::from(from) * inv) as u8;
        Rgb::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }

    /// Scales the colour by `factor`, without a float round.
    #[must_use]
    pub fn scaled(self, factor: f32) -> Rgb {
        let scale = (factor.max(0.0) * 256.0) as u32;
        let apply = |v: u8| (((u32::from(v) * scale) >> 8).min(255)) as u8;
        Rgb::new(apply(self.r), apply(self.g), apply(self.b))
    }
}

/// A flat, opaque RGB bitmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelCanvas {
    width: u16,
    height: u16,
    pixels: Vec<Rgb>,
}

impl PixelCanvas {
    /// Creates a canvas, filled with `background`.
    #[must_use]
    pub fn new(width: u16, height: u16, background: Rgb) -> Self {
        let count = usize::from(width) * usize::from(height);
        PixelCanvas {
            width,
            height,
            pixels: vec![background; count],
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

    /// Whether `(x, y)` is inside the canvas.
    #[must_use]
    pub const fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32
    }

    /// The colour at `(x, y)`, or black when out of range.
    #[must_use]
    pub fn get(&self, x: i32, y: i32) -> Rgb {
        if !self.contains(x, y) {
            return Rgb::BLACK;
        }
        self.pixels[(y as usize) * usize::from(self.width) + (x as usize)]
    }

    /// Writes `colour` at `(x, y)`, ignoring positions off the canvas.
    pub fn set(&mut self, x: i32, y: i32, colour: Rgb) {
        if self.contains(x, y) {
            let index = (y as usize) * usize::from(self.width) + (x as usize);
            self.pixels[index] = colour;
        }
    }

    /// Blends `colour` over the existing pixel with 0..=255 opacity.
    ///
    /// Straight (un-premultiplied) alpha over an opaque destination, which is all
    /// the scene renderer needs: dust, light spills, and water sheen.
    pub fn blend(&mut self, x: i32, y: i32, colour: Rgb, alpha: u8) {
        if alpha == 0 || !self.contains(x, y) {
            return;
        }
        if alpha == 255 {
            self.set(x, y, colour);
            return;
        }
        let under = self.get(x, y);
        let a = u32::from(alpha);
        let inv = 255 - a;
        let mix =
            |top: u8, bottom: u8| (((u32::from(top) * a) + (u32::from(bottom) * inv)) / 255) as u8;
        self.set(
            x,
            y,
            Rgb::new(
                mix(colour.r, under.r),
                mix(colour.g, under.g),
                mix(colour.b, under.b),
            ),
        );
    }

    /// Fills a rectangle, clipped to the canvas.
    pub fn fill_rect(&mut self, x: i32, y: i32, width: i32, height: i32, colour: Rgb) {
        let Some(bounds) = self.clip(x, y, width, height) else {
            return;
        };
        for row in bounds.1..bounds.3 {
            let start = (row as usize) * usize::from(self.width) + bounds.0 as usize;
            let end = start + (bounds.2 - bounds.0) as usize;
            self.pixels[start..end].fill(colour);
        }
    }

    /// Blends a rectangle over the canvas, clipped.
    pub fn blend_rect(&mut self, x: i32, y: i32, width: i32, height: i32, colour: Rgb, alpha: u8) {
        let Some(bounds) = self.clip(x, y, width, height) else {
            return;
        };
        if alpha == 0 {
            return;
        }
        if alpha == 255 {
            self.fill_rect(x, y, width, height, colour);
            return;
        }
        // Ambient occlusion lays these over most of the scene, so the inner loop
        // indexes the buffer directly rather than going through `blend`.
        let a = u32::from(alpha);
        let inv = 255 - a;
        let mix = |top: u8, bottom: u8| div255(u32::from(top) * a + u32::from(bottom) * inv) as u8;
        let stride = usize::from(self.width);
        for row in bounds.1..bounds.3 {
            let start = row as usize * stride + bounds.0 as usize;
            let end = start + (bounds.2 - bounds.0) as usize;
            for pixel in &mut self.pixels[start..end] {
                *pixel = Rgb::new(
                    mix(colour.r, pixel.r),
                    mix(colour.g, pixel.g),
                    mix(colour.b, pixel.b),
                );
            }
        }
    }

    /// Draws a one-pixel horizontal line from `x0` to `x1` inclusive.
    pub fn hline(&mut self, x0: i32, x1: i32, y: i32, colour: Rgb) {
        let (from, to) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        self.fill_rect(from, y, to - from + 1, 1, colour);
    }

    /// Draws a one-pixel vertical line from `y0` to `y1` inclusive.
    pub fn vline(&mut self, x: i32, y0: i32, y1: i32, colour: Rgb) {
        let (from, to) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        self.fill_rect(x, from, 1, to - from + 1, colour);
    }

    /// Blends a horizontal line, for highlights and sheens.
    pub fn hline_blend(&mut self, x0: i32, x1: i32, y: i32, colour: Rgb, alpha: u8) {
        let (from, to) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        self.blend_rect(from, y, to - from + 1, 1, colour, alpha);
    }

    /// Draws a rectangle outline one pixel thick.
    pub fn rect_outline(&mut self, x: i32, y: i32, width: i32, height: i32, colour: Rgb) {
        if width <= 0 || height <= 0 {
            return;
        }
        self.hline(x, x + width - 1, y, colour);
        self.hline(x, x + width - 1, y + height - 1, colour);
        self.vline(x, y, y + height - 1, colour);
        self.vline(x + width - 1, y, y + height - 1, colour);
    }

    /// Draws a filled circle, clipped.
    pub fn fill_circle(&mut self, cx: i32, cy: i32, radius: i32, colour: Rgb) {
        if radius < 0 {
            return;
        }
        for dy in -radius..=radius {
            let span = ((radius * radius - dy * dy) as f32).sqrt().round() as i32;
            self.fill_rect(cx - span, cy + dy, span * 2 + 1, 1, colour);
        }
    }

    /// The pixels as a flat row-major slice, for bulk full-frame passes.
    ///
    /// The lighting pass touches every pixel; going through [`PixelCanvas::get`]
    /// and [`PixelCanvas::set`] would add a bounds check and a multiply to each
    /// of them, which is measurable at forty thousand pixels. Callers that use
    /// this are responsible for their own indexing.
    #[must_use]
    pub fn pixels(&self) -> &[Rgb] {
        &self.pixels
    }

    /// Mutable access to the pixels, for bulk full-frame passes.
    pub fn pixels_mut(&mut self) -> &mut [Rgb] {
        &mut self.pixels
    }

    /// The canvas scaled up by an integer factor, nearest-neighbour.
    ///
    /// For the preview tool: the logical scene is a couple of hundred pixels
    /// across, so it has to be enlarged before a human can judge it.
    #[must_use]
    pub fn scaled(&self, factor: u16) -> PixelCanvas {
        let factor = factor.max(1);
        let width = self.width * factor;
        let height = self.height * factor;
        let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height));
        for y in 0..height {
            let src_y = usize::from(y / factor) * usize::from(self.width);
            for x in 0..width {
                pixels.push(self.pixels[src_y + usize::from(x / factor)]);
            }
        }
        PixelCanvas {
            width,
            height,
            pixels,
        }
    }

    /// The whole bitmap as row-major RGB bytes, for the PNG encoder.
    #[must_use]
    pub fn as_rgb_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 3);
        for pixel in &self.pixels {
            out.extend_from_slice(&pixel.to_array());
        }
        out
    }

    /// Clips a rectangle to the canvas, returning `(x0, y0, x1, y1)`.
    fn clip(&self, x: i32, y: i32, width: i32, height: i32) -> Option<(i32, i32, i32, i32)> {
        if width <= 0 || height <= 0 {
            return None;
        }
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + width).min(self.width as i32);
        let y1 = (y + height).min(self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            None
        } else {
            Some((x0, y0, x1, y1))
        }
    }
}

/// Divides by 255 without the division.
///
/// The scene blends tens of thousands of pixels per frame, and an integer
/// division each is measurable. This is the exact reciprocal multiply for every
/// numerator below 65,536, which is the largest a blend of two 8-bit channels
/// can produce.
fn div255(value: u32) -> u32 {
    (value * 32_897) >> 23
}

/// A small deterministic value hash, used for per-tile variation.
///
/// Tile art must differ from tile to tile without being random per frame: the
/// same tile has to look the same every time it is drawn, or the world would
/// shimmer as the camera moved.
#[must_use]
pub fn hash2(x: i32, y: i32, seed: u32) -> u32 {
    let mut value = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (y as u32).wrapping_mul(0x85eb_ca6b)
        ^ seed.wrapping_mul(0xc2b2_ae35);
    value ^= value >> 15;
    value = value.wrapping_mul(0x2545_f491);
    value ^= value >> 13;
    value = value.wrapping_mul(0x27d4_eb2d);
    value ^ (value >> 16)
}

#[cfg(test)]
mod tests {
    use super::{PixelCanvas, Rgb, hash2};

    #[test]
    fn colours_round_trip_and_shade() {
        let colour = Rgb::hex(0x40_80_c0);
        assert_eq!(Rgb::new(0x40, 0x80, 0xc0), colour);
        assert_eq!([0x40, 0x80, 0xc0], colour.to_array());
        assert_eq!(Rgb::new(0x20, 0x40, 0x60), colour.shaded(0.5));
        assert_eq!(Rgb::new(0xa0, 0xc0, 0xe0), colour.lighten(0.5));
        assert_eq!(colour, colour.lerp(Rgb::BLACK, 0.0));
        assert_eq!(Rgb::BLACK, colour.lerp(Rgb::BLACK, 1.0));
    }

    #[test]
    fn writes_clip_to_the_canvas() {
        let mut canvas = PixelCanvas::new(4, 3, Rgb::BLACK);
        canvas.set(-1, 0, Rgb::new(1, 2, 3));
        canvas.set(4, 0, Rgb::new(1, 2, 3));
        canvas.set(0, -1, Rgb::new(1, 2, 3));
        assert_eq!(Rgb::BLACK, canvas.get(0, 0));
        canvas.set(3, 2, Rgb::hex(0x11_22_33));
        assert_eq!(Rgb::hex(0x11_22_33), canvas.get(3, 2));
    }

    #[test]
    fn rectangles_and_lines_stay_inside() {
        let mut canvas = PixelCanvas::new(8, 8, Rgb::BLACK);
        let white = Rgb::hex(0xff_ff_ff);
        canvas.fill_rect(-4, -4, 6, 6, white);
        assert_eq!(white, canvas.get(0, 0));
        assert_eq!(Rgb::BLACK, canvas.get(2, 2));
        assert_eq!(Rgb::BLACK, canvas.get(7, 7));

        canvas.hline(7, 20, 3, white);
        assert_eq!(white, canvas.get(7, 3));
        canvas.vline(5, -10, 1, white);
        assert_eq!(white, canvas.get(5, 0));
        assert_eq!(white, canvas.get(5, 1));
        assert_eq!(Rgb::BLACK, canvas.get(5, 2));
    }

    #[test]
    fn the_division_stands_in_for_a_real_one() {
        for value in [0u32, 1, 127, 128, 255, 256, 65_025, 65_535] {
            assert_eq!(value / 255, super::div255(value), "value {value}");
        }
        // The numerators that actually occur: `top * alpha + bottom * (255 - alpha)`.
        for alpha in [0u32, 1, 64, 128, 254, 255] {
            for top in [0u32, 1, 64, 128, 255] {
                for bottom in [0u32, 64, 200, 255] {
                    let numerator = top * alpha + bottom * (255 - alpha);
                    assert!(numerator <= 65_025, "the identity only holds below 65,536");
                    assert_eq!(numerator / 255, super::div255(numerator));
                }
            }
        }
    }

    #[test]
    fn blending_is_proportional() {
        let mut canvas = PixelCanvas::new(2, 1, Rgb::BLACK);
        canvas.blend(0, 0, Rgb::hex(0xff_ff_ff), 128);
        let half = canvas.get(0, 0);
        assert!((127..=129).contains(&half.r), "got {half:?}");
        canvas.blend(1, 0, Rgb::hex(0xff_ff_ff), 0);
        assert_eq!(Rgb::BLACK, canvas.get(1, 0));
    }

    #[test]
    fn iterated_blending_never_escapes_the_range() {
        let mut canvas = PixelCanvas::new(1, 1, Rgb::BLACK);
        for _ in 0..64 {
            canvas.blend(0, 0, Rgb::hex(0xff_00_00), 200);
        }
        let colour = canvas.get(0, 0);
        assert!(colour.r >= 250, "expected saturation, got {colour:?}");
        assert!(colour.g <= 5 && colour.b <= 5, "got {colour:?}");
    }

    #[test]
    fn circles_and_outlines_are_bounded() {
        let mut canvas = PixelCanvas::new(16, 16, Rgb::BLACK);
        let white = Rgb::hex(0xff_ff_ff);
        canvas.fill_circle(8, 8, 3, white);
        assert_eq!(white, canvas.get(8, 8));
        assert_eq!(Rgb::BLACK, canvas.get(8, 12));
        canvas.rect_outline(2, 2, 5, 5, white);
        assert_eq!(white, canvas.get(2, 2));
        assert_eq!(white, canvas.get(6, 6));
        assert_eq!(Rgb::BLACK, canvas.get(4, 4));
    }

    #[test]
    fn the_hash_is_stable_and_spread_out() {
        assert_eq!(hash2(3, 7, 1), hash2(3, 7, 1));
        assert_ne!(hash2(3, 7, 1), hash2(3, 8, 1));
        assert_ne!(hash2(3, 7, 1), hash2(3, 7, 2));
        // Across a 64x64 field the low bits should hit every bucket.
        let mut seen = [false; 8];
        for x in 0..8 {
            for y in 0..8 {
                seen[(hash2(x, y, 5) % 8) as usize] = true;
            }
        }
        assert!(seen.iter().all(|hit| *hit));
    }

    #[test]
    fn rgb_bytes_are_row_major_triples() {
        let mut canvas = PixelCanvas::new(2, 1, Rgb::BLACK);
        canvas.set(1, 0, Rgb::hex(0x01_02_03));
        assert_eq!(vec![0, 0, 0, 1, 2, 3], canvas.as_rgb_bytes());
    }
}
