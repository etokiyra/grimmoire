//! A procedural true-colour atmosphere drawn underneath the glyphs.
//!
//! The renderer composes one glyph, one foreground colour and one background
//! colour per cell. That model cannot express a gradient: there is nowhere to put
//! a smooth change of colour. This module adds exactly one thing to it — a
//! **background layer** — without changing the renderer above it.
//!
//! # How it composes
//!
//! The atmosphere resolves a field of upper-half-block cells, [`HALF_BLOCK`],
//! whose foreground is the colour of the top half of the cell and whose
//! background is the colour of the bottom half. That is two vertical samples per
//! cell, so a field `h` rows tall is drawn from `2h` samples.
//!
//! It is painted first, into an otherwise empty frame. Every later draw keeps the
//! background it finds, because that is what [`Canvas::set`] already does for a
//! style that leaves its background at [`Color::Default`]:
//!
//! ```text
//!   layer 3   overlays   panels, menus              inherits the background
//!   layer 2   figures    the fixed sprites          inherits the background
//!   layer 1   scenery    architecture, glyphs       inherits the background
//!   layer 0   atmosphere the two-sample field       owns foreground *and* background
//! ```
//!
//! # What survives, and what does not
//!
//! Being precise about this, because it is the whole point:
//!
//! * A cell the atmosphere keeps is a half block carrying **two** samples.
//! * A cell any later layer draws a glyph into keeps only the **lower** sample,
//!   as that glyph's background; the upper sample is gone with the glyph slot.
//!   The error that introduces is bounded by half a row of the ramp, because the
//!   lower sample is half a row below the cell's centre.
//! * A later layer that sets its own background replaces both samples.
//!
//! So the resolution is two samples per cell in the open, one sample per cell
//! beneath a glyph, and the caller's own colour wherever the caller paints one.
//! The tests in this module assert each of those cases rather than assuming them.
//!
//! # Cadence and cost
//!
//! An animated full-screen true-colour field changes almost every cell every
//! frame, which defeats the presenter's diffing and is exactly how the first
//! prototype burned a tenth of a core to draw one screen. Three things keep this
//! module cheap:
//!
//! 1. **The field is cached.** [`Atmosphere::resolve`] regenerates it only when
//!    the clock crosses a tick or the area changes; between ticks [`paint`]
//!    blits a canvas that is byte-identical, so the presenter writes nothing.
//! 2. **The clock is quantised.** The sample time is derived from a whole tick
//!    index, never from a continuous elapsed time, so two resolves inside one
//!    tick are bit-identical. There is nothing to flicker.
//! 3. **The drift is bounded.** [`Drift`] says which rows are allowed to change.
//!    A resolve after a tick repaints only those rows, so a drifting sky band
//!    costs the band and not the screen.
//!
//! [`Atmosphere::next_update`] reports when the next tick is due, so a screen can
//! hand the loop a slow cadence for the atmosphere while characters and effects
//! keep their own.
//!
//! [`Canvas::set`]: crate::render::Canvas::set
//! [`paint`]: Atmosphere::paint

use std::ops::Range;
use std::time::Duration;

use crate::render::{Canvas, Cell, Color, Frame, Rect, Style};
use crate::terminal::ColorSupport;

/// The glyph that carries two vertical samples in one cell.
pub const HALF_BLOCK: char = '▀';

/// A colour in eight bits per channel, the atmosphere's own working type.
///
/// The renderer's [`Color`] is an enum of terminal colour spaces, which is the
/// wrong shape for interpolating a gradient. Ramps and fields are mixed here and
/// reduced to a [`Color`] only at the moment they are written to a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red, 0 to 255.
    pub r: u8,
    /// Green, 0 to 255.
    pub g: u8,
    /// Blue, 0 to 255.
    pub b: u8,
}

impl Rgb {
    /// Creates a colour from its three channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Rgb { r, g, b }
    }

    /// A linear interpolation towards `other`, with `t` clamped to `0..=1`.
    #[must_use]
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| -> u8 {
            let value = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
            value.clamp(0.0, 255.0) as u8
        };
        Rgb::new(
            blend(self.r, other.r),
            blend(self.g, other.g),
            blend(self.b, other.b),
        )
    }

    /// A perceptual brightness, 0 for black and 1 for white.
    ///
    /// Weighted the way an eye weighs the channels, so a test can assert that a
    /// field gets darker with distance without caring which channel carries it.
    #[must_use]
    pub fn luminance(self) -> f32 {
        (0.2126 * f32::from(self.r) + 0.7152 * f32::from(self.g) + 0.0722 * f32::from(self.b))
            / 255.0
    }

    /// The renderer colour for this value, reduced to what `support` can show.
    ///
    /// A terminal that cannot do 24-bit colour gets the nearest entry of its own
    /// palette instead: the 6×6×6 cube for 256-colour, and the nearest classic
    /// colour for 16. The picture is the same, with fewer steps.
    ///
    /// The result is never [`Color::Default`], whatever is asked for. A field is
    /// drawn into every cell it covers, so a cell whose colour fell back to "the
    /// terminal's own" would let the user's background show through the game.
    #[must_use]
    pub fn to_color(self, support: ColorSupport) -> Color {
        match support {
            ColorSupport::TrueColor => Color::Rgb(self.r, self.g, self.b),
            ColorSupport::Ansi256 => Color::Indexed(nearest_256(self)),
            ColorSupport::Ansi16 => match Color::Rgb(self.r, self.g, self.b).nearest_basic() {
                // The nearest-classic table is topped by white, which is stored
                // as the default slot; a background cannot be the default.
                Color::Default => Color::White,
                colour => colour,
            },
        }
    }
}

/// The nearest index of the terminal's 6×6×6 colour cube.
fn nearest_256(colour: Rgb) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |value: u8| -> u8 {
        let mut best = 0u8;
        let mut best_distance = u8::MAX;
        for (index, step) in STEPS.iter().enumerate() {
            let distance = value.abs_diff(*step);
            if distance < best_distance {
                best_distance = distance;
                best = index as u8;
            }
        }
        best
    };
    let (r, g, b) = (nearest(colour.r), nearest(colour.g), nearest(colour.b));
    16 + r * 36 + g * 6 + b
}

/// An ordered colour ramp, sampled by position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ramp {
    stops: Vec<Rgb>,
}

impl Ramp {
    /// Creates a ramp from its stops, top to bottom.
    ///
    /// # Panics
    ///
    /// Panics if fewer than two stops are given: a ramp of one colour is a
    /// configuration mistake, not a smooth change, and it should be loud.
    #[must_use]
    pub fn new(stops: impl Into<Vec<Rgb>>) -> Self {
        let stops = stops.into();
        assert!(stops.len() >= 2, "a ramp needs at least two stops");
        Ramp { stops }
    }

    /// The stops this ramp was built from.
    #[must_use]
    pub fn stops(&self) -> &[Rgb] {
        &self.stops
    }

    /// Samples the ramp at `t`, clamped to `0..=1`.
    #[must_use]
    pub fn sample(&self, t: f32) -> Rgb {
        let last = self.stops.len() - 1;
        let position = t.clamp(0.0, 1.0) * last as f32;
        // The floor can equal `last` at exactly t == 1, so clamp the pair index.
        let index = (position.floor() as usize).min(last - 1);
        self.stops[index].mix(self.stops[index + 1], position - index as f32)
    }
}

/// Where and when to sample a field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Probe {
    /// Horizontal position, 0 at the left edge and 1 at the right.
    pub u: f32,
    /// Vertical position, 0 at the top and 1 at the bottom.
    pub v: f32,
    /// The field's width divided by its height *in pixels*, so a shape drawn in
    /// this space comes out round rather than stretched.
    pub aspect: f32,
    /// Seconds on the field's clock.
    pub time: f32,
}

/// A moon: a disc with a smooth halo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Moon {
    /// Horizontal position, 0 to 1.
    pub u: f32,
    /// Vertical position, 0 to 1.
    pub v: f32,
    /// The radius of the bright disc, in `u` units.
    pub radius: f32,
    /// How far the halo reaches, in `u` units. Must exceed `radius`.
    pub halo: f32,
}

impl Moon {
    /// The light the moon sheds at `(u, v)`: 1 inside the disc, falling smoothly
    /// to 0 at `halo`, and 0 beyond it.
    ///
    /// The falloff is a quadratic in the distance from the centre, so the halo
    /// has no edge and the disc joins it without a step. Neither is a rectangle,
    /// which is the failure mode this shape exists to avoid.
    #[must_use]
    pub fn light(self, u: f32, v: f32, aspect: f32) -> f32 {
        let dx = (u - self.u) * aspect;
        let dy = v - self.v;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance <= self.radius {
            1.0
        } else if distance >= self.halo {
            0.0
        } else {
            let t = (distance - self.radius) / (self.halo - self.radius);
            (1.0 - t) * (1.0 - t)
        }
    }
}

/// Which part of a field is allowed to change over time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Drift {
    /// The field is drawn once and never changes. Costs nothing to keep.
    Still,
    /// Only the rows between these fractions of the height change. This is what
    /// keeps a drifting sky band from repainting the ground.
    Band {
        /// The top of the drifting region, 0 at the top of the field.
        top: f32,
        /// The bottom of the drifting region, 1 at the bottom of the field.
        bottom: f32,
    },
    /// The whole field drifts. The most expensive option, and the honest one for
    /// a field that genuinely moves everywhere.
    Whole,
}

impl Drift {
    /// Whether anything changes at all.
    #[must_use]
    pub const fn animates(self) -> bool {
        !matches!(self, Drift::Still)
    }

    /// The rows that change, for a field `height` tall.
    #[must_use]
    pub fn rows(self, height: u16) -> Range<u16> {
        match self {
            Drift::Still => 0..0,
            Drift::Whole => 0..height,
            Drift::Band { top, bottom } => {
                let extent = f32::from(height);
                let first = (top.clamp(0.0, 1.0) * extent).floor() as u16;
                let last = (bottom.clamp(0.0, 1.0) * extent).ceil() as u16;
                // A band given backwards is empty rather than inverted.
                if first >= last {
                    0..0
                } else {
                    first..last.min(height)
                }
            }
        }
    }
}

/// A procedural sky: a vertical ramp, a light gathered at the horizon, an
/// optional moon, optional stars, and an optional drifting aurora.
///
/// Every field is authorable, so a place can have its own sky rather than a
/// colour picked out of a match. [`Sky::night`] is the deliberate default.
#[derive(Debug, Clone)]
pub struct Sky {
    /// The vertical ramp, from the top of the field to the bottom.
    ///
    /// One ramp and not two. A separate sky ramp above the horizon and ground
    /// ramp below it meets at a seam, and a seam in a gradient is a visible
    /// horizontal line: the sky, the horizon and the land below it are one
    /// continuous change of colour, and the ring of stops around the horizon is
    /// what shapes it.
    pub ramp: Ramp,
    /// Where the sky's features end, as a fraction of the height. This is what
    /// confines the aurora to the upper sky; it is not a colour boundary.
    pub horizon: f32,
    /// The colour of the light gathered at the horizon.
    pub glow: Rgb,
    /// Where the glow is centred, 0 at the left edge and 1 at the right.
    ///
    /// Without this the glow is a function of the row alone and reads as a
    /// bright horizontal slab laid across the field, however smooth its falloff.
    pub glow_u: f32,
    /// The row the glow is brightest on, as a fraction of the height.
    pub glow_row: f32,
    /// How many rows the glow fades over, as a fraction of the height.
    pub glow_width: f32,
    /// How far the glow spreads sideways, as a fraction of the width.
    pub glow_span: f32,
    /// The strongest the glow ever gets, 0 to 1.
    pub glow_strength: f32,
    /// A moon, if this sky has one.
    pub moon: Option<Moon>,
    /// How many stars fall per cell, 0 for none.
    pub stars: f32,
    /// The tint of the drifting aurora, if this sky has one.
    pub aurora: Option<Rgb>,
    /// Where the field is allowed to change over time.
    pub drift: Drift,
    /// How many times a second the drifting part is resampled.
    pub tick_rate: f32,
}

impl Sky {
    /// A deliberate dark-fantasy night: near-black at the zenith, indigo and
    /// violet through the middle, a rose ring where the horizon is, and the land
    /// below falling away into darkness. A warm light gathers on the horizon, a
    /// cold moon hangs high on the right, and an aurora drifts across the top.
    ///
    /// The numbers are chosen, not guessed: the ramp changes by no more than a
    /// few units of colour per vertical sample and the glow is spread over half
    /// the height, because a steeper gradient than that shows as horizontal
    /// bands at this resolution. `a_night_sky_shows_no_bands` holds it to that.
    #[must_use]
    pub fn night() -> Self {
        Sky {
            ramp: Ramp::new(vec![
                Rgb::new(4, 4, 15),
                Rgb::new(10, 9, 30),
                Rgb::new(19, 15, 47),
                Rgb::new(31, 20, 58),
                Rgb::new(46, 28, 66),
                Rgb::new(63, 36, 70),
                Rgb::new(36, 24, 42),
                Rgb::new(20, 14, 24),
                Rgb::new(9, 8, 14),
            ]),
            horizon: 0.66,
            glow: Rgb::new(255, 152, 94),
            glow_u: 0.42,
            glow_row: 0.62,
            glow_width: 0.46,
            glow_span: 0.55,
            glow_strength: 0.45,
            moon: Some(Moon {
                u: 0.78,
                v: 0.15,
                radius: 0.035,
                halo: 0.20,
            }),
            stars: 1.0,
            aurora: Some(Rgb::new(96, 122, 220)),
            // Only the top quarter of the field drifts, and only four times a
            // second. Both numbers are load-bearing rather than decorative: the
            // rows outside this band are drawn once and never retransmitted, and
            // the tick rate is what keeps an animated sky off the frame budget.
            drift: Drift::Band {
                top: 0.0,
                bottom: 0.26,
            },
            tick_rate: 4.0,
        }
    }

    /// The same sky, held still.
    ///
    /// A screen in reduced-motion mode builds its sky through this, so the
    /// atmosphere stops moving without the field disappearing. [`Drift::Still`]
    /// is the same thing spelled out.
    #[must_use]
    pub fn stilled(mut self) -> Self {
        self.drift = Drift::Still;
        self
    }

    /// Samples the continuous field at `at`.
    ///
    /// This is the pure part: no size in cells, no colour space, no cache. Given
    /// the same probe it always returns the same colour, which is what makes the
    /// field testable and its animation stable.
    #[must_use]
    pub fn sample(&self, at: Probe) -> Rgb {
        let Probe { u, v, aspect, time } = at;
        let v = v.clamp(0.0, 1.0);

        let mut colour = self.ramp.sample(v);

        // The aurora: two slow waves that drift in opposite directions, confined
        // to the sky and darkened towards the horizon so it never meets the glow.
        //
        // The waves are deliberately brisk enough to change the field between two
        // ticks at the default rate: a drift slower than the tick would resolve to
        // the same colours and quietly cost a full repaint for no visible motion.
        if let Some(tint) = self.aurora {
            let ceiling = (1.0 - v / self.horizon.max(f32::EPSILON)).max(0.0);
            let wave = ((u * 3.4 + time * 0.60).sin() * 0.5 + 0.5) * 0.6
                + ((u * 1.7 - time * 0.90).sin() * 0.5 + 0.5) * 0.4;
            let amount = (ceiling * ceiling * wave * 0.36).clamp(0.0, 1.0);
            colour = colour.mix(tint, amount);
        }

        // The light at the horizon: a gaussian in both `u` and `v`. In `v` so it
        // has no edge to see; in `u` so it is a light rather than a bright
        // horizontal slab stretched the full width of the field.
        let down = (v - self.glow_row) / self.glow_width.max(f32::EPSILON);
        let across = (u - self.glow_u) / self.glow_span.max(f32::EPSILON);
        let glow = (-down * down - across * across).exp() * self.glow_strength;
        colour = colour.mix(self.glow, glow.clamp(0.0, 1.0));

        if let Some(moon) = self.moon {
            let light = moon.light(u, v, aspect);
            if light > 0.0 {
                colour = colour.mix(Rgb::new(252, 248, 236), (light * 0.88).clamp(0.0, 1.0));
            }
        }

        colour
    }

    /// How brightly a star burns at cell `(x, y)` at `time`, 0 for no star.
    ///
    /// Stars are points, not a field: which cell holds one is a hash of the
    /// cell's own position, so a star never moves, and only its brightness is a
    /// function of the clock. That is what stops a twinkling sky from crawling.
    #[must_use]
    pub fn star(&self, x: u16, y: u16, time: f32) -> f32 {
        if self.stars <= 0.0 {
            return 0.0;
        }
        let seed = u32::from(x).wrapping_mul(0x9E37_79B9) ^ u32::from(y).wrapping_mul(0x85EB_CA6B);
        let roll = f32::from((seed >> 8) as u16) / f32::from(u16::MAX);
        if roll > 0.013 * self.stars {
            return 0.0;
        }
        let phase = (seed % 1000) as f32 * 0.017;
        let twinkle = ((time * 1.7 + phase).sin() * 0.5 + 0.5).powi(2);
        0.30 + twinkle * 0.70
    }
}

/// A resolved atmosphere field for one area, with its own clock and cache.
///
/// The field is generated in [`resolve`](Atmosphere::resolve) and only blitted in
/// [`paint`](Atmosphere::paint), so a screen can resolve on its own schedule in
/// `update` and paint cheaply in `compose`.
#[derive(Debug)]
pub struct Atmosphere {
    sky: Sky,
    support: ColorSupport,
    elapsed: f32,
    tick: i64,
    cache: Canvas,
    resolved: Rect,
    stale: bool,
}

impl Atmosphere {
    /// Creates an atmosphere for a terminal with the given colour support.
    #[must_use]
    pub fn new(sky: Sky, support: ColorSupport) -> Self {
        Atmosphere {
            sky,
            support,
            elapsed: 0.0,
            tick: 0,
            cache: Canvas::new(0, 0),
            resolved: Rect::new(0, 0, 0, 0),
            stale: true,
        }
    }

    /// The sky this atmosphere draws.
    #[must_use]
    pub fn sky(&self) -> &Sky {
        &self.sky
    }

    /// The colour support this atmosphere was resolved for.
    #[must_use]
    pub const fn support(&self) -> ColorSupport {
        self.support
    }

    /// The resolved field, for tests and diagnostics.
    #[must_use]
    pub fn field(&self) -> &Canvas {
        &self.cache
    }

    /// The area the cached field covers.
    #[must_use]
    pub const fn area(&self) -> Rect {
        self.resolved
    }

    /// The time the cached field was sampled at, on the field's own clock.
    ///
    /// Derived from the tick index rather than from elapsed time, so it is
    /// identical for every resolve inside one tick.
    #[must_use]
    pub fn time(&self) -> f32 {
        self.tick as f32 / self.sky.tick_rate.max(f32::EPSILON)
    }

    /// Advances the clock, returning whether the field now needs resolving.
    ///
    /// Returns `false` immediately for a still sky, and for every call that does
    /// not reach the next tick.
    pub fn advance(&mut self, delta: Duration) -> bool {
        if !self.sky.drift.animates() {
            return false;
        }
        self.elapsed += delta.as_secs_f32();
        let rate = self.sky.tick_rate.max(f32::EPSILON);
        let tick = (self.elapsed * rate) as i64;
        if tick <= self.tick {
            return false;
        }
        self.tick = tick;
        self.stale = true;
        true
    }

    /// How long until the field may change again, or `None` if it never will.
    ///
    /// A screen returns this from `Screen::next_frame`, which is how the
    /// atmosphere gets a cadence of its own instead of dragging the whole game up
    /// to the animation rate.
    #[must_use]
    pub fn next_update(&self) -> Option<Duration> {
        if !self.sky.drift.animates() {
            return None;
        }
        let rate = self.sky.tick_rate.max(f32::EPSILON);
        let next = (self.tick + 1) as f32 / rate;
        Some(Duration::from_secs_f32((next - self.elapsed).max(0.0)))
    }

    /// Whether the cache is out of date for `area`.
    #[must_use]
    pub fn needs_resolve(&self, area: Rect) -> bool {
        self.stale || self.resolved != area
    }

    /// Regenerates the cached field for `area`, if it is out of date.
    ///
    /// A tick repaints only the rows [`Drift`] allows to change; a change of area
    /// repaints everything. Cheap to call every frame: an up-to-date cache costs
    /// one comparison.
    pub fn resolve(&mut self, area: Rect) {
        if !self.needs_resolve(area) {
            return;
        }
        if self.resolved != area {
            self.cache.resize(area.width, area.height);
            self.resolved = area;
            self.paint_rows(area, 0..area.height);
        } else {
            self.paint_rows(area, self.sky.drift.rows(area.height));
        }
        self.stale = false;
    }

    /// Draws the cached field into `frame`, underneath everything else.
    ///
    /// Call this before composing the rest of the screen. Cells not covered by
    /// the resolved area are left alone.
    pub fn paint(&self, frame: &mut Frame) {
        if self.resolved.is_empty() || self.cache.width() == 0 {
            return;
        }
        frame
            .canvas()
            .blit(&self.cache, self.resolved.x, self.resolved.y);
    }

    /// Fills the given rows of the cache from the field.
    fn paint_rows(&mut self, area: Rect, rows: Range<u16>) {
        if area.is_empty() || rows.is_empty() {
            return;
        }
        let width = area.width;
        let height = area.height;
        // The field is sampled `2 * height` times vertically and `width` times
        // horizontally. A cell is about twice as tall as it is wide, so those two
        // counts are in nearly square pixels, and this is the ratio between them.
        let aspect = f32::from(width) / (f32::from(height) * 2.0).max(f32::EPSILON);
        let time = self.time();
        let body = Rgb::new(255, 250, 240);

        for y in rows {
            if y >= height {
                break;
            }
            for x in 0..width {
                let u = (f32::from(x) + 0.5) / f32::from(width);
                let upper = self.sky.sample(Probe {
                    u,
                    v: (f32::from(y) * 2.0 + 0.5) / (f32::from(height) * 2.0),
                    aspect,
                    time,
                });
                let lower = self.sky.sample(Probe {
                    u,
                    v: (f32::from(y) * 2.0 + 1.5) / (f32::from(height) * 2.0),
                    aspect,
                    time,
                });
                // A star is a point, so it lights the upper half of its cell.
                let star = self.sky.star(x, y, time);
                let upper = if star > 0.0 {
                    upper.mix(body, star)
                } else {
                    upper
                };
                self.cache.set(
                    x,
                    y,
                    Cell::new(
                        HALF_BLOCK,
                        Style::new()
                            .with_fg(upper.to_color(self.support))
                            .with_bg(lower.to_color(self.support)),
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Atmosphere, Drift, Moon, Ramp, Rgb, Sky};
    use crate::render::{Canvas, Cell, Color, Frame, Rect, Style};
    use crate::terminal::{ColorSupport, Presenter};

    /// A sky with only a ramp: no moon, no stars, no aurora, no glow. The
    /// simplest field that still has two samples per cell, so a test can assert
    /// the shape of the result rather than one particular colour.
    fn plain(drift: Drift) -> Sky {
        Sky {
            ramp: Ramp::new(vec![Rgb::new(0, 0, 0), Rgb::new(255, 255, 255)]),
            horizon: 1.0,
            glow: Rgb::new(255, 255, 255),
            // A glow with no horizontal falloff, so a test that varies the row
            // and reads one column sees only the vertical shape it is testing.
            glow_u: 0.5,
            glow_row: 0.5,
            glow_width: 0.1,
            glow_span: 9.0,
            glow_strength: 0.0,
            moon: None,
            stars: 0.0,
            aurora: None,
            drift,
            tick_rate: 4.0,
        }
    }

    /// A sky that is a flat colour apart from whatever the test turns on.
    fn flat(drift: Drift) -> Sky {
        let mut sky = plain(drift);
        sky.ramp = Ramp::new(vec![Rgb::new(0, 0, 0), Rgb::new(0, 0, 0)]);
        sky
    }

    fn resolved(sky: Sky, width: u16, height: u16) -> Atmosphere {
        let mut atmosphere = Atmosphere::new(sky, ColorSupport::TrueColor);
        atmosphere.resolve(Rect::from_size(width, height));
        atmosphere
    }

    fn cell(atmosphere: &Atmosphere, x: u16, y: u16) -> Cell {
        atmosphere.field().get(x, y).expect("cell")
    }

    /// Every cell of the resolved field, row by row.
    fn cells(atmosphere: &Atmosphere) -> Vec<Cell> {
        let area = atmosphere.area();
        let mut out = Vec::with_capacity(usize::from(area.width) * usize::from(area.height));
        for y in 0..area.height {
            for x in 0..area.width {
                out.push(cell(atmosphere, x, y));
            }
        }
        out
    }

    fn channel(cell: &Cell, which: fn(&Style) -> Color) -> (u8, u8, u8) {
        match which(&cell.style) {
            Color::Rgb(r, g, b) => (r, g, b),
            other => panic!("expected true colour, got {other:?}"),
        }
    }

    fn fg(cell: &Cell) -> (u8, u8, u8) {
        channel(cell, |style| style.fg)
    }

    fn bg(cell: &Cell) -> (u8, u8, u8) {
        channel(cell, |style| style.bg)
    }

    /// The field painted into a fresh canvas exactly as a screen would.
    fn painted(atmosphere: &Atmosphere) -> Canvas {
        let area = atmosphere.area();
        let mut canvas = Canvas::new(area.width, area.height);
        let mut images = Vec::new();
        let mut frame = Frame::begin(&mut canvas, &mut images);
        atmosphere.paint(&mut frame);
        canvas
    }

    // -- the composition rules ------------------------------------------------

    #[test]
    fn every_cell_is_a_half_block_carrying_two_different_samples() {
        // The claim the whole design rests on: one cell, two vertical samples.
        let atmosphere = resolved(plain(Drift::Still), 4, 8);
        for y in 0..8 {
            for x in 0..4 {
                let cell = cell(&atmosphere, x, y);
                assert_eq!(super::HALF_BLOCK, cell.ch, "cell ({x}, {y})");
                assert_ne!(fg(&cell), bg(&cell), "cell ({x}, {y}) held one sample");
            }
        }
    }

    #[test]
    fn the_samples_of_a_cell_continue_the_ramp_of_the_cell_above() {
        // A vertical ramp with nothing else in it: the lower sample of one cell
        // and the upper sample of the next are two steps of the same ramp, which
        // is what makes the field read as continuous rather than striped.
        let atmosphere = resolved(plain(Drift::Still), 3, 6);
        for y in 1..6 {
            for x in 0..3 {
                let above = f64::from(bg(&cell(&atmosphere, x, y - 1)).0);
                let below = f64::from(fg(&cell(&atmosphere, x, y)).0);
                assert!(
                    (above - below).abs() <= 24.0,
                    "row {y} jumped: {above} then {below}"
                );
            }
        }
    }

    #[test]
    fn a_glyph_drawn_over_the_field_keeps_the_lower_sample_as_its_background() {
        // The honest limit: a glyph takes the cell's glyph slot with it, so only
        // the lower of the two samples survives, as that glyph's background.
        let atmosphere = resolved(plain(Drift::Still), 4, 4);
        let lower = bg(&cell(&atmosphere, 1, 2));

        let mut images = Vec::new();
        let mut canvas = Canvas::new(4, 4);
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            atmosphere.paint(&mut frame);
            let mut view = frame.view(frame.bounds());
            view.put_char(1, 2, '│', Style::new().with_fg(Color::LightCyan));
        }

        let cell = canvas.get(1, 2).expect("cell");
        assert_eq!('│', cell.ch);
        assert_eq!(Color::LightCyan, cell.style.fg);
        assert_eq!(Color::Rgb(lower.0, lower.1, lower.2), cell.style.bg);
    }

    #[test]
    fn a_glyph_with_its_own_background_replaces_both_samples() {
        let atmosphere = resolved(plain(Drift::Still), 4, 4);
        let mut images = Vec::new();
        let mut canvas = Canvas::new(4, 4);
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            atmosphere.paint(&mut frame);
            let mut view = frame.view(frame.bounds());
            view.put_char(
                2,
                2,
                '#',
                Style::new()
                    .with_fg(Color::LightYellow)
                    .with_bg(Color::Indexed(53)),
            );
        }
        let cell = canvas.get(2, 2).expect("cell");
        assert_eq!('#', cell.ch);
        assert_eq!(Color::Indexed(53), cell.style.bg);
        assert_eq!(Color::LightYellow, cell.style.fg);
    }

    #[test]
    fn a_blank_space_over_the_field_keeps_the_lower_sample_and_drops_the_block() {
        // A space is drawable, so a later layer that writes one *does* displace
        // the half block: the cell becomes flat in the lower sample's colour.
        // Recorded here because it is the one way a caller quietly flattens the
        // field, and it is what happens to every cell of text.
        let atmosphere = resolved(plain(Drift::Still), 4, 4);
        let lower = bg(&cell(&atmosphere, 3, 1));

        let mut images = Vec::new();
        let mut canvas = Canvas::new(4, 4);
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            atmosphere.paint(&mut frame);
            let mut view = frame.view(frame.bounds());
            view.put_str(3, 1, " ", Style::new().with_fg(Color::White));
        }

        let cell = canvas.get(3, 1).expect("cell");
        assert_eq!(' ', cell.ch);
        assert_eq!(Color::Rgb(lower.0, lower.1, lower.2), cell.style.bg);
    }

    #[test]
    fn a_sprite_keeps_its_ink_and_takes_the_field_as_its_background() {
        let atmosphere = resolved(plain(Drift::Still), 14, 9);
        let mut images = Vec::new();
        let mut canvas = Canvas::new(14, 9);
        let art = crate::ui::sprite::for_figure(crate::game::Figure::Mage);
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            atmosphere.paint(&mut frame);
            let mut view = frame.view(frame.bounds());
            crate::ui::sprite::draw(
                &mut view,
                6,
                3,
                art,
                crate::ui::sprite::Pose::standing(),
                art.inks,
                false,
            );
        }

        let mut found = 0;
        for y in 3..3 + u32::from(art.height) {
            for x in 0..14u16 {
                let cell = canvas.get(x, y as u16).expect("cell");
                if cell.ch != ' ' && cell.ch != super::HALF_BLOCK {
                    found += 1;
                    // The glyph has its own ink, and the field is still beneath it.
                    assert_ne!(Color::Default, cell.style.fg);
                    assert_ne!(Color::Default, cell.style.bg, "the field was lost");
                    assert_ne!(cell.style.fg, cell.style.bg, "the ink was overwritten");
                }
            }
        }
        assert!(found > 0, "the sprite drew nothing");
    }

    // -- the field ------------------------------------------------------------

    #[test]
    fn a_ramp_interpolates_between_its_stops_and_clamps_outside_them() {
        let ramp = Ramp::new(vec![
            Rgb::new(0, 0, 0),
            Rgb::new(100, 0, 0),
            Rgb::new(200, 0, 0),
        ]);
        assert_eq!(Rgb::new(0, 0, 0), ramp.sample(0.0));
        assert_eq!(Rgb::new(200, 0, 0), ramp.sample(1.0));
        assert_eq!(Rgb::new(100, 0, 0), ramp.sample(0.5));
        assert_eq!(Rgb::new(50, 0, 0), ramp.sample(0.25));
        assert_eq!(Rgb::new(0, 0, 0), ramp.sample(-1.0));
        assert_eq!(Rgb::new(200, 0, 0), ramp.sample(4.0));
    }

    #[test]
    fn a_ramp_of_one_stop_is_a_loud_mistake() {
        let stopped = std::panic::catch_unwind(|| Ramp::new(vec![Rgb::new(1, 2, 3)]));
        assert!(stopped.is_err(), "a one-stop ramp should panic");
    }

    #[test]
    fn the_field_is_deterministic_for_a_given_tick() {
        let first = resolved(Sky::night(), 40, 12);
        let second = resolved(Sky::night(), 40, 12);
        assert_eq!(cells(&first), cells(&second));
    }

    #[test]
    fn reaching_the_same_tick_by_a_different_route_gives_the_same_field() {
        // The clock is quantised, so the path to a tick cannot matter.
        let mut stepped = Atmosphere::new(Sky::night(), ColorSupport::TrueColor);
        let mut jumped = Atmosphere::new(Sky::night(), ColorSupport::TrueColor);
        let area = Rect::from_size(30, 10);
        stepped.resolve(area);
        jumped.resolve(area);
        for _ in 0..4 {
            stepped.advance(Duration::from_millis(125));
        }
        stepped.resolve(area);
        jumped.advance(Duration::from_millis(500));
        jumped.resolve(area);
        assert_eq!(cells(&stepped), cells(&jumped));
    }

    #[test]
    fn a_gradient_runs_smoothly_without_a_step() {
        // A slab is a large brightness jump between neighbouring rows. A ramp
        // must never produce one.
        let atmosphere = resolved(plain(Drift::Still), 20, 24);
        let mut previous: Option<f32> = None;
        for y in 0..24 {
            let value = f32::from(fg(&cell(&atmosphere, 10, y)).0);
            if let Some(previous) = previous {
                let step = (value - previous).abs();
                assert!(step < 40.0, "row {y} stepped by {step}");
            }
            previous = Some(value);
        }
    }

    #[test]
    fn a_glow_produces_a_falloff_and_never_a_bright_slab() {
        // A bright horizontal band with hard edges is the failure this shape
        // exists to avoid, so the glow is checked for smoothness as well as
        // direction.
        let mut sky = flat(Drift::Still);
        sky.glow = Rgb::new(255, 255, 255);
        sky.glow_u = 0.5;
        sky.glow_row = 0.5;
        sky.glow_width = 0.12;
        sky.glow_span = 9.0;
        sky.glow_strength = 0.9;
        let atmosphere = resolved(sky, 8, 48);

        // The height is 48, so the glow's row (0.5) is row 24.
        let at = |y: u16| f32::from(fg(&cell(&atmosphere, 4, y)).0);
        assert!(at(24) > at(12), "the glow did not fade upwards");
        assert!(at(24) > at(36), "the glow did not fade downwards");
        assert!(at(12) > at(4), "the glow did not fade far upwards");
        assert!(at(36) > at(44), "the glow did not fade far downwards");

        for y in 1..48 {
            let step = (at(y) - at(y - 1)).abs();
            assert!(step < 60.0, "the glow jumped {step} between rows {}", y - 1);
        }
    }

    #[test]
    fn a_night_sky_shows_no_bands() {
        // The failure this test exists to catch was found by measuring a capture:
        // a glow steep enough to change by twenty units of colour per half-row
        // reads as a stack of horizontal lines. The whole field is required to
        // move by no more than a few units per sample, at the sizes that are
        // actually used and at a smaller one, where there are fewer samples to
        // spread the same change over.
        let mut sky = Sky::night();
        sky.moon = None; // a disc is a shape, and a shape is allowed hard edges
        sky.stars = 0.0;
        for (width, height) in [(120, 29), (80, 22), (200, 50)] {
            let atmosphere = resolved(sky.clone(), width, height);
            let samples = height * 2;
            for x in 0..width {
                let mut previous: Option<(u8, u8, u8)> = None;
                for row in 0..samples {
                    let y = row / 2;
                    let cell = cell(&atmosphere, x, y);
                    let colour = if row % 2 == 0 { fg(&cell) } else { bg(&cell) };
                    if let Some(previous) = previous {
                        let step = [
                            previous.0.abs_diff(colour.0),
                            previous.1.abs_diff(colour.1),
                            previous.2.abs_diff(colour.2),
                        ]
                        .into_iter()
                        .max()
                        .unwrap_or(0);
                        assert!(
                            step <= 7,
                            "{width}x{height}: column {x} stepped {step} units at sample {row}"
                        );
                    }
                    previous = Some(colour);
                }
            }
        }
    }

    #[test]
    fn the_moon_is_a_disc_and_not_a_rectangle() {
        let mut sky = flat(Drift::Still);
        sky.moon = Some(Moon {
            u: 0.5,
            v: 0.5,
            radius: 0.08,
            halo: 0.11,
        });
        let atmosphere = resolved(sky, 60, 30);

        // The moon's body is blended at 88%, so a lit cell is about (221, 218,
        // 207) over a black field. The threshold is well clear of the halo.
        let lit = |y: u16| -> usize {
            (0..60)
                .filter(|x| {
                    let (r, g, b) = fg(&cell(&atmosphere, *x, y));
                    r > 200 && g > 195 && b > 180
                })
                .count()
        };
        let centre = lit(15);
        assert!(
            centre > 2,
            "the moon was not drawn: {centre} cells at the centre"
        );
        let above = lit(12);
        let below = lit(18);
        assert!(
            above < centre && below < centre,
            "the moon has straight sides: {above}, {centre}, {below}"
        );
    }

    #[test]
    fn the_stars_stay_where_they_are_and_only_twinkle() {
        // A star whose position moved would make the sky crawl. Which cell holds
        // one is a hash of the cell, so it cannot.
        let sky = Sky::night();
        let first: Vec<bool> = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .map(|(x, y)| sky.star(x, y, 0.0) > 0.0)
            .collect();
        let later: Vec<bool> = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .map(|(x, y)| sky.star(x, y, 9.0) > 0.0)
            .collect();
        assert_eq!(first, later, "a star moved between frames");
        assert!(first.iter().any(|lit| *lit), "there were no stars at all");
    }

    #[test]
    fn narrow_fields_resolve_without_panicking() {
        for (width, height) in [(1, 1), (1, 8), (8, 1), (2, 2), (0, 0), (3, 0)] {
            let mut atmosphere = Atmosphere::new(Sky::night(), ColorSupport::TrueColor);
            let area = Rect::from_size(width, height);
            atmosphere.resolve(area);
            let mut canvas = Canvas::new(width, height);
            let mut images = Vec::new();
            let mut frame = Frame::begin(&mut canvas, &mut images);
            atmosphere.paint(&mut frame);
        }
    }

    #[test]
    fn a_field_filling_the_whole_frame_leaves_no_cell_transparent() {
        // A cell the field covers must own its colour on every support level, or
        // the terminal's own background would show through the game.
        for support in [
            ColorSupport::Ansi16,
            ColorSupport::Ansi256,
            ColorSupport::TrueColor,
        ] {
            let mut atmosphere = Atmosphere::new(Sky::night(), support);
            let area = Rect::from_size(40, 16);
            atmosphere.resolve(area);
            for y in 0..area.height {
                for x in 0..area.width {
                    let cell = cell(&atmosphere, x, y);
                    assert_ne!(
                        Color::Default,
                        cell.style.fg,
                        "{support:?} left ({x}, {y})'s ink to the terminal"
                    );
                    assert_ne!(
                        Color::Default,
                        cell.style.bg,
                        "{support:?} left ({x}, {y})'s background to the terminal"
                    );
                }
            }
        }
    }

    // -- cadence --------------------------------------------------------------

    #[test]
    fn a_sky_can_be_stilled_so_reduced_motion_costs_nothing() {
        let stilled = Sky::night().stilled();
        assert_eq!(Drift::Still, stilled.drift);
        // And the field is otherwise untouched: the same picture, not a blank one.
        let moved = resolved(Sky::night(), 24, 10);
        let frozen = resolved(stilled, 24, 10);
        assert_eq!(cells(&moved), cells(&frozen));
    }

    #[test]
    fn a_still_field_never_goes_stale() {
        let mut atmosphere = Atmosphere::new(plain(Drift::Still), ColorSupport::TrueColor);
        atmosphere.resolve(Rect::from_size(20, 10));
        assert!(!atmosphere.advance(Duration::from_secs(10)));
        assert!(!atmosphere.needs_resolve(Rect::from_size(20, 10)));
        assert_eq!(None, atmosphere.next_update());
    }

    #[test]
    fn the_clock_is_quantised_so_nothing_changes_inside_a_tick() {
        let mut atmosphere = Atmosphere::new(plain(Drift::Whole), ColorSupport::TrueColor);
        atmosphere.resolve(Rect::from_size(20, 10));
        // The rate is 4, so a tick is 250 ms; four 50 ms steps stay inside one.
        for _ in 0..4 {
            assert!(!atmosphere.advance(Duration::from_millis(50)));
        }
        assert!(atmosphere.advance(Duration::from_millis(50)));
    }

    #[test]
    fn only_the_drifting_band_is_repainted_on_a_tick() {
        let mut sky = flat(Drift::Band {
            top: 0.0,
            bottom: 0.25,
        });
        sky.aurora = Some(Rgb::new(255, 255, 255));
        let mut atmosphere = Atmosphere::new(sky, ColorSupport::TrueColor);
        let area = Rect::from_size(24, 24);
        atmosphere.resolve(area);
        let before = cells(&atmosphere);

        assert!(atmosphere.advance(Duration::from_millis(500)));
        atmosphere.resolve(area);
        let after = cells(&atmosphere);

        let at = |list: &[Cell], x: usize, y: usize| list[y * 24 + x];
        assert!(
            (0..6).any(|y| at(&before, 6, y) != at(&after, 6, y)),
            "the band did not move"
        );
        for y in 8..24 {
            assert_eq!(
                at(&before, 6, y),
                at(&after, 6, y),
                "row {y} changed outside the band"
            );
        }
    }

    #[test]
    fn the_whole_field_drifts_when_asked() {
        let mut sky = flat(Drift::Whole);
        sky.aurora = Some(Rgb::new(255, 255, 255));
        let mut atmosphere = Atmosphere::new(sky, ColorSupport::TrueColor);
        let area = Rect::from_size(24, 24);
        atmosphere.resolve(area);
        let before = cells(&atmosphere);
        assert!(atmosphere.advance(Duration::from_millis(500)));
        atmosphere.resolve(area);
        assert_ne!(before, cells(&atmosphere), "nothing moved anywhere");
    }

    #[test]
    fn the_next_update_follows_the_tick_rate() {
        let mut sky = plain(Drift::Whole);
        sky.tick_rate = 4.0;
        let mut atmosphere = Atmosphere::new(sky, ColorSupport::TrueColor);
        atmosphere.resolve(Rect::from_size(8, 4));
        let wait = atmosphere.next_update().expect("a drifting field updates");
        assert!(
            (wait.as_secs_f32() - 0.25).abs() < 0.001,
            "expected a quarter second, got {wait:?}"
        );
        atmosphere.advance(Duration::from_millis(100));
        let wait = atmosphere.next_update().expect("a drifting field updates");
        assert!(
            (wait.as_secs_f32() - 0.15).abs() < 0.001,
            "expected 150 ms, got {wait:?}"
        );
    }

    #[test]
    fn a_band_given_backwards_is_empty_rather_than_inverted() {
        assert_eq!(
            0..0,
            Drift::Band {
                top: 0.7,
                bottom: 0.2
            }
            .rows(40)
        );
        assert_eq!(
            10..20,
            Drift::Band {
                top: 0.25,
                bottom: 0.5
            }
            .rows(40)
        );
        assert_eq!(0..40, Drift::Whole.rows(40));
        assert_eq!(0..0, Drift::Still.rows(40));
        assert_eq!(0..0, Drift::Still.rows(0));
    }

    // -- support, resize, and the presenter ------------------------------------

    #[test]
    fn the_field_degrades_to_the_palette_the_terminal_has() {
        let area = Rect::from_size(16, 8);
        assert!(matches!(
            cell(&resolved(Sky::night(), 16, 8), 2, 2).style.fg,
            Color::Rgb(..)
        ));

        let mut indexed = Atmosphere::new(Sky::night(), ColorSupport::Ansi256);
        indexed.resolve(area);
        for y in 0..8 {
            for x in 0..16 {
                let cell = cell(&indexed, x, y);
                assert!(
                    matches!(cell.style.fg, Color::Indexed(_))
                        && matches!(cell.style.bg, Color::Indexed(_)),
                    "cell ({x}, {y}) was not reduced to the cube: {:?}",
                    cell.style
                );
            }
        }

        let mut basic = Atmosphere::new(Sky::night(), ColorSupport::Ansi16);
        basic.resolve(area);
        for y in 0..8 {
            for x in 0..16 {
                let cell = cell(&basic, x, y);
                assert!(
                    !matches!(cell.style.fg, Color::Rgb(..)),
                    "cell ({x}, {y}) kept true colour on a 16-colour terminal"
                );
            }
        }
    }

    #[test]
    fn the_colour_cube_reduction_lands_on_exact_cube_values() {
        assert_eq!(16, super::nearest_256(Rgb::new(0, 0, 0)));
        assert_eq!(231, super::nearest_256(Rgb::new(255, 255, 255)));
        assert_eq!(196, super::nearest_256(Rgb::new(255, 0, 0)));
        assert_eq!(46, super::nearest_256(Rgb::new(0, 255, 0)));
        assert_eq!(21, super::nearest_256(Rgb::new(0, 0, 255)));
    }

    #[test]
    fn resizing_regenerates_the_whole_field() {
        let mut atmosphere = Atmosphere::new(Sky::night(), ColorSupport::TrueColor);
        atmosphere.resolve(Rect::from_size(10, 5));
        assert_eq!(10, atmosphere.field().width());
        assert!(!atmosphere.needs_resolve(Rect::from_size(10, 5)));
        assert!(atmosphere.needs_resolve(Rect::from_size(20, 8)));

        atmosphere.resolve(Rect::from_size(20, 8));
        assert_eq!(20, atmosphere.field().width());
        assert_eq!(8, atmosphere.field().height());
        assert!(!atmosphere.needs_resolve(Rect::from_size(20, 8)));
    }

    #[test]
    fn resolving_twice_without_a_tick_changes_nothing() {
        let mut atmosphere = Atmosphere::new(Sky::night(), ColorSupport::TrueColor);
        let area = Rect::from_size(30, 12);
        atmosphere.resolve(area);
        let before = cells(&atmosphere);
        atmosphere.resolve(area);
        assert_eq!(before, cells(&atmosphere));
    }

    #[test]
    fn the_presenter_writes_nothing_at_all_for_a_settled_field() {
        // The acceptance criterion as a test: a still frame costs nothing to keep
        // on screen, because the cached field is byte-identical between frames.
        let atmosphere = resolved(Sky::night(), 40, 12);
        let mut presenter = Presenter::new();
        let mut out = Vec::new();
        presenter
            .present(&painted(&atmosphere), &mut out)
            .expect("present");
        assert!(presenter.last_stats().full_redraw);

        let mut out = Vec::new();
        presenter
            .present(&painted(&atmosphere), &mut out)
            .expect("present");
        let stats = presenter.last_stats();
        assert_eq!(0, stats.cells_written, "a settled field was retransmitted");
        assert!(!stats.full_redraw);
        assert!(stats.bytes < 64, "unexpected output: {} bytes", stats.bytes);
    }

    #[test]
    fn a_banded_tick_rewrites_only_the_rows_that_moved() {
        let mut sky = Sky::night();
        sky.tick_rate = 8.0;
        let mut atmosphere = Atmosphere::new(sky, ColorSupport::TrueColor);
        let area = Rect::from_size(60, 24);
        atmosphere.resolve(area);

        let mut presenter = Presenter::new();
        let mut out = Vec::new();
        presenter
            .present(&painted(&atmosphere), &mut out)
            .expect("present");

        // Two ticks at once, so the band has plainly moved.
        assert!(atmosphere.advance(Duration::from_millis(250)));
        atmosphere.resolve(area);
        let mut out = Vec::new();
        presenter
            .present(&painted(&atmosphere), &mut out)
            .expect("present");

        let stats = presenter.last_stats();
        let band_rows = Sky::night().drift.rows(24);
        let ceiling = u32::from(band_rows.end - band_rows.start) * 60;
        assert!(!stats.full_redraw);
        assert!(
            stats.cells_written > 0,
            "the band did not move at all, so the test proves nothing"
        );
        assert!(
            stats.cells_written <= ceiling,
            "a banded tick wrote {} cells for a band of {ceiling}",
            stats.cells_written
        );
    }
}
