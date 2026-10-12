//! A feasibility scene for the procedural atmosphere layer, and the harness the
//! layer's cost is measured with.
//!
//! This is **experimental**. It does not touch the game: it is a separate binary
//! built out of the same [`Screen`] trait, the same [`Terminal`], the same
//! `Presenter` and the same [`Canvas`] the game already uses, so what it proves
//! is not that a technique works in isolation but that it drops into the existing
//! architecture without changing it.
//!
//! ```sh
//! cargo run --release --example skyline
//! ```
//!
//! The layer itself is `ui::atmosphere`. This file only supplies a scene to look
//! at and a way to make it work harder.
//!
//! # Modes
//!
//! `GRIMMOIRE_SKY_MODE` selects how much is moving, so the same scene can be
//! measured at every load:
//!
//! | Mode | Atmosphere | Else | Why |
//! | --- | --- | --- | --- |
//! | `still` | drawn once, never redrawn | nothing | the idle floor |
//! | `field` | drawn once, never redrawn | **nothing at all** | the gradient with no glyphs over it |
//! | `slow` | the shipped default | nothing | the atmosphere's own cadence |
//! | `bare` | **absent** | motes and breathing at 60 Hz | the baseline to subtract |
//! | `live` | the shipped default | motes and breathing at 60 Hz | normal idle animation |
//! | `walk` | the whole field at 12 Hz | a walking figure, many motes | walking through a scene |
//! | `heavy` | the whole field at 30 Hz | everything at once | the worst case, measured separately |
//! | `overlay` | the shipped default | motes, breathing, a dialogue plate | an overlay over the field |
//!
//! `--bench [columns] [rows] [frames]` measures every mode headlessly, composing
//! and diffing into a `Vec<u8>` with no terminal in the way. That is where the
//! numbers in `docs/renderer-investigation.md` come from; it bypasses the
//! terminal only to measure, never to render.

use std::time::{Duration, Instant};

use grimmoire::app::{App, Startup};
use grimmoire::game::Figure;
use grimmoire::input::{Key, KeyCode};
use grimmoire::render::{Canvas, Color, Frame, Rect, Style};
use grimmoire::terminal::{
    Capabilities, ColorSupport, GraphicsSupport, PresentStats, Presenter, Terminal,
};
use grimmoire::ui::FrameContext;
use grimmoire::ui::atmosphere::{Atmosphere, Drift, Rgb, Sky};
use grimmoire::ui::screen::{Command, Screen};
use grimmoire::ui::sprite;
use grimmoire::ui::{Motion, Palette};

/// Brightens a colour, the way light falling on something does.
fn lit(colour: Rgb, amount: f32) -> Rgb {
    colour.mix(Rgb::new(255, 245, 224), amount.clamp(0.0, 1.0))
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
            let value = seed
                .wrapping_mul(2_654_435_761)
                .wrapping_add(n.wrapping_mul(40_503));
            (value >> 8) as f32 / 16_777_216.0
        };
        Mote {
            x: rand(1),
            rise: 0.05 + rand(3) * 0.14,
            drift: (rand(4) - 0.5) * 0.06,
            phase: rand(5),
        }
    }
}

/// How much work the scene is doing, so its cost can be measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Still,
    Slow,
    Bare,
    Field,
    Live,
    Walk,
    Heavy,
    Overlay,
}

impl Mode {
    fn from_environment() -> Self {
        match std::env::var("GRIMMOIRE_SKY_MODE").as_deref() {
            Ok("still") => Mode::Still,
            Ok("slow") => Mode::Slow,
            Ok("bare") => Mode::Bare,
            Ok("field") => Mode::Field,
            Ok("walk") => Mode::Walk,
            Ok("heavy") => Mode::Heavy,
            Ok("overlay") => Mode::Overlay,
            _ => Mode::Live,
        }
    }

    /// Whether the atmosphere is drawn at all.
    fn has_atmosphere(self) -> bool {
        self != Mode::Bare
    }

    /// Whether anything is drawn on top of the field. `field` mode deliberately
    /// draws nothing else, so the gradient can be inspected with no glyphs over
    /// it.
    fn has_scenery(self) -> bool {
        self != Mode::Field
    }

    /// The sky this mode draws: the module's own default for the load levels it
    /// is meant to measure, and an explicit override only for the worst cases.
    fn sky(self) -> Sky {
        let sky = Sky::night();
        match self {
            Mode::Still | Mode::Bare | Mode::Field => sky.stilled(),
            Mode::Walk => Sky {
                drift: Drift::Whole,
                tick_rate: 12.0,
                ..sky
            },
            Mode::Heavy => Sky {
                drift: Drift::Whole,
                tick_rate: 30.0,
                ..sky
            },
            Mode::Slow | Mode::Live | Mode::Overlay => sky,
        }
    }

    fn motes(self) -> usize {
        match self {
            Mode::Still | Mode::Slow | Mode::Field => 0,
            Mode::Bare | Mode::Live | Mode::Overlay => 40,
            Mode::Walk => 80,
            Mode::Heavy => 240,
        }
    }

    /// Whether the figure is breathing and idling on the frame clock.
    fn breathes(self) -> bool {
        matches!(
            self,
            Mode::Bare | Mode::Live | Mode::Walk | Mode::Heavy | Mode::Overlay
        )
    }

    fn walks(self) -> bool {
        matches!(self, Mode::Walk | Mode::Heavy)
    }
}

/// The prototype scene.
struct Skyline {
    mode: Mode,
    /// Seconds since the screen opened, for everything outside the atmosphere.
    time: f32,
    atmosphere: Atmosphere,
    area: Rect,
    motes: Vec<Mote>,
    selected: usize,
}

impl Skyline {
    fn new(mode: Mode, support: ColorSupport) -> Self {
        let sky = mode.sky();
        Skyline {
            mode,
            time: 0.0,
            atmosphere: Atmosphere::new(sky, support),
            area: Rect::from_size(0, 0),
            motes: (0..mode.motes() as u32).map(Mote::new).collect(),
            selected: 0,
        }
    }

    /// Distant hills, then the cathedral: glyphs, drawn over the field.
    fn compose_scenery(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        if bounds.height < 10 || bounds.width < 40 {
            return;
        }
        let ground = bounds.bottom().saturating_sub(4);
        let left = i32::from(bounds.centered_x(46)) - 6;
        // A silhouette, not a facade: against a lit sky the building reads as a
        // dark mass whose windows are the only bright thing about it. Stone the
        // same value as the sky between it is a drawing of nothing.
        let stone = Rgb::new(22, 19, 34);
        let stone_lit = Rgb::new(56, 48, 72);
        let glass = Rgb::new(255, 196, 104);
        let mut view = frame.view(bounds);

        // Hills behind the building.
        for x in 0..bounds.width {
            let f = f32::from(x) / f32::from(bounds.width);
            let ridge = (f * 6.0).sin() * 1.4 + (f * 17.0).sin() * 0.6;
            let top = (f32::from(ground) - 3.0 - ridge) as u16;
            for y in top..ground {
                let shade = if y == top {
                    Rgb::new(19, 16, 32)
                } else {
                    Rgb::new(11, 10, 21)
                };
                view.put_char(
                    x,
                    y,
                    '▒',
                    Style::new().with_fg(shade.to_color(ColorSupport::TrueColor)),
                );
            }
        }

        let base = i32::from(ground);
        let put = |view: &mut grimmoire::render::CanvasView, x: i32, y: i32, ch: char, c: Rgb| {
            if x < 0 || y < 0 {
                return;
            }
            let (x, y) = (x as u16, y as u16);
            if x >= bounds.width || y >= bounds.height {
                return;
            }
            view.put_char(
                x,
                y,
                ch,
                Style::new().with_fg(c.to_color(ColorSupport::TrueColor)),
            );
        };

        // The nave.
        for y in 0..14i32 {
            for x in 8..38i32 {
                let row = base - 1 - y;
                let ch = if x > 9 && x < 36 {
                    continue;
                } else if x == 8 || x == 37 {
                    '│'
                } else {
                    '█'
                };
                put(&mut view, left + x, row, ch, stone);
            }
        }
        // The gable.
        for step in 0..7 {
            for x in (8 + step)..(38 - step) {
                put(&mut view, left + x, base - 15 - step, '█', stone_lit);
            }
        }
        // Towers, and their spires.
        for (x0, height) in [(2i32, 20i32), (41, 17)] {
            for y in 0..height {
                for dx in 0..5 {
                    let ch = if dx == 0 || dx == 4 { '│' } else { '█' };
                    put(&mut view, left + x0 + dx, base - 1 - y, ch, stone);
                }
            }
            for step in 0..6 {
                let y = base - 1 - height - step;
                put(&mut view, left + x0 + 2, y, '▲', stone_lit);
                if step < 4 {
                    put(&mut view, left + x0 + 1, y, '·', stone_lit);
                    put(&mut view, left + x0 + 3, y, '·', stone_lit);
                }
            }
        }
        // A rose window, lit lancets, a lit doorway and columns.
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
            put(&mut view, left + 22 + dx, base - 10 + dy, ch, glass);
        }
        for x in [13i32, 17, 27, 31] {
            for y in 0..5 {
                put(&mut view, left + x, base - 4 - y, '▌', glass);
            }
        }
        for dx in 0..4 {
            put(&mut view, left + 21 + dx, base - 2, '▄', glass);
        }
        for x in [10i32, 36] {
            for y in 0..13 {
                put(&mut view, left + x, base - 1 - y, '║', stone_lit);
            }
        }
    }

    /// Motes rising off the ground, in true colour.
    fn compose_motes(&self, frame: &mut Frame) {
        if self.motes.is_empty() {
            return;
        }
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        let mut view = frame.view(bounds);
        for mote in &self.motes {
            let life = (self.time * mote.rise + mote.phase).fract();
            let v = 0.95 - life * 0.85;
            let u = (mote.x + self.time * mote.drift).rem_euclid(1.0);
            let fade = (life * std::f32::consts::PI).sin().max(0.0);
            let x = (u * f32::from(bounds.width)) as u16;
            let y = (v * f32::from(bounds.height)) as u16;
            if x >= bounds.width || y >= bounds.height {
                continue;
            }
            let warm = lit(Rgb::new(255, 214, 150), fade * 0.8);
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
                Style::new().with_fg(warm.to_color(ColorSupport::TrueColor)),
            );
        }
    }

    /// The figure, standing or walking before the doors.
    fn compose_figure(&self, frame: &mut Frame) {
        let bounds = frame.bounds();
        let art = sprite::for_figure(Figure::Mage);
        let x = bounds.centered_x(art.width);
        let y = bounds.bottom().saturating_sub(7);
        if y + art.height > bounds.height {
            return;
        }
        let mut view = frame.view(bounds);

        // A pool of light at the feet, so the figure stands somewhere.
        for dx in 0..7u16 {
            for dy in 0..2u16 {
                let px = x.saturating_sub(2).saturating_add(dx);
                let py = y.saturating_add(art.height).saturating_add(dy);
                if px < bounds.width && py < bounds.height {
                    let glow = lit(Rgb::new(255, 236, 190), 0.45 - f32::from(dy) * 0.2);
                    view.put_char(
                        px,
                        py,
                        '▁',
                        Style::new().with_fg(glow.to_color(ColorSupport::TrueColor)),
                    );
                }
            }
        }

        let pose = if self.mode.walks() {
            // A stride advanced on the clock, so the legs alternate at a walking
            // cadence without the scene needing a world to walk through.
            sprite::Pose::Walk {
                step: (self.time * 7.0) as u32,
            }
        } else if self.mode.breathes() {
            sprite::Pose::Idle {
                breathing: (self.time * 1.4).sin() > 0.5,
            }
        } else {
            sprite::Pose::standing()
        };
        sprite::draw(&mut view, x, y, art, pose, art.inks, false);
    }

    /// The wordmark, the menu, and — in `overlay` mode — a dialogue plate.
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
            let colour = Rgb::new(255, 226, 150).mix(Rgb::new(240, 120, 96), t);
            view.put_char(
                bounds
                    .centered_x(word.chars().count() as u16)
                    .saturating_add(index as u16),
                top,
                ch,
                Style::new()
                    .with_fg(colour.to_color(ColorSupport::TrueColor))
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
        let list_x = 5u16.min(bounds.width.saturating_sub(20));
        view.fill_style(
            Rect::new(
                list_x.saturating_sub(3),
                list_top.saturating_sub(1),
                30.min(bounds.width),
                (items.len() as u16) * 2 + 1,
            ),
            Style::new().with_bg(Color::Rgb(14, 12, 26)),
        );
        for (index, item) in items.iter().enumerate() {
            let y = list_top.saturating_add(index as u16 * 2);
            let chosen = index == self.selected;
            let base = if chosen {
                Rgb::new(255, 232, 168)
            } else {
                Rgb::new(176, 168, 200)
            };
            if chosen {
                view.put_str(
                    list_x.saturating_sub(2),
                    y,
                    "▸",
                    Style::new().with_fg(Color::Rgb(255, 216, 120)).bold(),
                );
            }
            view.put_str(
                list_x,
                y,
                item,
                Style::new().with_fg(base.to_color(ColorSupport::TrueColor)),
            );
        }

        if self.mode == Mode::Overlay {
            // A plate like a conversation's: an opaque surface over the field, so
            // the shot shows what an overlay does to the atmosphere.
            let plate = Rect::new(
                bounds.centered_x(56).max(2),
                bounds.height.saturating_sub(11).max(2),
                56.min(bounds.width.saturating_sub(4)),
                7,
            );
            view.fill_style(plate, Style::new().with_bg(Color::Rgb(18, 15, 30)));
            view.draw_border(plate, Style::new().with_fg(Color::Rgb(120, 104, 150)));
            view.put_str(
                plate.x + 2,
                plate.y + 1,
                "the veiled keeper",
                Style::new().with_fg(Color::Rgb(214, 178, 108)).bold(),
            );
            view.put_str(
                plate.x + 2,
                plate.y + 2,
                "The door remembers you. It always did.",
                Style::new().with_fg(Color::Rgb(206, 200, 224)),
            );
            view.put_str(
                plate.x + 2,
                plate.y + 4,
                "> Answer it",
                Style::new().with_fg(Color::Rgb(255, 226, 150)),
            );
        }

        let hint = "w/s choose · enter select · esc quit · a prototype";
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
            KeyCode::Esc | KeyCode::Char('q') => Command::Quit,
            KeyCode::Down | KeyCode::Char('s') => {
                self.selected = (self.selected + 1) % 4;
                Command::None
            }
            KeyCode::Up | KeyCode::Char('w') => {
                self.selected = (self.selected + 3) % 4;
                Command::None
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.time += delta.as_secs_f32();
        self.atmosphere.advance(delta);
        // Cheap when nothing changed: one comparison.
        self.atmosphere.resolve(self.area);
    }

    fn on_resize(&mut self, area: Rect) {
        self.area = area;
        self.atmosphere.resolve(area);
    }

    fn compose(&self, frame: &mut Frame, _context: &FrameContext) {
        if self.mode.has_atmosphere() {
            self.atmosphere.paint(frame);
        }
        if !self.mode.has_scenery() {
            return;
        }
        self.compose_scenery(frame);
        self.compose_motes(frame);
        self.compose_figure(frame);
        self.compose_ui(frame);
    }

    fn is_animating(&self) -> bool {
        // The modes that hold a whole scene moving ask for the animation rate;
        // `slow` deliberately does not, so its cost is the atmosphere's own.
        matches!(
            self.mode,
            Mode::Bare | Mode::Live | Mode::Walk | Mode::Heavy | Mode::Overlay
        )
    }

    fn next_frame(&self) -> Option<Duration> {
        if self.is_animating() {
            Some(Duration::ZERO)
        } else {
            self.atmosphere.next_update()
        }
    }
}

/// The modes, in the order `--bench` reports them.
const MODES: [Mode; 8] = [
    Mode::Still,
    Mode::Slow,
    Mode::Bare,
    Mode::Field,
    Mode::Live,
    Mode::Walk,
    Mode::Heavy,
    Mode::Overlay,
];

/// Measures composition, diffing and presentation with no terminal at all.
///
/// The presenter writes into a `Vec<u8>`, so the numbers are the renderer's own
/// cost and nothing else: no pty, no emulator, no write blocking. That is what
/// makes them repeatable, and it is the measurement the atmosphere's budget is
/// argued from. The end-to-end run through a pty is the sanity check beside it.
fn bench(columns: u16, rows: u16, frames: u32) {
    let area = Rect::from_size(columns, rows);
    let dt = Duration::from_micros(16_667);
    println!(
        "bench {columns}x{rows}, {frames} frames at 60 Hz, composition + diff + present to a Vec"
    );
    println!(
        "{:9} {:>10} {:>12} {:>12} {:>12}",
        "mode", "us/frame", "writes/frame", "bytes/frame", "fps implied"
    );
    for mode in MODES {
        let mut screen = Skyline::new(mode, ColorSupport::TrueColor);
        screen.on_resize(area);
        let context = FrameContext {
            capabilities: Capabilities {
                colors: ColorSupport::TrueColor,
            },
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Unsupported,
            art: grimmoire::ui::art::Art::none(),
            stats: PresentStats::default(),
            frame_time: dt,
            animating: screen.is_animating(),
            key_releases: true,
            motion: Motion::Full,
        };
        let mut canvas = Canvas::new(columns, rows);
        let mut images = Vec::new();
        let mut presenter = Presenter::new();
        let mut sink: Vec<u8> = Vec::new();
        let mut bytes = 0usize;
        let mut writes = 0u64;

        let started = Instant::now();
        for _ in 0..frames {
            screen.update(dt);
            {
                let mut frame = Frame::begin(&mut canvas, &mut images);
                screen.compose(&mut frame, &context);
            }
            sink.clear();
            let stats = presenter.present(&canvas, &mut sink).expect("present");
            bytes += sink.len();
            writes += u64::from(stats.cells_written);
        }
        let elapsed = started.elapsed().as_secs_f64();
        let per_frame = elapsed / f64::from(frames) * 1e6;
        println!(
            "{:9} {:>10.1} {:>12.1} {:>12.1} {:>12.0}",
            format!("{mode:?}").to_lowercase(),
            per_frame,
            writes as f64 / f64::from(frames),
            bytes as f64 / f64::from(frames),
            1e6 / per_frame,
        );
    }
}

fn main() -> std::io::Result<()> {
    // `--bench` measures the renderer headlessly and exits; every other
    // invocation drives the real terminal.
    let argument = std::env::args().nth(1);
    if argument.as_deref() == Some("--bench") {
        let columns = std::env::args()
            .nth(2)
            .and_then(|value| value.parse().ok())
            .unwrap_or(150);
        let rows = std::env::args()
            .nth(3)
            .and_then(|value| value.parse().ok())
            .unwrap_or(37);
        let frames = std::env::args()
            .nth(4)
            .and_then(|value| value.parse().ok())
            .unwrap_or(600);
        bench(columns, rows, frames);
        return Ok(());
    }

    grimmoire::terminal::install_panic_hook();
    let mode = Mode::from_environment();
    let mut terminal = Terminal::new()?;
    // True colour, explicitly: this prototype is about what a layer of 24-bit
    // colour underneath the glyphs buys.
    let mut startup = Startup::headless(Capabilities {
        colors: ColorSupport::TrueColor,
    });
    startup.save = None;
    let mut app = App::with_root(
        startup,
        Box::new(Skyline::new(mode, ColorSupport::TrueColor)),
    );
    app.run(&mut terminal)?;
    println!("skyline: {mode:?} finished");
    Ok(())
}
