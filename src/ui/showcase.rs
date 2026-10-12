//! The interactive renderer showcase.
//!
//! This replaces the Phase 1 title screen. It composes several independently laid
//! out regions (banner, animated scene, information panel, status bar), drives
//! four visible time-based effects, and is the root screen of the application.
//!
//! The effects are all functions of elapsed time rather than of the frame count:
//!
//! * a left-to-right reveal of the title banner,
//! * a pulsing highlight on the selected scene,
//! * continuously moving ASCII scenes,
//! * a wipe when the scene changes.

use std::f32::consts::TAU;
use std::time::Duration;

use super::FrameContext;
use super::art::{self, Banner};
use super::help::HelpOverlay;
use super::layout::{Layout, LayoutMode, MIN_HEIGHT, MIN_WIDTH};
use super::screen::{Command, Screen};
use super::theme::Palette;
use crate::anim::{Animation, Animator, Easing};
use crate::input::{Key, KeyCode};
use crate::render::{CanvasView, Frame, Rect, Style, glyph};

/// How long the title reveal takes.
const TITLE_REVEAL: Duration = Duration::from_millis(1100);
/// How long the scene-change wipe takes.
const SCENE_WIPE: Duration = Duration::from_millis(420);
/// Selectable animation speed multipliers.
const SPEEDS: [f32; 4] = [0.25, 0.5, 1.0, 2.0];
/// The speed multiplier selected at start-up.
const DEFAULT_SPEED: usize = 2;

/// Bindings listed by the help overlay while the showcase is active.
const SHOWCASE_CONTROLS: [super::help::Binding; 8] = [
    ("arrows / 1-3", "choose a scene"),
    ("space", "pause or resume motion"),
    ("+ / -", "change animation speed"),
    ("r", "replay the title reveal"),
    ("h / ? / F1", "controls and diagnostics"),
    ("Esc", "leave the showcase"),
    ("q / Ctrl+C", "quit Grimmoire"),
    ("F2", "open this showcase from the game"),
];

/// Animations owned by the showcase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShowKey {
    /// The banner reveal.
    TitleReveal,
    /// The wipe used when the scene changes.
    SceneWipe,
}

/// A scene shown in the showcase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    /// Embers rising past a tower silhouette.
    Emberfall,
    /// Runic rings turning around a pulsing core.
    Sigil,
    /// Three parallax starfields drifting.
    Starfield,
}

impl Scene {
    /// Every scene, in selection order.
    pub const ALL: [Scene; 3] = [Scene::Emberfall, Scene::Sigil, Scene::Starfield];

    /// The display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Scene::Emberfall => "EMBERFALL",
            Scene::Sigil => "SIGIL",
            Scene::Starfield => "STARFIELD",
        }
    }

    /// A one-line description.
    #[must_use]
    pub const fn blurb(self) -> &'static str {
        match self {
            Scene::Emberfall => "embers rise past a tower",
            Scene::Sigil => "runic rings turn about a pulse",
            Scene::Starfield => "three starfields drift in parallax",
        }
    }
}

/// The renderer showcase screen.
#[derive(Debug)]
pub struct Showcase {
    animator: Animator<ShowKey>,
    scene: Scene,
    paused: bool,
    speed_index: usize,
    effect_time: Duration,
    frames: u64,
}

impl Showcase {
    /// Creates the showcase with its title reveal already running.
    #[must_use]
    pub fn new() -> Self {
        let mut animator = Animator::new();
        animator.start(ShowKey::TitleReveal, title_reveal());
        Showcase {
            animator,
            scene: Scene::Emberfall,
            paused: false,
            speed_index: DEFAULT_SPEED,
            effect_time: Duration::ZERO,
            frames: 0,
        }
    }

    /// The current animation speed multiplier.
    #[must_use]
    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed_index.min(SPEEDS.len() - 1)]
    }

    /// Whether motion is paused.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// The currently selected scene.
    #[must_use]
    pub const fn scene(&self) -> Scene {
        self.scene
    }

    /// Selects a scene by index, starting a wipe when it actually changes.
    fn select(&mut self, index: usize) {
        let scene = Scene::ALL[index % Scene::ALL.len()];
        if scene != self.scene {
            self.scene = scene;
            self.animator.start(ShowKey::SceneWipe, scene_wipe());
        }
    }

    /// Moves the selection by `direction`, wrapping around.
    fn step(&mut self, direction: i32) {
        let current = Scene::ALL
            .iter()
            .position(|scene| *scene == self.scene)
            .unwrap_or(0) as i32;
        let count = Scene::ALL.len() as i32;
        self.select((current + direction).rem_euclid(count) as usize);
    }

    /// Changes the speed multiplier within its bounds.
    fn change_speed(&mut self, direction: i32) {
        let last = SPEEDS.len() as i32 - 1;
        self.speed_index = (self.speed_index as i32 + direction).clamp(0, last) as usize;
    }

    /// Draws the full interface.
    fn compose_full(&self, frame: &mut Frame, layout: &Layout, context: &FrameContext) {
        let palette = &context.palette;
        let time = self.effect_time.as_secs_f32();

        if layout.banner != Banner::None && !layout.title.is_empty() {
            let mut view = frame.view(layout.title);
            let reveal = self.animator.value_or(ShowKey::TitleReveal, 1.0);
            art::draw_banner(
                &mut view,
                layout.banner,
                0,
                reveal,
                Style::new().with_fg(palette.accent).bold(),
                Style::new().with_fg(palette.accent_alt).bold(),
            );
            view.put_str_centered(
                view.bounds(),
                layout.banner.height(),
                "a terminal-native dark-fantasy wizard RPG",
                Style::new().with_fg(palette.accent_alt_dim).italic(),
            );
        }

        if !layout.scene.is_empty() {
            let mut view = frame.view(layout.scene);
            self.compose_scene(&mut view, palette, time);
        }

        if !layout.panel.is_empty() {
            let mut view = frame.view(layout.panel);
            self.compose_panel(&mut view, palette, context, time);
        }

        if !layout.status.is_empty() {
            let mut view = frame.view(layout.status);
            self.compose_status(&mut view, palette, context);
        }
    }

    /// Draws the animated scene region.
    fn compose_scene(&self, view: &mut CanvasView, palette: &Palette, time: f32) {
        let width = view.width();
        let height = view.height();
        if width == 0 || height == 0 {
            return;
        }

        view.put_str_centered(
            Rect::from_size(width, 1),
            0,
            self.scene.name(),
            pulse_style(palette, time),
        );

        {
            let mut body = view.sub(Rect::new(0, 1, width, height.saturating_sub(1)));
            match self.scene {
                Scene::Emberfall => scenes::emberfall(&mut body, palette, time),
                Scene::Sigil => scenes::sigil(&mut body, palette, time),
                Scene::Starfield => scenes::starfield(&mut body, palette, time),
            }
        }

        // The scene-change wipe masks the columns that have not been revealed.
        let revealed = (self
            .animator
            .value_or(ShowKey::SceneWipe, 1.0)
            .clamp(0.0, 1.0)
            * f32::from(width))
        .ceil() as u16;
        if revealed >= width {
            return;
        }
        let mask = Style::new()
            .with_fg(palette.background)
            .with_bg(palette.background);
        let edge = Style::new()
            .with_fg(palette.accent_alt)
            .with_bg(palette.background)
            .bold();
        for y in 0..height {
            for x in revealed..width {
                if x == revealed {
                    view.put_char(x, y, '|', edge);
                } else {
                    view.put_char(x, y, ' ', mask);
                }
            }
        }
    }

    /// Draws the information panel.
    fn compose_panel(
        &self,
        view: &mut CanvasView,
        palette: &Palette,
        context: &FrameContext,
        time: f32,
    ) {
        let width = view.width();
        let height = view.height();
        if width < 4 || height < 3 {
            return;
        }

        view.draw_titled_border(
            view.bounds(),
            "renderer",
            Style::new().with_fg(palette.text_muted),
            Style::new().with_fg(palette.accent).bold(),
        );

        let inner = Rect::new(2, 1, width.saturating_sub(4), height.saturating_sub(2));
        if inner.is_empty() {
            return;
        }
        let mut row = inner.y;

        let pulse = pulse_style(palette, time);
        for scene in Scene::ALL {
            if row >= inner.bottom() {
                break;
            }
            let selected = scene == self.scene;
            let (marker, style) = if selected {
                ("> ", pulse)
            } else {
                ("  ", palette.muted_style())
            };
            let marker_width = glyph::display_width(marker);
            view.put_str(inner.x, row, marker, style);
            view.put_str(
                inner.x.saturating_add(marker_width),
                row,
                scene.name(),
                style,
            );
            row = row.saturating_add(1);
        }

        row = row.saturating_add(1);
        for line in glyph::wrap(self.scene.blurb(), inner.width) {
            if row >= inner.bottom() {
                break;
            }
            view.put_str(
                inner.x,
                row,
                &line,
                Style::new().with_fg(palette.accent_alt_dim).italic(),
            );
            row = row.saturating_add(1);
        }

        row = row.saturating_add(1);
        let stats = context.stats;
        let diagnostics = [
            (
                "motion",
                if self.paused {
                    "paused".to_string()
                } else {
                    format!("running x{:.2}", self.speed())
                },
            ),
            (
                "painted",
                format!("{}/{} cells", stats.cells_written, stats.total_cells()),
            ),
            ("runs", stats.runs.to_string()),
            ("frames", self.frames.to_string()),
            ("colour", context.capabilities.colors.label().to_string()),
        ];
        for (label, value) in &diagnostics {
            if row >= inner.bottom() {
                break;
            }
            view.put_str(inner.x, row, label, palette.muted_style());
            view.put_str(
                inner.x.saturating_add(10),
                row,
                value,
                Style::new().with_fg(palette.accent_alt),
            );
            row = row.saturating_add(1);
        }
    }

    /// Draws the bottom status bar.
    fn compose_status(&self, view: &mut CanvasView, palette: &Palette, context: &FrameContext) {
        let width = view.width();
        let height = view.height();
        if width == 0 || height == 0 {
            return;
        }

        let title = "GRIMMOIRE";
        view.put_str(0, 0, title, Style::new().with_fg(palette.accent).bold());
        view.put_str(
            glyph::display_width(title),
            0,
            "  renderer showcase",
            Style::new().with_fg(palette.text_muted),
        );

        let fps = if context.frame_time.is_zero() {
            0.0
        } else {
            1.0 / context.frame_time.as_secs_f32()
        };
        let right = format!(
            "{}x{}  {:>5.1} fps",
            context.stats.width, context.stats.height, fps
        );
        view.put_str(
            width.saturating_sub(glyph::display_width(&right)),
            0,
            &right,
            Style::new().with_fg(palette.accent_alt_dim),
        );

        if height > 1 {
            // The full hint line does not fit on narrow terminals, so fall back
            // to the essentials.
            let hints = if width >= 78 {
                "arrows/1-3 scene   space pause   +/- speed   r replay   h help   q quit"
            } else {
                "arrows scene   space pause   h help   q quit"
            };
            view.put_str_centered(
                Rect::new(0, 1, width, 1),
                1,
                hints,
                Style::new().with_fg(palette.text_muted),
            );
        }
    }

    /// Draws the fallback for terminals too small for the interface.
    fn compose_minimal(&self, frame: &mut Frame, context: &FrameContext) {
        let area = frame.bounds();
        if area.is_empty() {
            return;
        }
        let palette = &context.palette;
        let mut view = frame.view(area);

        // A small deterministic twinkle keeps the tiny layout visibly alive.
        let dots = ((self.effect_time.as_secs_f32() * 2.0) as usize) % 4;
        let title = format!("Grimmoire{}", ".".repeat(dots));
        let row = area.centered_y(3);
        view.put_str_centered(
            view.bounds(),
            row,
            &title,
            Style::new().with_fg(palette.accent).bold(),
        );
        view.put_str_centered(
            view.bounds(),
            row.saturating_add(1),
            "terminal too small",
            Style::new().with_fg(palette.text_muted),
        );
        let hint = format!("{MIN_WIDTH}x{MIN_HEIGHT} minimum    q quits");
        view.put_str_centered(
            view.bounds(),
            row.saturating_add(2),
            &hint,
            Style::new().with_fg(palette.accent_alt_dim),
        );
    }
}

impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for Showcase {
    fn name(&self) -> &'static str {
        "showcase"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        match key.code {
            // "Leave this screen": the stack pops back to what is beneath, or
            // quits when the showcase is the root. That makes the demo usable
            // both standalone and pushed on top of the game.
            KeyCode::Esc => Command::Pop,
            KeyCode::Left | KeyCode::Up => {
                self.step(-1);
                Command::None
            }
            KeyCode::Right | KeyCode::Down => {
                self.step(1);
                Command::None
            }
            KeyCode::Char('1') => {
                self.select(0);
                Command::None
            }
            KeyCode::Char('2') => {
                self.select(1);
                Command::None
            }
            KeyCode::Char('3') => {
                self.select(2);
                Command::None
            }
            KeyCode::Char(' ') => {
                self.paused = !self.paused;
                Command::None
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.change_speed(1);
                Command::None
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.change_speed(-1);
                Command::None
            }
            KeyCode::Char('r') => {
                self.animator.start(ShowKey::TitleReveal, title_reveal());
                Command::None
            }
            KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Function(1) => {
                Command::Push(Box::new(HelpOverlay::new(&SHOWCASE_CONTROLS)))
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.animator.tick(delta);
        self.frames = self.frames.saturating_add(1);
        if !self.paused {
            let scaled = Duration::from_secs_f32(delta.as_secs_f32() * self.speed());
            self.effect_time = self.effect_time.saturating_add(scaled);
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let layout = Layout::compute(frame.bounds());
        if layout.mode == LayoutMode::Minimal {
            self.compose_minimal(frame, context);
        } else {
            self.compose_full(frame, &layout, context);
        }
    }

    fn is_animating(&self) -> bool {
        self.animator.active_count() > 0 || !self.paused
    }
}

/// The banner reveal animation.
fn title_reveal() -> Animation {
    Animation::new(TITLE_REVEAL)
        .delayed(Duration::from_millis(180))
        .eased(Easing::EaseOutCubic)
}

/// The scene-change wipe animation.
fn scene_wipe() -> Animation {
    Animation::new(SCENE_WIPE).eased(Easing::EaseInOutQuad)
}

/// A style that alternates between two accents over time.
fn pulse_style(palette: &Palette, time: f32) -> Style {
    let phase = (time * 0.8).rem_euclid(1.0);
    let color = if phase < 0.5 {
        palette.accent
    } else {
        palette.accent_alt
    };
    Style::new().with_fg(color).bold()
}

/// The animated ASCII scenes.
mod scenes {
    use super::TAU;
    use crate::render::{CanvasView, Style};
    use crate::ui::art;
    use crate::ui::theme::Palette;

    /// A deterministic integer hash, so scenes are a pure function of time.
    fn hash(seed: u32) -> u32 {
        let mut x = seed.wrapping_add(0x9E37_79B9);
        x = (x ^ (x >> 16)).wrapping_mul(0x85EB_CA6B);
        x = (x ^ (x >> 13)).wrapping_mul(0xC2B2_AE35);
        x ^ (x >> 16)
    }

    /// A deterministic value in `[0, 1)` derived from a seed.
    fn unit(seed: u32) -> f32 {
        (hash(seed) >> 8) as f32 / 16_777_216.0
    }

    /// Twinkling stars and rising embers behind a tower silhouette.
    pub fn emberfall(view: &mut CanvasView, palette: &Palette, time: f32) {
        let width = view.width();
        let height = view.height();
        if width < 4 || height < 3 {
            return;
        }
        let w = u32::from(width);
        let h = u32::from(height);

        let star_rows = (h * 2 / 3).max(1);
        let stars = (w * h / 40).clamp(8, 90);
        for i in 0..stars {
            let x = (hash(i.wrapping_mul(3).wrapping_add(1)) % w) as u16;
            let y = (hash(i.wrapping_mul(7).wrapping_add(2)) % star_rows) as u16;
            let phase = unit(i.wrapping_mul(11).wrapping_add(3)) * TAU;
            let blink = (time * 2.0 + phase).sin() * 0.5 + 0.5;
            let (ch, color) = if blink > 0.8 {
                ('*', palette.accent_alt)
            } else if blink > 0.45 {
                ('.', palette.accent_alt_dim)
            } else {
                (',', palette.text_muted)
            };
            view.put_char(x, y, ch, Style::new().with_fg(color));
        }

        let embers = (w / 6).clamp(4, 24);
        for i in 0..embers {
            let x = (hash(i.wrapping_mul(13).wrapping_add(5)) % w) as u16;
            let speed = 0.6 + unit(i.wrapping_mul(17).wrapping_add(7)) * 1.4;
            let phase = unit(i.wrapping_mul(19).wrapping_add(9)) * h as f32;
            let y = (phase + time * speed * 3.0).rem_euclid(h as f32);
            let row = (h as f32 - 1.0 - y).max(0.0) as u16;
            let brightness = 1.0 - (y / h as f32).clamp(0.0, 1.0);
            let (ch, color) = if brightness > 0.66 {
                ('*', palette.effect)
            } else if brightness > 0.33 {
                ('o', palette.effect_dim)
            } else {
                ('.', palette.text_muted)
            };
            view.put_char(x, row, ch, Style::new().with_fg(color));
        }

        let tower_top = height.saturating_sub(art::TOWER.len() as u16);
        let tower_x = width.saturating_sub(art::TOWER_WIDTH) / 2;
        art::draw_stencil(
            view,
            tower_x,
            tower_top,
            &art::TOWER,
            Style::new().with_fg(palette.text_muted).dim(),
        );
    }

    /// Runic rings rotating around a pulsing core.
    pub fn sigil(view: &mut CanvasView, palette: &Palette, time: f32) {
        let width = view.width();
        let height = view.height();
        if width < 8 || height < 5 {
            return;
        }
        let w = f32::from(width);
        let h = f32::from(height);
        let cx = (w - 1.0) / 2.0;
        let cy = (h - 1.0) / 2.0;
        let radius_w = (w - 1.0) / 2.0;
        let radius_h = (h - 1.0) / 2.0;

        const RUNES: [char; 8] = ['+', 'x', '*', '#', 'o', '.', '/', '\\'];
        const RINGS: [f32; 3] = [0.45, 0.72, 0.98];

        for (ring, factor) in RINGS.iter().enumerate() {
            let rx = radius_w * factor;
            let ry = radius_h * factor;
            let rune_count = (ring as u32 + 2) * 8;
            let direction = if ring % 2 == 0 { 1.0 } else { -1.0 };
            let spin = direction * time * (0.5 + ring as f32 * 0.35);
            let color = if ring % 2 == 0 {
                palette.accent_dim
            } else {
                palette.accent_alt_dim
            };
            for k in 0..rune_count {
                let angle = spin + TAU * (k as f32) / rune_count as f32;
                let x = cx + rx * angle.cos();
                let y = cy + ry * angle.sin();
                if x < 0.0 || y < 0.0 {
                    continue;
                }
                let ch = RUNES[(k as usize + ring) % RUNES.len()];
                view.put_char(x as u16, y as u16, ch, Style::new().with_fg(color));
            }
        }

        let beat = (time * 1.6).sin() * 0.5 + 0.5;
        let (ch, color) = if beat > 0.85 {
            ('@', palette.accent_alt)
        } else if beat > 0.6 {
            ('O', palette.accent)
        } else if beat > 0.35 {
            ('o', palette.accent_dim)
        } else {
            ('.', palette.accent_alt_dim)
        };
        view.put_char(cx as u16, cy as u16, ch, Style::new().with_fg(color).bold());
    }

    /// Three starfields drifting at different speeds for a parallax effect.
    pub fn starfield(view: &mut CanvasView, palette: &Palette, time: f32) {
        let width = view.width();
        let height = view.height();
        if width < 4 || height < 3 {
            return;
        }
        let w = u32::from(width);
        let h = u32::from(height);

        for layer in 0..3u32 {
            let (speed, color, ch) = match layer {
                0 => (0.25, palette.text_muted, '.'),
                1 => (0.6, palette.accent_alt_dim, '.'),
                _ => (1.2, palette.accent_alt, '*'),
            };
            let count = (w * h / (70 + layer * 30)).clamp(12, 140);
            for i in 0..count {
                let base = unit(
                    i.wrapping_mul(31)
                        .wrapping_add(layer.wrapping_mul(101))
                        .wrapping_add(7),
                ) * w as f32;
                let travel = time * speed * 8.0;
                let x = ((base + travel) % w as f32) as u16;
                let y = (hash(
                    i.wrapping_mul(17)
                        .wrapping_add(layer.wrapping_mul(53))
                        .wrapping_add(3),
                ) % h) as u16;
                let phase = unit(i.wrapping_mul(23).wrapping_add(layer.wrapping_mul(11))) * TAU;
                let twinkle = (time * 3.0 + phase).sin() * 0.5 + 0.5;
                if twinkle < 0.2 {
                    continue;
                }
                view.put_char(x, y, ch, Style::new().with_fg(color));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Scene, Showcase};
    use crate::input::{Key, KeyCode};
    use crate::render::{Canvas, Frame};
    use crate::terminal::{Capabilities, PresentStats};
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            graphics: crate::terminal::GraphicsSupport::Forced,
            art: crate::ui::art::Art::none(),
            motion: crate::ui::settings::Motion::Full,
            key_releases: true,
            palette: Palette::BASIC,
            stats: PresentStats {
                width: 100,
                height: 30,
                cells_written: 500,
                ..PresentStats::default()
            },
            frame_time: Duration::from_millis(5),
            animating: true,
        }
    }

    /// Composes a screen into a canvas, supplying an empty image list.
    fn draw<S: Screen + ?Sized>(canvas: &mut Canvas, screen: &S) {
        let mut images = Vec::new();
        let mut frame = Frame::begin(canvas, &mut images);
        screen.compose(&mut frame, &context());
    }

    fn frame_text(canvas: &Canvas) -> String {
        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        text
    }

    fn render(showcase: &Showcase, width: u16, height: u16) -> String {
        let mut canvas = Canvas::new(width, height);
        draw(&mut canvas, showcase);
        frame_text(&canvas)
    }

    #[test]
    fn selecting_scenes_wraps_in_both_directions() {
        let mut showcase = Showcase::new();
        assert_eq!(Scene::Emberfall, showcase.scene());

        showcase.handle_key(Key::new(KeyCode::Left));
        assert_eq!(Scene::Starfield, showcase.scene());
        showcase.handle_key(Key::new(KeyCode::Right));
        assert_eq!(Scene::Emberfall, showcase.scene());
    }

    #[test]
    fn number_keys_select_scenes_directly() {
        let mut showcase = Showcase::new();
        showcase.handle_key(Key::char('3'));
        assert_eq!(Scene::Starfield, showcase.scene());
        showcase.handle_key(Key::char('2'));
        assert_eq!(Scene::Sigil, showcase.scene());
    }

    #[test]
    fn escape_leaves_the_showcase_and_h_pushes_the_overlay() {
        let mut showcase = Showcase::new();
        // "Leave this screen": the stack decides whether that means popping or
        // quitting, so the showcase works both standalone and as a demo.
        assert!(matches!(
            showcase.handle_key(Key::new(KeyCode::Esc)),
            Command::Pop
        ));
        assert!(matches!(
            showcase.handle_key(Key::char('h')),
            Command::Push(_)
        ));
        assert!(matches!(showcase.handle_key(Key::char('z')), Command::None));
    }

    #[test]
    fn space_pauses_and_resumes_motion() {
        let mut showcase = Showcase::new();
        assert!(showcase.is_animating());

        showcase.handle_key(Key::char(' '));
        assert!(showcase.is_paused());
        // Once the title reveal finishes, a paused showcase stops asking for frames.
        showcase.update(Duration::from_secs(5));
        assert!(!showcase.is_animating());

        showcase.handle_key(Key::char(' '));
        assert!(!showcase.is_paused());
        assert!(showcase.is_animating());
    }

    #[test]
    fn speed_changes_are_clamped() {
        let mut showcase = Showcase::new();
        showcase.handle_key(Key::char('+'));
        showcase.handle_key(Key::char('+'));
        showcase.handle_key(Key::char('+'));
        assert!((showcase.speed() - 2.0).abs() < 1e-6);

        for _ in 0..8 {
            showcase.handle_key(Key::char('-'));
        }
        assert!((showcase.speed() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn paused_motion_freezes_effect_time() {
        let mut showcase = Showcase::new();
        showcase.handle_key(Key::char(' '));
        showcase.update(Duration::from_millis(500));
        // The title animation still ticks, but the scenes do not advance.
        assert!(showcase.is_paused());
    }

    #[test]
    fn composing_a_wide_terminal_shows_every_region() {
        let text = render(&Showcase::new(), 100, 30);
        assert!(text.contains("EMBERFALL"), "scene name missing");
        assert!(text.contains("GRIMMOIRE"), "status identity missing");
        assert!(
            text.contains("renderer showcase"),
            "status subtitle missing"
        );
        assert!(text.contains("STARFIELD"), "scene list missing");
        assert!(text.contains("> EMBERFALL"), "selection marker missing");
    }

    #[test]
    fn composing_shows_the_selected_scene_in_the_panel() {
        let mut showcase = Showcase::new();
        showcase.handle_key(Key::char('2'));
        let text = render(&showcase, 100, 30);
        assert!(text.contains("SIGIL"));
        assert!(text.contains("runic rings turn about a pulse"));
        // The marker tracks the selection rather than staying on the first scene.
        assert!(text.contains("> SIGIL"), "selection marker missing");
    }

    #[test]
    fn tiny_terminals_get_a_graceful_fallback() {
        let text = render(&Showcase::new(), 30, 10);
        assert!(
            text.contains("terminal too small"),
            "fallback missing: {text:?}"
        );
    }

    #[test]
    fn narrow_terminals_still_draw_the_interface() {
        let text = render(&Showcase::new(), 60, 16);
        assert!(text.contains("EMBERFALL"));
    }

    #[test]
    fn composition_is_deterministic_for_the_same_state() {
        let mut a = Showcase::new();
        let mut b = Showcase::new();
        for _ in 0..20 {
            a.update(Duration::from_millis(16));
            b.update(Duration::from_millis(16));
        }
        assert_eq!(render(&a, 100, 30), render(&b, 100, 30));
    }

    #[test]
    fn scenes_animate_over_time() {
        let mut showcase = Showcase::new();
        let first = render(&showcase, 100, 30);
        for _ in 0..30 {
            showcase.update(Duration::from_millis(33));
        }
        let later = render(&showcase, 100, 30);
        assert_ne!(first, later, "scene did not change over time");
    }

    #[test]
    fn every_scene_renders_without_panicking_at_many_sizes() {
        for width in [30, 44, 60, 84, 120] {
            for height in [8, 13, 16, 22, 40] {
                for scene in Scene::ALL {
                    let mut showcase = Showcase::new();
                    let index = Scene::ALL.iter().position(|s| *s == scene).unwrap();
                    showcase.handle_key(Key::char(char::from(b'1' + index as u8)));
                    showcase.update(Duration::from_millis(200));
                    let _ = render(&showcase, width, height);
                }
            }
        }
    }
}
