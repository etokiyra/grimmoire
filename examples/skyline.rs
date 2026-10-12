//! A feasibility prototype for a layered, true-colour renderer.
//!
//! This is **experimental**. It does not touch the game: it is a separate binary
//! built out of the same `Screen` trait, the same `Terminal`, the same
//! `Presenter` and the same `Canvas` the game already uses, so what it proves is
//! not that a technique works in isolation but that it drops into the existing
//! architecture without changing it.
//!
//! ```sh
//! cargo run --release --example skyline
//! ```
//!
//! # The architecture it is testing
//!
//! The current renderer puts one glyph, one foreground colour and one flat
//! background in every cell. That is why a sky is impossible in it: there is
//! nowhere to put a gradient.
//!
//! This composes **one extra layer underneath everything** — a procedural
//! true-colour field drawn as the upper-half-block `▀`, whose foreground is the
//! colour of the top half of the cell and whose background is the colour of the
//! bottom half. That doubles the vertical resolution of the *background* only.
//! Every layer above it then draws glyphs that inherit that background, so:
//!
//! ```text
//!   layer 3   figures    glyphs, their own inks       (inherits)
//!   layer 2   effects    motes and sparks, true colour (inherits)
//!   layer 1   scenery    architecture, glyphs          (inherits)
//!   layer 0   atmosphere true-colour field, half blocks
//! ```
//!
//! The rule is the one the canvas already has: a draw that leaves its background
//! unset keeps whatever was under it. So the atmosphere fills the background, and
//! nothing above it has to know it exists.

use std::time::Duration;

use grimmoire::app::{App, Startup};
use grimmoire::game::Figure;
use grimmoire::input::Key;
use grimmoire::render::{Color, Frame, Rect, Style};
use grimmoire::terminal::{Capabilities, ColorSupport, Terminal};
use grimmoire::ui::screen::{Command, Screen};
use grimmoire::ui::sprite;
use grimmoire::ui::{FrameContext, Palette};

/// A colour in eight bits per channel.
type Rgb = (u8, u8, u8);

/// Linear interpolation between two colours, `t` from 0 to 1.
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
    (lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2))
}

/// Brightens a colour, as light falling on something does.
fn lit(colour: Rgb, amount: f32) -> Rgb {
    mix(colour, (255, 245, 224), amount.clamp(0.0, 1.0))
}

/// A cheap value noise, for a sky that is not a flat ramp.
fn noise(x: f32, y: f32, seed: f32) -> f32 {
    let n = (x * 12.9898 + y * 78.233 + seed * 37.719).sin() * 43_758.547;
    (n - n.floor()) * 2.0 - 1.0
}

/// The sky, sampled at a fractional position.
///
/// `u` runs 0 to 1 across the screen, `v` 0 at the top to 1 at the horizon. The
/// whole point of the exercise: a field that varies smoothly across every cell,
/// which the tile-based renderer has no way to express.
fn sky(u: f32, v: f32, time: f32) -> Rgb {
    // Night at the top, ember at the horizon.
    let deep = (8, 6, 24);
    let mid = (34, 20, 66);
    let warm = (92, 42, 78);
    let ember = (196, 100, 70);
    let ground = (12, 10, 20);
    // The light gathers *at* the horizon and the ground below it falls away, so
    // the glow reads as a skyline rather than as a bright floor.
    let mut colour = if v < 0.34 {
        mix(deep, mid, v / 0.34)
    } else if v < 0.58 {
        mix(mid, warm, (v - 0.34) / 0.24)
    } else if v < 0.72 {
        mix(warm, ember, ((v - 0.58) / 0.14).powf(0.8))
    } else {
        mix(ember, ground, ((v - 0.72) / 0.28).powf(0.55))
    };
    // An aurora, drifting across the upper third.
    let band = ((u * 3.1 + time * 0.06).sin() * 0.5 + 0.5) * (1.0 - v * 2.4).max(0.0);
    colour = lit(colour, band * 0.28);
    // And a slow veil of noise, so the ramp is not a machine gradient.
    let veil = noise(u * 6.0, v * 9.0 - time * 0.05, 3.7) * 0.06;
    (
        (f32::from(colour.0) * (1.0 + veil)).clamp(0.0, 255.0) as u8,
        (f32::from(colour.1) * (1.0 + veil)).clamp(0.0, 255.0) as u8,
        (f32::from(colour.2) * (1.0 + veil)).clamp(0.0, 255.0) as u8,
    )
}

/// The moon: a disc with a soft halo, in screen coordinates.
fn moon_light(u: f32, v: f32, moon: (f32, f32), aspect: f32, time: f32) -> f32 {
    let dx = (u - moon.0) * aspect;
    let dy = v - moon.1;
    let distance = (dx * dx + dy * dy).sqrt();
    let pulse = 1.0 + (time * 0.5).sin() * 0.04;
    if distance < 0.045 * pulse {
        1.0
    } else {
        (0.16 - distance).max(0.0) / 0.16 * 0.35
    }
}

/// One drifting mote.
#[derive(Debug, Clone, Copy)]
struct Mote {
    x: f32,
    rise: f32,
    drift: f32,
    phase: f32,
}

impl Mote {
    fn new(seed: u32) -> Self {
        let rand = |n: u32| {
            let v = seed
                .wrapping_mul(2_654_435_761)
                .wrapping_add(n.wrapping_mul(40_503));
            (v >> 8) as f32 / 16_777_216.0
        };
        Mote {
            x: rand(1),
            rise: 0.008 + rand(3) * 0.022,
            drift: (rand(4) - 0.5) * 0.02,
            phase: rand(5) * std::f32::consts::TAU,
        }
    }
}

/// The prototype scene.
struct Skyline {
    /// Seconds since the screen opened.
    time: f32,
    motes: Vec<Mote>,
    selected: usize,
}

impl Skyline {
    fn new() -> Self {
        Skyline {
            time: 0.0,
            motes: (0..90).map(Mote::new).collect(),
            selected: 0,
        }
    }

    /// The atmosphere: one `▀` per cell, two true colours.
    ///
    /// This is the whole technique. The upper half of the cell takes the sky
    /// sampled at the top pixel and the background the sky sampled at the bottom
    /// pixel, so a 110x28 terminal draws a 110x56 field.
    fn compose_atmosphere(&self, frame: &mut Frame, palette: &Palette) {
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        let (width, height) = (bounds.width.max(1), bounds.height.max(1));
        let aspect = f32::from(width) / (f32::from(height) * 2.0);
        let moon = (0.78, 0.16);
        let horizon = 1.0;
        let mut view = frame.view(bounds);

        for y in 0..height {
            for x in 0..width {
                let u = f32::from(x) / f32::from(width);
                let top = (f32::from(y) * 2.0) / (f32::from(height) * 2.0);
                let bottom = (f32::from(y) * 2.0 + 1.0) / (f32::from(height) * 2.0);
                let sample = |v: f32| {
                    let v = (v / horizon).min(1.0);
                    let mut colour = sky(u, v, self.time);
                    let glow = moon_light(u, v * horizon, moon, aspect, self.time);
                    if glow > 1.0 {
                        return (250, 248, 236);
                    }
                    colour = lit(colour, glow);
                    // Stars, thinning towards the horizon.
                    let sparkle = noise(u * 180.0, v * 180.0, 11.0);
                    if sparkle > 0.97 && v < 0.7 {
                        let twinkle = 0.5 + 0.5 * (self.time * 2.0 + sparkle * 40.0).sin();
                        colour = lit(colour, 0.35 + twinkle * 0.5);
                    }
                    colour
                };
                let upper = sample(top);
                let lower = sample(bottom);
                // The one draw: `▀` with the top half in the foreground and the
                // bottom half in the background.
                view.put_char(
                    x,
                    y,
                    '▀',
                    Style::new()
                        .with_fg(Color::Rgb(upper.0, upper.1, upper.2))
                        .with_bg(Color::Rgb(lower.0, lower.1, lower.2)),
                );
            }
        }
        let _ = palette;
    }

    /// Distant hills, then the cathedral: glyphs, drawn over the field.
    fn compose_scenery(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        if bounds.height < 10 || bounds.width < 40 {
            return;
        }
        let mut view = frame.view(bounds);
        let ground = bounds.bottom().saturating_sub(4);
        // Hills behind the building.
        for x in 0..bounds.width {
            let f = x as f32 / f32::from(bounds.width);
            let ridge = (f * 6.0).sin() * 1.4 + (f * 17.0).sin() * 0.6;
            let top = (ground as f32 - 3.0 - ridge) as u16;
            for y in top..ground {
                let shade = if y == top { (26, 22, 48) } else { (14, 12, 30) };
                view.put_char(
                    x,
                    y,
                    '▒',
                    Style::new().with_fg(Color::Rgb(shade.0, shade.1, shade.2)),
                );
            }
        }

        // The cathedral: two towers, a nave, a rose window and a spire. Written
        // as a glyph silhouette rather than generated, so it is a composition
        // rather than a texture.
        let stone = (78, 70, 104);
        let stone_lit = (132, 120, 152);
        let glass = (244, 190, 96);
        let left = bounds.centered_x(46).saturating_sub(6);
        let base = ground;
        let put = |view: &mut grimmoire::render::CanvasView, x: i32, y: i32, ch: char, c: Rgb| {
            if x < 0 || y < 0 {
                return;
            }
            let (x, y) = (x as u16, y as u16);
            if x >= bounds.width || y >= bounds.height {
                return;
            }
            view.put_char(x, y, ch, Style::new().with_fg(Color::Rgb(c.0, c.1, c.2)));
        };
        // Nave.
        let base = i32::from(base);
        for y in 0..14i32 {
            for x in 8..38i32 {
                let row = base - 1 - y;
                let inside = x > 9 && x < 36;
                let ch = if y >= 13 {
                    '▀'
                } else if inside {
                    ' '
                } else if x == 8 || x == 37 {
                    '│'
                } else {
                    '▓'
                };
                if ch == ' ' {
                    continue;
                }
                put(&mut view, left as i32 + x, row, ch, stone);
            }
        }
        // Roof: a gable.
        for step in 0..7 {
            let y = base - 15 - step;
            for x in (8 + step)..(38 - step) {
                put(&mut view, left as i32 + x, y, '▄', stone_lit);
            }
        }
        // Towers, flanking, with spires.
        for (x0, height) in [(2i32, 20i32), (41, 17)] {
            for y in 0..height {
                let row = base - 1 - y;
                for dx in 0..5 {
                    let ch = if dx == 0 || dx == 4 { '│' } else { '▓' };
                    put(&mut view, left as i32 + x0 + dx, row, ch, stone);
                }
            }
            for step in 0..6 {
                let y = base - 1 - height - step;
                put(&mut view, left as i32 + x0 + 2, y, '▲', stone_lit);
                if step < 4 {
                    put(&mut view, left as i32 + x0 + 1, y, '·', stone_lit);
                    put(&mut view, left as i32 + x0 + 3, y, '·', stone_lit);
                }
            }
        }
        // A rose window, and lit lancets down the nave.
        let rose_y = base - 10;
        for (dx, dy, ch) in [
            (-2i32, 0i32, '◆'),
            (-1, -1, '·'),
            (0, -1, '◆'),
            (1, -1, '·'),
            (-1, 1, '·'),
            (1, 1, '·'),
            (0, 1, '◆'),
            (-2, 1, '·'),
            (2, 1, '·'),
            (-1, 0, '◆'),
            (1, 0, '◆'),
        ] {
            put(&mut view, left as i32 + 22 + dx, rose_y + dy, ch, glass);
        }
        for x in [13i32, 17, 27, 31] {
            for y in 0..5 {
                put(&mut view, left as i32 + x, base - 4 - y, '▌', glass);
            }
        }
        // A lit doorway.
        for dx in 0..4 {
            put(&mut view, left as i32 + 21 + dx, base - 2, '▄', glass);
        }
        // Columns along the front, so the building has depth rather than a face.
        for x in [10i32, 36] {
            for y in 0..13 {
                put(&mut view, left as i32 + x, base - 1 - y, '║', stone_lit);
            }
        }
    }

    /// Motes rising off the ground, drawn in true colour over everything but the
    /// figure, so the air reads as air.
    fn compose_motes(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        let mut view = frame.view(bounds);
        for mote in &self.motes {
            let life = (self.time * mote.rise + mote.phase).fract();
            let v = 0.95 - life * 0.85;
            let u =
                (mote.x + (self.time * mote.drift).fract() + noise(mote.phase, life, 5.0) * 0.01)
                    .rem_euclid(1.0);
            let fade = (life * std::f32::consts::PI).sin().max(0.0);
            let x = (u * f32::from(bounds.width)) as u16;
            let y = (v * f32::from(bounds.height)) as u16;
            if x >= bounds.width || y >= bounds.height {
                continue;
            }
            let warm = lit((255, 214, 150), fade * 0.7);
            let glyph = if fade > 0.75 {
                '✧'
            } else if fade > 0.4 {
                '·'
            } else {
                '.'
            };
            view.put_char(
                x,
                y,
                glyph,
                Style::new().with_fg(Color::Rgb(warm.0, warm.1, warm.2)),
            );
        }
    }

    /// The wizard, standing on the ground before the cathedral.
    fn compose_figure(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        let art = sprite::for_figure(Figure::Mage);
        let x = bounds.centered_x(art.width);
        let y = bounds.bottom().saturating_sub(7);
        if y + art.height > bounds.height {
            return;
        }
        let mut view = frame.view(bounds);
        // A pool of light at the feet, so the figure is standing somewhere.
        for dx in 0..7u16 {
            for dy in 0..2u16 {
                let px = x.saturating_sub(2).saturating_add(dx);
                let py = y.saturating_add(art.height).saturating_add(dy);
                if px < bounds.width && py < bounds.height {
                    let glow = lit((255, 236, 190), 0.45 - dy as f32 * 0.2);
                    view.put_char(
                        px,
                        py,
                        '▁',
                        Style::new().with_fg(Color::Rgb(glow.0, glow.1, glow.2)),
                    );
                }
            }
        }
        let pose = sprite::Pose::Idle {
            breathing: (self.time * 1.4).sin() > 0.55,
        };
        sprite::draw(&mut view, x, y, art, pose, art.inks, false);
    }

    /// The wordmark and the menu.
    fn compose_ui(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        if bounds.width < 40 || bounds.height < 12 {
            return;
        }
        let mut view = frame.view(bounds);
        let word = "G R I M M O I R E";
        let top = 3u16;
        for (index, ch) in word.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let t = index as f32 / word.chars().count() as f32;
            let colour = mix((255, 226, 150), (240, 120, 96), t);
            view.put_char(
                bounds
                    .centered_x(word.chars().count() as u16)
                    .saturating_add(index as u16),
                top,
                ch,
                Style::new()
                    .with_fg(Color::Rgb(colour.0, colour.1, colour.2))
                    .bold(),
            );
        }
        let tagline = "a dark-fantasy tale, drawn entirely in glyphs";
        view.put_str(
            bounds.centered_x(grimmoire::render::glyph::display_width(tagline)),
            top + 2,
            tagline,
            Style::new().with_fg(Color::Rgb(200, 190, 220)),
        );

        let items = ["New Game", "Continue", "Settings", "Quit"];
        let list_top = bounds.bottom().saturating_sub(12);
        // A scrim: a menu over a skyline is unreadable without one, and a game's
        // interface has to win against its own scenery.
        let list_x = 5u16.min(bounds.width.saturating_sub(20));
        {
            let rows = items.len() as u16 * 2;
            let scrim = Rect::new(
                list_x.saturating_sub(3),
                list_top.saturating_sub(1),
                30.min(bounds.width),
                rows.saturating_add(1),
            );
            view.fill_style(scrim, Style::new().with_bg(Color::Rgb(14, 12, 26)));
        }
        for (index, item) in items.iter().enumerate() {
            let y = list_top.saturating_add(index as u16 * 2);
            let chosen = index == self.selected;
            let base = if chosen {
                (255, 232, 168)
            } else {
                (176, 168, 200)
            };
            let x = list_x;
            if chosen {
                view.put_str(
                    x.saturating_sub(2),
                    y,
                    "▸",
                    Style::new().with_fg(Color::Rgb(255, 216, 120)).bold(),
                );
            }
            view.put_str(
                x,
                y,
                item,
                Style::new().with_fg(Color::Rgb(base.0, base.1, base.2)),
            );
        }
        let hint = "w/s choose · enter select · esc quit · this is a prototype";
        view.put_str(
            bounds.centered_x(grimmoire::render::glyph::display_width(hint)),
            bounds.bottom().saturating_sub(1),
            hint,
            Style::new().with_fg(Color::Rgb(120, 116, 140)),
        );
    }
}

impl Screen for Skyline {
    fn name(&self) -> &'static str {
        "skyline"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        if !key.is_down() || key.is_repeat() {
            return Command::None;
        }
        match key.code {
            grimmoire::input::KeyCode::Esc | grimmoire::input::KeyCode::Char('q') => Command::Quit,
            grimmoire::input::KeyCode::Down | grimmoire::input::KeyCode::Char('s') => {
                self.selected = (self.selected + 1) % 4;
                Command::None
            }
            grimmoire::input::KeyCode::Up | grimmoire::input::KeyCode::Char('w') => {
                self.selected = (self.selected + 3) % 4;
                Command::None
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.time += delta.as_secs_f32();
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        self.compose_atmosphere(frame, &context.palette);
        self.compose_scenery(frame);
        self.compose_motes(frame);
        self.compose_figure(frame);
        self.compose_ui(frame);
    }

    fn is_animating(&self) -> bool {
        true
    }

    fn next_frame(&self) -> Option<Duration> {
        Some(Duration::ZERO)
    }
}

fn main() -> std::io::Result<()> {
    grimmoire::terminal::install_panic_hook();
    let mut terminal = Terminal::new()?;
    // True colour, explicitly: this prototype is about what a layer of 24-bit
    // colour under the glyphs buys.
    let mut startup = Startup::headless(Capabilities {
        colors: ColorSupport::TrueColor,
    });
    startup.save = None;
    let mut app = App::with_root(startup, Box::new(Skyline::new()));
    app.run(&mut terminal)?;
    println!("skyline: prototype finished");
    Ok(())
}
