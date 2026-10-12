//! The application coordinator: input, update, compose, present.
//!
//! The loop owns the clock, the screen stack, the transition, and the frame
//! buffer. It deliberately keeps responsibilities thin: it advances time, routes
//! input, recomposes the frame on demand, and hands the frame to the presenter.
//! Frame layout lives in [`crate::ui`], terminal I/O in [`crate::terminal`].
//!
//! # When frames happen
//!
//! Screens report how soon they want another frame. Continuous motion - a step,
//! a fading pulse, a transition - asks for frames at the animation rate
//! ([`FRAME_INTERVAL`], sixty a second); slow ambient scenery asks for one every
//! few tens of milliseconds; a settled screen asks for none and the loop blocks
//! on input. An idle game therefore costs almost nothing while still breathing.
//!
//! The animation clock is **deadline based**. Each frame is scheduled from the
//! previous frame's deadline rather than by sleeping a fixed interval after
//! composing, so composition time does not accumulate into drift: a loop asked
//! for sixty frames a second delivers sixty a second whether a frame takes 0.3 ms
//! or 3 ms.
//!
//! Sixty updates a second is not the same as sixty *distinct* frames on screen.
//! A frame is composed and diffed every tick, but only the cells that actually
//! changed are written, and on a still frame that is nothing at all.

use std::io;
use std::time::{Duration, Instant};

use crate::anim::Animator;
use crate::assets::Assets;
use crate::input::{Event, Key, KeyCode};
use crate::render::{Canvas, Frame, ImagePlacement};
use crate::save::SaveStore;
use crate::terminal::{Capabilities, GraphicsSupport, PresentStats, Presenter, Terminal};
use crate::ui::art::Art;
use crate::ui::transition;
use crate::ui::{ExplorationScreen, FrameContext, Palette, Screen, ScreenStack, viewport};

/// The interval between frames while something is animating.
///
/// Sixty a second: enough for a fade or a curtain to read as motion, cheap
/// enough that the compose and diff on a still frame is lost in the noise.
const FRAME_INTERVAL: Duration = crate::ui::animation::FRAME_INTERVAL;
/// How long to wait for input while nothing is animating.
const IDLE_TIMEOUT: Duration = Duration::from_millis(200);
/// Upper bound on a single time step, so a stall cannot make movement jump.
const MAX_DELTA: Duration = Duration::from_millis(60);
/// The shortest an animation frame may be waited for.
///
/// Without a floor, a slow compose would leave the deadline permanently in the
/// past and the loop would spin at full speed instead of pacing itself.
const MIN_WAIT: Duration = Duration::from_micros(250);

/// Everything the application needs before it starts, resolved by the caller so
/// that failures are reported before the alternate screen is entered.
pub struct Startup {
    /// Detected colour support.
    pub capabilities: Capabilities,
    /// Whether terminal graphics can be used.
    pub graphics: GraphicsSupport,
    /// Generated artwork.
    pub assets: Assets,
    /// The artwork, by role.
    pub art: Art,
    /// Where to save, if anywhere.
    pub save: Option<SaveStore>,
    /// The player's preferences, shared with the screens that change them.
    pub settings: crate::ui::Shared,
}

impl Startup {
    /// A startup with no artwork, no saving, and no graphics: used by tests.
    #[must_use]
    pub fn headless(capabilities: Capabilities) -> Self {
        Startup {
            capabilities,
            graphics: GraphicsSupport::Unsupported,
            assets: Assets::default(),
            art: Art::none(),
            save: None,
            settings: crate::ui::Shared::new(crate::ui::Settings::new()),
        }
    }
}

/// Animations owned by the application rather than by a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppAnimation {
    /// The curtain shown when the screen stack changes.
    Transition,
}

/// The top-level application.
pub struct App {
    capabilities: Capabilities,
    graphics: GraphicsSupport,
    assets: Assets,
    art: Art,
    palette: Palette,
    stack: ScreenStack,
    transition: Animator<AppAnimation>,
    presenter: Presenter,
    frame: Canvas,
    frame_images: Vec<ImagePlacement>,
    stats: PresentStats,
    frame_time: Duration,
    /// Frames presented since the rate was last worked out.
    frame_count: u32,
    /// Frames presented since the application started, for the exit report.
    frames_presented: u64,
    /// Bytes written to the terminal since the application started.
    bytes_presented: u64,
    /// When the application started, for the exit report.
    started: Instant,
    /// When the rate was last worked out.
    frame_window: Instant,
    /// The most recent measured frame rate.
    frames_per_second: f32,
    key_releases: bool,
    /// The player's preferences, shared with the screens that change them.
    settings: crate::ui::Shared,
    /// The terminal's cell size in device pixels, when it reports one.
    cell_px: Option<(u16, u16)>,
    running: bool,
    dirty: bool,
}

impl App {
    /// Creates the application from resolved startup information.
    #[must_use]
    pub fn new(mut startup: Startup) -> Self {
        let portrait = startup.art.portrait;
        let save = startup.save.take();
        let settings = startup.settings.clone();
        let screen = crate::ui::TitleScreen::new(save, portrait, settings);
        Self::with_root(startup, Box::new(screen))
    }

    /// Creates an application with explicit capabilities and nothing else.
    ///
    /// Used by tests: deterministic, with no environment or filesystem access.
    #[must_use]
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        let startup = Startup::headless(capabilities);
        let screen = ExplorationScreen::from_save(None, None);
        Self::with_root(startup, Box::new(screen))
    }

    /// Creates an application with an explicit root screen.
    #[must_use]
    pub fn with_root(startup: Startup, root: Box<dyn Screen>) -> Self {
        let mut stack = ScreenStack::new();
        stack.push_boxed(root);
        App {
            capabilities: startup.capabilities,
            graphics: startup.graphics,
            assets: startup.assets,
            art: startup.art,
            settings: startup.settings,
            palette: Palette::for_colors(startup.capabilities.colors),
            stack,
            transition: Animator::new(),
            presenter: Presenter::with_graphics(startup.graphics),
            frame: Canvas::new(0, 0),
            frame_images: Vec::new(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            frame_count: 0,
            frames_presented: 0,
            bytes_presented: 0,
            started: Instant::now(),
            frame_window: Instant::now(),
            frames_per_second: 0.0,
            key_releases: false,
            cell_px: None,
            running: true,
            dirty: true,
        }
    }

    /// Runs until the user quits, rendering to `terminal`.
    ///
    /// The terminal is entered and left here, so it is restored on both the
    /// success and the error path. Screens are told the application is finishing
    /// before the terminal is released, which is when they persist their state.
    pub fn run(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        terminal.enter()?;
        self.key_releases = terminal.keyboard_enhanced();
        self.stack.on_terminal_ready(self.key_releases);
        self.refresh_cell_px();

        let result = self.event_loop(terminal);
        self.stack.on_exit();
        // Deleting the images before leaving means the terminal is never left
        // holding graphics, even on the error path.
        let _ = self.presenter.clear_graphics(terminal.writer());
        let left = terminal.leave();
        // After leaving, so the report lands on an ordinary screen instead of
        // being wiped with the alternate one.
        self.report_frame_stats();
        result.and(left)
    }

    /// The main loop: pump input, advance time, recompose when needed, present.
    fn event_loop(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        // The clock starts with the loop, not with startup: the keyboard
        // handshake can block for seconds on a terminal that never answers, and
        // counting that as idle time would understate the frame rate.
        self.started = Instant::now();
        let mut last = Instant::now();
        let mut last_frame = Instant::now();
        let mut deadline = Instant::now();
        while self.running {
            let animating = self.is_animating();
            // While animating, wait exactly long enough to hit the next frame;
            // otherwise wait however long the screens ask for.
            let timeout = if animating {
                // Never wait longer than one animation frame: the deadline may
                // have been set while the game was idle, and inheriting an idle
                // wait is exactly how a "60 Hz" loop ends up at 30.
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(FRAME_INTERVAL)
                    .max(MIN_WAIT)
            } else {
                self.stack
                    .next_frame()
                    .map_or(IDLE_TIMEOUT, |wait| wait.min(IDLE_TIMEOUT))
            };
            let had_event = self.pump_events(terminal, timeout)?;

            let now = Instant::now();
            let delta = now.saturating_duration_since(last).min(MAX_DELTA);
            last = now;
            self.frame_time = delta;
            self.advance(delta);

            let (width, height) = terminal.size()?;
            self.set_size(width, height);

            let elapsed = now.saturating_duration_since(last_frame);
            let animating = self.is_animating();
            let frame_due =
                animating || self.stack.next_frame().is_some_and(|wait| wait <= elapsed);

            if self.dirty || had_event || frame_due {
                self.compose_frame();
                self.stats = self.presenter.present_with(
                    &self.frame,
                    &self.frame_images,
                    &self.assets,
                    terminal.writer(),
                )?;
                self.dirty = false;
                last_frame = now;
                self.count_frame(now);
                // The presenter builds fresh statistics each frame, so the rate it
                // cannot know is put back in.
                self.stats.frames_per_second = self.frames_per_second;
                // Schedule from the deadline, not from now, so composition time
                // does not push every later frame later still.
                deadline = if animating {
                    deadline
                        .checked_add(FRAME_INTERVAL)
                        .filter(|next| *next > now)
                        .unwrap_or_else(|| now + FRAME_INTERVAL)
                } else {
                    // Idle frames are paced by the screens, not by this clock, so
                    // park the deadline one frame out rather than a whole idle
                    // timeout: that keeps the move into animation smooth.
                    now + FRAME_INTERVAL
                };
            }
        }
        Ok(())
    }

    /// Drains pending input, blocking for up to `timeout` on the first event.
    fn pump_events(&mut self, terminal: &mut Terminal, timeout: Duration) -> io::Result<bool> {
        let mut had_event = false;
        let mut wait = timeout;
        while let Some(event) = terminal.poll_event(wait)? {
            had_event = true;
            self.handle_event(event);
            wait = Duration::ZERO;
        }
        Ok(had_event)
    }

    /// Applies a single input event.
    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize { .. } => {
                self.refresh_cell_px();
                self.dirty = true;
            }
        }
    }

    /// Applies a key press.
    fn handle_key(&mut self, key: Key) {
        // Quitting is deliberate and global so it always works, regardless of
        // which screen is active. Releases must not trigger it.
        if key.is_down() && (key.is_ctrl('c') || key.is_char('q')) {
            self.running = false;
            return;
        }

        // While the viewport is too small the game is suspended, so the only
        // thing input can do is leave. Nothing is routed to the screens, which
        // means no blind movement and no state change.
        if !viewport::is_usable(self.frame.bounds()) {
            if key.is_down() && key.code == KeyCode::Esc {
                self.running = false;
            }
            return;
        }

        let outcome = self.stack.dispatch(key);
        self.dirty = true;
        if outcome.quit {
            self.running = false;
        }
        if outcome.changed && self.stack.wants_curtain() {
            self.transition
                .start(AppAnimation::Transition, transition::animation());
        }
    }

    /// Prints what the loop actually achieved when the development flag is set.
    ///
    /// The honest answer to "did it hit sixty?" is a measurement, not a claim, and
    /// this is the only place that can make it: the loop is the only thing that
    /// knows how often it drew, and how much it wrote.
    fn report_frame_stats(&self) {
        let wanted = matches!(
            std::env::var("GRIMMOIRE_FRAME_STATS").as_deref(),
            Ok("1") | Ok("true") | Ok("yes")
        );
        if !wanted {
            return;
        }
        let elapsed = self.started.elapsed().as_secs_f64().max(1e-6);
        eprintln!(
            "grimmoire: {} frames in {:.1}s = {:.1} frames/s, {:.1} KiB/s to the terminal",
            self.frames_presented,
            elapsed,
            self.frames_presented as f64 / elapsed,
            self.bytes_presented as f64 / elapsed / 1024.0,
        );
    }

    /// Records a presented frame and periodically works out the rate.
    ///
    /// A half-second window is long enough to be stable and short enough to react
    /// when the game starts or stops moving.
    fn count_frame(&mut self, now: Instant) {
        self.frame_count = self.frame_count.saturating_add(1);
        self.frames_presented = self.frames_presented.saturating_add(1);
        self.bytes_presented = self
            .bytes_presented
            .saturating_add(u64::from(self.stats.bytes));
        let elapsed = now.saturating_duration_since(self.frame_window);
        if elapsed >= Duration::from_millis(500) {
            self.frames_per_second = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.frame_window = now;
        }
    }

    /// Advances the clock by `delta`, then lets the active screen settle.
    fn advance(&mut self, delta: Duration) {
        self.stack.update(delta);
        self.transition.tick(delta);
        // A screen may have finished something on its own schedule, such as a
        // panel whose exit animation has just played out.
        let outcome = self.stack.settle();
        if outcome.quit {
            self.running = false;
        }
        if outcome.changed {
            self.dirty = true;
        }
    }

    /// Re-reads the terminal's cell size, which the pixel scene needs to keep its
    /// own pixels square.
    fn refresh_cell_px(&mut self) {
        self.cell_px = crossterm::terminal::window_size()
            .ok()
            .and_then(|size| {
                (size.columns > 0 && size.rows > 0)
                    .then(|| (size.width / size.columns, size.height / size.rows))
            })
            .filter(|(width, height)| *width > 0 && *height > 0);
        self.stack.on_cell_metrics(self.cell_px);
    }

    /// Resizes the frame buffer if the terminal size changed.
    fn set_size(&mut self, width: u16, height: u16) {
        if width == self.frame.width() && height == self.frame.height() {
            return;
        }
        self.frame.resize(width, height);
        self.stack.on_resize(self.frame.bounds());
        // A resize clears the screen, which clears images too, so the presenter
        // re-establishes both layers.
        self.presenter.invalidate();
        self.dirty = true;
    }

    /// Composes the current frame from the screen stack and the transition.
    fn compose_frame(&mut self) {
        self.frame.clear_with(self.palette.backdrop());

        // Too small to play in: suspend the interface rather than drawing a
        // mangled layout. Screens are not composed, so nothing can be left in a
        // half-drawn state, and all game state is untouched.
        if !viewport::is_usable(self.frame.bounds()) {
            let mut frame = Frame::begin(&mut self.frame, &mut self.frame_images);
            viewport::compose(&mut frame, &self.palette);
            return;
        }

        let context = FrameContext {
            capabilities: self.capabilities,
            palette: self.palette,
            graphics: self.graphics,
            art: self.art,
            stats: self.stats,
            frame_time: self.frame_time,
            animating: self.is_animating(),
            key_releases: self.key_releases,
            motion: self.settings.get().motion,
        };

        {
            let mut frame = Frame::begin(&mut self.frame, &mut self.frame_images);
            self.stack.compose(&mut frame, &context);
        }

        if self.transition.is_active(AppAnimation::Transition) {
            let progress = self.transition.value_or(AppAnimation::Transition, 1.0);
            transition::compose_curtain(&mut self.frame, progress, &self.palette);
        }
    }

    /// Whether anything needs continuous frames right now.
    fn is_animating(&self) -> bool {
        self.stack.is_animating() || self.transition.is_active(AppAnimation::Transition)
    }

    /// The composed frame. Exposed for tests and diagnostics.
    #[must_use]
    pub fn frame(&self) -> &Canvas {
        &self.frame
    }

    /// The number of screens on the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.depth()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::{App, Startup};
    use crate::input::Key;
    use crate::render::Color;
    use crate::save::SaveStore;
    use crate::terminal::Capabilities;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("grimmoire-app-{tag}-{unique}"))
    }

    fn fixture() -> App {
        let mut app = App::with_capabilities(Capabilities::conservative());
        app.set_size(120, 40);
        app.compose_frame();
        app
    }

    /// An application whose root is the world rather than the title screen.
    fn world_fixture() -> App {
        world_fixture_with(Startup::headless(Capabilities::conservative()))
    }

    fn world_fixture_with(startup: Startup) -> App {
        let world = crate::game::World::new(crate::game::areas::all(), crate::game::areas::CHAPEL);
        let save = startup.save.clone();
        let mut app = App::with_root(
            startup,
            Box::new(crate::ui::ExplorationScreen::new(world, save, None)),
        );
        app.set_size(120, 40);
        app.compose_frame();
        app
    }

    fn frame_text(app: &App) -> String {
        let canvas = app.frame();
        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn q_and_ctrl_c_quit_but_releases_do_not() {
        let mut app = fixture();
        app.handle_key(Key::char('q'));
        assert!(!app.running);

        let mut app = fixture();
        app.handle_key(Key::char('c').with_ctrl());
        assert!(!app.running);

        let mut app = fixture();
        app.handle_key(Key::char('q').released());
        assert!(app.running, "a released q must not quit");
    }

    #[test]
    fn the_application_opens_on_the_title_screen() {
        let mut startup = Startup::headless(Capabilities::conservative());
        // With motion off the title composes whole on its first frame, so the
        // menu can be read without waiting for the opening to play.
        startup.settings = crate::ui::Shared::new(crate::ui::Settings {
            motion: crate::ui::Motion::Off,
        });
        let mut app = App::new(startup);
        app.set_size(120, 40);
        app.compose_frame();
        assert_eq!("title", app.stack.active_name());
        let text = frame_text(&app);
        assert!(text.contains("New Game"), "the menu is missing:\n{text}");
        assert!(text.contains("Continue"), "{text}");
    }

    #[test]
    fn composing_the_world_produces_the_world() {
        // The world is reached through the title, so the world is what this
        // checks: the title's own composition is tested where it lives.
        let app = world_fixture();
        let text = frame_text(&app);
        assert!(text.contains("Ruined Chapel"), "area name missing");
        assert!(text.contains("(o:o)"), "the mage is missing");
        assert!(text.contains("facing"), "status missing");
    }

    #[test]
    fn resize_recomputes_the_frame_and_invalidates_the_presenter() {
        let mut app = fixture();
        app.dirty = false;
        app.set_size(80, 24);
        assert!(app.dirty);
        assert_eq!(80, app.frame().width());
        assert_eq!(24, app.frame().height());

        app.dirty = false;
        app.set_size(80, 24);
        assert!(!app.dirty, "an unchanged size should not force a redraw");
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        // Without graphics the cell layer must paint the whole frame, so the
        // terminal's own background can never show through.
        let mut app = fixture();
        for stage in ["game", "help"] {
            app.compose_frame();
            let canvas = app.frame();
            for y in 0..canvas.height() {
                for x in 0..canvas.width() {
                    assert_ne!(
                        Some(Color::Default),
                        canvas.get(x, y).map(|cell| cell.style.bg),
                        "{stage}: cell ({x}, {y}) would show the terminal background"
                    );
                }
            }
            app.handle_key(Key::char('h'));
        }
    }

    #[test]
    fn a_small_viewport_suspends_the_game_and_swallows_input() {
        let mut app = App::with_capabilities(Capabilities::conservative());
        app.set_size(60, 20);
        app.compose_frame();

        let text = frame_text(&app);
        assert!(text.contains("too small"), "expected the suspend screen");
        assert!(!text.contains("Ruined Chapel"), "the game must not draw");

        for _ in 0..4 {
            app.handle_key(Key::char('w'));
        }

        app.set_size(120, 40);
        app.compose_frame();
        let text = frame_text(&app);
        assert!(text.contains("Ruined Chapel"), "play must resume");
        assert!(
            text.contains("70,44"),
            "the player must not have moved while suspended"
        );
    }

    #[test]
    fn frames_are_only_asked_for_while_something_is_moving() {
        // Movement is event-driven and ambient scenery is slow, so the loop can
        // idle: no polling storm, no wasted CPU.
        let mut app = fixture();
        assert!(
            !app.is_animating(),
            "an idle game needs no continuous frames"
        );
        assert!(
            app.stack
                .next_frame()
                .is_some_and(|wait| wait > Duration::ZERO),
            "an idle game should still breathe slowly"
        );

        // A step plays a short stride, so it asks for frames - but only briefly.
        app.handle_key(Key::char('w'));
        assert!(app.is_animating(), "the stride needs frames");
        app.advance(Duration::from_millis(400));
        assert!(!app.is_animating(), "and then the game settles again");
    }

    #[test]
    fn a_screen_pushed_through_the_stack_composites_over_the_world() {
        let mut app = fixture();
        app.handle_key(Key::char('h'));
        app.advance(Duration::from_secs(1));
        app.compose_frame();
        let text = frame_text(&app);
        assert!(text.contains("help & diagnostics"));
        assert!(text.contains("Ruined Chapel"));
    }

    #[test]
    fn composition_is_deterministic_across_instances() {
        let a = fixture();
        let b = fixture();
        assert_eq!(frame_text(&a), frame_text(&b));
    }

    #[test]
    fn shutting_down_persists_the_game() {
        let directory = scratch("app-exit");
        let store = SaveStore::at(directory.join("save.txt"));
        let mut startup = Startup::headless(Capabilities::conservative());
        startup.save = Some(store.clone());
        let mut app = world_fixture_with(startup);
        app.set_size(120, 40);

        app.handle_key(Key::char('w'));
        app.stack.on_exit();

        let data = store.load().expect("progress should have been saved");
        assert_eq!("ruined-chapel", data.area);
        assert_eq!(43, data.y, "one step north of the spawn");

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn the_study_can_be_the_root_screen() {
        let mut app = App::with_root(
            Startup::headless(Capabilities::conservative()),
            Box::new(crate::ui::StageScreen::new()),
        );
        app.set_size(120, 40);
        app.advance(Duration::from_secs(1));
        app.compose_frame();
        assert!(frame_text(&app).contains("presentation study"));
        assert_eq!(1, app.depth());
    }

    #[test]
    fn a_resize_event_is_handled_without_touching_the_world() {
        let mut app = fixture();
        app.handle_event(crate::input::Event::Resize {
            width: 200,
            height: 60,
        });
        assert!(app.dirty);
        app.set_size(200, 60);
        app.compose_frame();
        assert!(frame_text(&app).contains("70,44"));
    }
}
