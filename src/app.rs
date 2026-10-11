//! The application coordinator: input, update, compose, present.
//!
//! The loop owns the clock, the screen stack, the transition, and the frame
//! buffer. It deliberately keeps responsibilities thin: it advances time, routes
//! input, recomposes the frame on demand, and hands the frame to the presenter.
//! Frame layout lives in [`crate::ui`], terminal I/O in [`crate::terminal`].
//!
//! # When frames happen
//!
//! Screens report how soon they want another frame. Continuous motion (a step
//! being interpolated, a transition) asks for frames at the animation rate; slow
//! ambient scenery asks for one every few hundred milliseconds; a settled screen
//! asks for none and the loop blocks on input. An idle game therefore costs
//! almost nothing while still breathing.

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
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// How long to wait for input while nothing is animating.
const IDLE_TIMEOUT: Duration = Duration::from_millis(200);
/// Upper bound on a single time step, so a stall cannot make movement jump.
const MAX_DELTA: Duration = Duration::from_millis(60);

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
    key_releases: bool,
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
        let screen = ExplorationScreen::from_save(save, portrait);
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
            palette: Palette::for_colors(startup.capabilities.colors),
            stack,
            transition: Animator::new(),
            presenter: Presenter::with_graphics(startup.graphics),
            frame: Canvas::new(0, 0),
            frame_images: Vec::new(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
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
        result.and(left)
    }

    /// The main loop: pump input, advance time, recompose when needed, present.
    fn event_loop(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        let mut last = Instant::now();
        let mut last_frame = Instant::now();
        while self.running {
            let continuous = self.is_animating();
            let timeout = if continuous {
                FRAME_INTERVAL
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
            let frame_due =
                continuous || self.stack.next_frame().is_some_and(|wait| wait <= elapsed);

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
        if outcome.changed {
            self.transition
                .start(AppAnimation::Transition, transition::animation());
        }
    }

    /// Advances the clock by `delta`.
    fn advance(&mut self, delta: Duration) {
        self.stack.update(delta);
        self.transition.tick(delta);
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
    fn composing_the_root_screen_produces_the_world() {
        let app = fixture();
        let text = frame_text(&app);
        assert!(text.contains("Ruined Chapel"), "area name missing");
        assert!(text.contains("(o)"), "player missing");
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

        app.handle_key(Key::char('w'));
        assert!(!app.is_animating(), "a step is instant, not a stream");
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
        let mut app = App::new(startup);
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
