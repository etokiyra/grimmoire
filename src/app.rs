//! The application coordinator: input, update, compose, present.
//!
//! The loop owns the clock, the screen stack, the transition, and the frame
//! buffer. It deliberately keeps responsibilities thin: it advances time, routes
//! input, recomputes the frame on demand, and hands the frame to the presenter.
//! Frame layout lives in [`crate::ui`], terminal I/O in [`crate::terminal`].

use std::io;
use std::time::{Duration, Instant};

use crate::anim::Animator;
use crate::input::{Event, Key, KeyCode};
use crate::render::{Canvas, Frame, ImagePlacement};
use crate::save::SaveStore;
use crate::terminal::{Capabilities, CellMetrics, ImageBackend, PresentStats, Presenter, Terminal};
use crate::ui::transition;
use crate::ui::{ExplorationScreen, FrameContext, Palette, Screen, ScreenStack, viewport};

/// The interval between frames while something is animating.
const FRAME_INTERVAL: Duration = Duration::from_millis(33);
/// How long to wait for input while nothing is animating.
const IDLE_TIMEOUT: Duration = Duration::from_millis(200);
/// Upper bound on a single time step, so a stall cannot make animations jump.
const MAX_DELTA: Duration = Duration::from_millis(100);

/// Animations owned by the application rather than by a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppAnimation {
    /// The curtain shown when the screen stack changes.
    Transition,
}

/// The top-level application.
pub struct App {
    capabilities: Capabilities,
    palette: Palette,
    stack: ScreenStack,
    transition: Animator<AppAnimation>,
    presenter: Presenter,
    frame: Canvas,
    frame_images: Vec<ImagePlacement>,
    image_backend: ImageBackend,
    cell_metrics: Option<CellMetrics>,
    stats: PresentStats,
    frame_time: Duration,
    running: bool,
    dirty: bool,
}

impl App {
    /// Creates the application: detects the terminal and loads any saved game.
    ///
    /// Persistence is skipped entirely when no conventional save location can be
    /// found, so an unusual environment simply starts a new game.
    #[must_use]
    pub fn new() -> Self {
        Self::with_save(Capabilities::detect(), SaveStore::default_location())
    }

    /// Creates an application with explicit capabilities and no persistence.
    ///
    /// Used by tests, and available to any caller that wants deterministic
    /// behaviour without reading the environment or the filesystem.
    #[must_use]
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        Self::with_save(capabilities, None)
    }

    /// Creates an application with explicit capabilities and an optional save.
    ///
    /// This launches directly into the playable exploration prototype; the
    /// renderer showcase is reachable from within it as a development screen.
    #[must_use]
    pub fn with_save(capabilities: Capabilities, save: Option<SaveStore>) -> Self {
        Self::with_root(capabilities, Box::new(ExplorationScreen::from_save(save)))
    }

    /// Creates an application with an explicit root screen.
    ///
    /// Used to launch the presentation study directly, and available to any
    /// caller that wants to start somewhere other than the game.
    #[must_use]
    pub fn with_root(capabilities: Capabilities, root: Box<dyn Screen>) -> Self {
        let mut stack = ScreenStack::new();
        stack.push_boxed(root);
        App {
            palette: Palette::for_colors(capabilities.colors),
            capabilities,
            stack,
            transition: Animator::new(),
            presenter: Presenter::new(),
            frame: Canvas::new(0, 0),
            frame_images: Vec::new(),
            image_backend: ImageBackend::select(capabilities.graphics, capabilities.cell_metrics),
            cell_metrics: capabilities.cell_metrics,
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
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
        let result = self.event_loop(terminal);
        self.stack.on_exit();
        let left = terminal.leave();
        result.and(left)
    }

    /// The main loop: pump input, advance time, recompose when needed, present.
    fn event_loop(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        let mut last = Instant::now();
        while self.running {
            let animating = self.is_animating();
            let timeout = if animating {
                FRAME_INTERVAL
            } else {
                IDLE_TIMEOUT
            };
            let had_event = self.pump_events(terminal, timeout)?;

            let now = Instant::now();
            let delta = now.saturating_duration_since(last).min(MAX_DELTA);
            last = now;
            self.frame_time = delta;
            self.advance(delta);

            let (width, height) = terminal.size()?;
            self.set_size(width, height);

            if self.dirty || had_event || animating {
                self.compose_frame();
                self.stats = self.presenter.present_with(
                    &self.frame,
                    &self.frame_images,
                    self.image_backend,
                    self.cell_metrics,
                    terminal.writer(),
                )?;
                self.dirty = false;
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
            Event::Resize { .. } => self.dirty = true,
        }
    }

    /// Applies a key press.
    fn handle_key(&mut self, key: Key) {
        // Quitting is deliberate and global so it always works, regardless of
        // which screen is active.
        if key.is_ctrl('c') || key.is_char('q') {
            self.running = false;
            return;
        }

        // While the viewport is too small the game is suspended, so the only
        // thing input can do is leave. Nothing is routed to the screens, which
        // means no blind movement and no state change.
        if !viewport::is_usable(self.frame.bounds()) {
            if key.code == KeyCode::Esc {
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

    /// Resizes the frame buffer if the terminal size changed.
    fn set_size(&mut self, width: u16, height: u16) {
        if width == self.frame.width() && height == self.frame.height() {
            return;
        }
        self.frame.resize(width, height);
        self.stack.on_resize(self.frame.bounds());
        self.presenter.invalidate();
        self.refresh_cell_metrics();
        self.dirty = true;
    }

    /// Re-reads the character cell size, which can change when the font does.
    fn refresh_cell_metrics(&mut self) {
        self.cell_metrics = CellMetrics::detect();
        self.capabilities.cell_metrics = self.cell_metrics;
        self.image_backend = ImageBackend::select(self.capabilities.graphics, self.cell_metrics);
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
            image_backend: self.image_backend,
            stats: self.stats,
            frame_time: self.frame_time,
            animating: self.is_animating(),
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

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::App;
    use crate::input::{Key, KeyCode};
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
        app.set_size(100, 30);
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
    fn q_and_ctrl_c_quit() {
        let mut app = fixture();
        app.handle_key(Key::char('q'));
        assert!(!app.running);

        let mut app = fixture();
        app.handle_key(Key::char('c').with_ctrl());
        assert!(!app.running);
    }

    #[test]
    fn plain_c_does_not_quit() {
        let mut app = fixture();
        app.handle_key(Key::char('c'));
        assert!(app.running);
    }

    #[test]
    fn help_is_pushed_and_popped() {
        let mut app = fixture();
        assert_eq!(1, app.depth());

        app.handle_key(Key::char('h'));
        assert_eq!(2, app.depth());
        assert!(app.transition.is_active(super::AppAnimation::Transition));

        // Input now routes to the overlay, which closes on Escape.
        app.handle_key(Key::new(KeyCode::Esc));
        assert_eq!(1, app.depth());
        assert!(app.running);

        // Escape at the root quits.
        app.handle_key(Key::new(KeyCode::Esc));
        assert!(!app.running);
    }

    #[test]
    fn composing_the_root_screen_produces_the_world() {
        let app = fixture();
        let text = frame_text(&app);
        assert!(text.contains("Ruined Chapel"), "area name missing");
        assert!(text.contains('@'), "player missing");
    }

    #[test]
    fn resizing_marks_the_frame_for_redraw() {
        let mut app = fixture();
        app.dirty = false;
        app.set_size(60, 16);
        assert!(app.dirty);
        assert_eq!(60, app.frame().width());
        assert_eq!(16, app.frame().height());

        app.dirty = false;
        app.set_size(60, 16);
        assert!(!app.dirty, "an unchanged size should not force a redraw");
    }

    #[test]
    fn the_loop_asks_for_frames_only_while_something_is_animating() {
        let mut app = fixture();
        assert!(
            !app.is_animating(),
            "an idle game should not ask for frames"
        );

        app.handle_key(Key::char('w'));
        assert!(app.is_animating(), "a step should keep the loop ticking");

        app.advance(Duration::from_millis(200));
        assert!(!app.is_animating(), "the step should settle by now");
    }

    #[test]
    fn the_transition_finishes_after_its_duration() {
        let mut app = fixture();
        app.handle_key(Key::char('h'));
        assert!(app.transition.is_active(super::AppAnimation::Transition));

        app.advance(crate::ui::transition::DURATION + Duration::from_millis(1));
        assert!(!app.transition.is_active(super::AppAnimation::Transition));
    }

    #[test]
    fn composition_is_deterministic_across_instances() {
        let a = fixture();
        let b = fixture();
        assert_eq!(frame_text(&a), frame_text(&b));
    }

    #[test]
    fn an_overlay_composites_over_the_world() {
        let mut app = fixture();
        app.handle_key(Key::char('h'));
        app.advance(Duration::from_secs(1)); // let the transition finish
        app.compose_frame();
        let text = frame_text(&app);
        // Both layers are visible: the overlay panel and the world beneath it.
        assert!(text.contains("help & diagnostics"));
        assert!(text.contains("Ruined Chapel"));
    }

    #[test]
    fn shutting_down_persists_the_game() {
        let directory = scratch("app-exit");
        let store = SaveStore::at(directory.join("save.txt"));
        let mut app = App::with_save(Capabilities::conservative(), Some(store.clone()));
        app.set_size(80, 24);

        app.handle_key(Key::char('w'));
        app.stack.on_exit();

        let data = store.load().expect("progress should have been saved");
        assert_eq!("ruined-chapel", data.area);
        assert_eq!(11, data.y);

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
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

        // Movement while suspended must do nothing at all.
        for _ in 0..4 {
            app.handle_key(Key::char('w'));
        }

        app.set_size(120, 32);
        app.compose_frame();
        let text = frame_text(&app);
        assert!(text.contains("Ruined Chapel"), "play must resume");
        assert!(
            text.contains("23, 12"),
            "the player must not have moved while suspended"
        );
    }

    #[test]
    fn the_study_can_be_the_root_screen() {
        let mut app = App::with_root(
            Capabilities::conservative(),
            Box::new(crate::ui::StageScreen::new()),
        );
        app.set_size(100, 30);
        app.advance(Duration::from_secs(1)); // let the opening reveal finish
        app.compose_frame();
        let text = frame_text(&app);
        assert!(text.contains("presentation study"));
        assert_eq!(1, app.depth());
    }
}
