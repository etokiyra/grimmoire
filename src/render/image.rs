//! CPU-side images that can be handed to a graphics backend.
//!
//! Grimmoire has no interest in being an image library. This is the minimum
//! needed to describe a picture the terminal could draw: an RGB buffer with a
//! handful of procedural constructors. There is no decoder, because nothing in
//! the game loads image files yet, and adding one would be a dependency without
//! a demonstrated need.

/// A packed RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Creates a colour from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// Linearly blends towards `other` by `t` in `[0, 1]`.
    #[must_use]
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
        Rgb::new(
            mix(self.r, other.r),
            mix(self.g, other.g),
            mix(self.b, other.b),
        )
    }
}

/// A top-left-origin RGB image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Image {
    /// A black image of the given size.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let len = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(3);
        Image {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    /// Builds an image from an existing RGB buffer, or `None` if the length is wrong.
    #[must_use]
    pub fn from_rgb(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(3)?;
        if pixels.len() != expected {
            return None;
        }
        Some(Image {
            width,
            height,
            pixels,
        })
    }

    /// A single flat colour.
    #[must_use]
    pub fn solid(width: u32, height: u32, colour: Rgb) -> Self {
        let mut image = Image::new(width, height);
        image.fill(colour);
        image
    }

    /// The image width in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// The image height in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The raw RGB buffer, three bytes per pixel.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// The colour at `(x, y)`, or black when out of bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> Rgb {
        if x >= self.width || y >= self.height {
            return Rgb::default();
        }
        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 3;
        Rgb::new(
            self.pixels[index],
            self.pixels[index + 1],
            self.pixels[index + 2],
        )
    }

    /// Sets the colour at `(x, y)`, ignoring out-of-bounds writes.
    pub fn set_pixel(&mut self, x: u32, y: u32, colour: Rgb) {
        if x >= self.width || y >= self.height {
            return;
        }
        let index = ((y as usize) * (self.width as usize) + (x as usize)) * 3;
        self.pixels[index] = colour.r;
        self.pixels[index + 1] = colour.g;
        self.pixels[index + 2] = colour.b;
    }

    /// Fills the whole image.
    pub fn fill(&mut self, colour: Rgb) {
        for chunk in self.pixels.chunks_exact_mut(3) {
            chunk[0] = colour.r;
            chunk[1] = colour.g;
            chunk[2] = colour.b;
        }
    }

    /// Fills an inclusive rectangle, clipped to the image.
    pub fn fill_rect(&mut self, x0: u32, y0: u32, x1: u32, y1: u32, colour: Rgb) {
        for y in y0..=y1.min(self.height.saturating_sub(1)) {
            for x in x0..=x1.min(self.width.saturating_sub(1)) {
                self.set_pixel(x, y, colour);
            }
        }
    }

    /// Scales with nearest-neighbour sampling. Good enough for backdrop art.
    #[must_use]
    pub fn scaled(&self, width: u32, height: u32) -> Image {
        if width == 0 || height == 0 || self.width == 0 || self.height == 0 {
            return Image::new(width, height);
        }
        let mut out = Image::new(width, height);
        for y in 0..height {
            let sy = y.saturating_mul(self.height) / height;
            for x in 0..width {
                let sx = x.saturating_mul(self.width) / width;
                out.set_pixel(
                    x,
                    y,
                    self.pixel(sx.min(self.width - 1), sy.min(self.height - 1)),
                );
            }
        }
        out
    }

    /// A vertical gradient between two colours.
    #[must_use]
    pub fn vertical_gradient(width: u32, height: u32, top: Rgb, bottom: Rgb) -> Image {
        let mut image = Image::new(width, height);
        if height == 0 {
            return image;
        }
        for y in 0..height {
            let t = y as f32 / (height.saturating_sub(1)).max(1) as f32;
            let colour = top.lerp(bottom, t);
            for x in 0..width {
                image.set_pixel(x, y, colour);
            }
        }
        image
    }

    /// A radial glow from `inner` at the centre out to `outer` at the rim.
    ///
    /// Used for sigils, portrait backdrops, and other soft "light" imagery.
    #[must_use]
    pub fn radial(width: u32, height: u32, inner: Rgb, outer: Rgb) -> Image {
        let mut image = Image::new(width, height);
        if width == 0 || height == 0 {
            return image;
        }
        let cx = (width as f32 - 1.0) / 2.0;
        let cy = (height as f32 - 1.0) / 2.0;
        let max_radius = (cx * cx + cy * cy).sqrt().max(1.0);
        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - cx;
                let dy = y as f32 - cy;
                let t = ((dx * dx + dy * dy).sqrt() / max_radius).clamp(0.0, 1.0);
                image.set_pixel(x, y, inner.lerp(outer, t));
            }
        }
        image
    }
}

#[cfg(test)]
mod tests {
    use super::{Image, Rgb};

    #[test]
    fn pixels_round_trip_and_clip() {
        let mut image = Image::new(4, 2);
        image.set_pixel(1, 1, Rgb::new(10, 20, 30));
        assert_eq!(Rgb::new(10, 20, 30), image.pixel(1, 1));
        assert_eq!(Rgb::default(), image.pixel(9, 9));
        image.set_pixel(9, 9, Rgb::new(1, 2, 3)); // ignored, not panicking
    }

    #[test]
    fn from_rgb_validates_the_length() {
        assert!(Image::from_rgb(2, 2, vec![0; 12]).is_some());
        assert!(Image::from_rgb(2, 2, vec![0; 11]).is_none());
    }

    #[test]
    fn gradients_run_between_their_ends() {
        let gradient = Image::vertical_gradient(3, 5, Rgb::new(0, 0, 0), Rgb::new(255, 255, 255));
        assert_eq!(Rgb::new(0, 0, 0), gradient.pixel(0, 0));
        assert_eq!(Rgb::new(255, 255, 255), gradient.pixel(0, 4));
    }

    #[test]
    fn radial_glows_from_the_centre() {
        let glow = Image::radial(11, 11, Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
        assert_eq!(Rgb::new(255, 255, 255), glow.pixel(5, 5));
        assert_eq!(Rgb::new(0, 0, 0), glow.pixel(0, 0));
    }

    #[test]
    fn scaling_preserves_corners_and_size() {
        let image = Image::vertical_gradient(2, 2, Rgb::new(0, 0, 0), Rgb::new(255, 0, 0));
        let scaled = image.scaled(8, 8);
        assert_eq!(8, scaled.width());
        assert_eq!(8, scaled.height());
        assert_eq!(image.pixel(0, 0), scaled.pixel(0, 0));
        assert_eq!(image.pixel(1, 1), scaled.pixel(7, 7));
    }

    #[test]
    fn scaling_to_zero_is_safe() {
        let image = Image::solid(4, 4, Rgb::new(1, 2, 3));
        let scaled = image.scaled(0, 0);
        assert_eq!(0, scaled.width());
        assert_eq!(0, scaled.height());
    }
}
