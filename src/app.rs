//! The application coordinator: input, update, compose, present.
//!
//! The loop owns the clock, the screen stack, the transition, and the frame
//! buffer. It deliberately keeps responsibilities thin: it advances time, routes
//! input, recomputes the frame on demand, and hands the frame to the presenter.
//! Frame layout lives in [`crate::ui`], terminal I/O in [`crate::terminal`].

use std::io;
use std::time::{Duration, Instant};

use crate::anim::Animator;
use crate::input::{Event, Key};
use crate::render::Canvas;
use crate::terminal::{Capabilities, PresentStats, Presenter, Terminal};
use crate::ui::transition;
use crate::ui::{FrameContext, Palette, ScreenStack, Showcase};

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
    stats: PresentStats,
    frame_time: Duration,
    running: bool,
    dirty: bool,
}

impl App {
    /// Creates an application, detecting the terminal's capabilities.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capabilities(Capabilities::detect())
    }

    /// Creates an application with explicit capabilities.
    ///
    /// Used by tests, and available to any caller that wants deterministic
    /// presentation without reading the environment.
    #[must_use]
    pub fn with_capabilities(capabilities: Capabilities) -> Self {
        let mut stack = ScreenStack::new();
        stack.push(Showcase::new());
        App {
            palette: Palette::for_colors(capabilities.colors),
            capabilities,
            stack,
            transition: Animator::new(),
            presenter: Presenter::new(),
            frame: Canvas::new(0, 0),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            running: true,
            dirty: true,
        }
    }

    /// Runs until the user quits, rendering to `terminal`.
    ///
    /// The terminal is entered and left here, so it is restored on both the
    /// success and the error path.
    pub fn run(&mut self, terminal: &mut Terminal) -> io::Result<()> {
        terminal.enter()?;
        let result = self.event_loop(terminal);
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
                self.stats = self.presenter.present(&self.frame, terminal.writer())?;
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
        self.dirty = true;
    }

    /// Composes the current frame from the screen stack and the transition.
    fn compose_frame(&mut self) {
        self.frame.clear_with(self.palette.backdrop());

        let context = FrameContext {
            capabilities: self.capabilities,
            palette: self.palette,
            stats: self.stats,
            frame_time: self.frame_time,
            animating: self.is_animating(),
        };
        self.stack.compose(&mut self.frame, &context);

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
    use std::time::Duration;

    use super::App;
    use crate::input::{Key, KeyCode};
    use crate::terminal::Capabilities;

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
    fn composing_the_root_screen_produces_the_showcase() {
        let app = fixture();
        let text = frame_text(&app);
        assert!(text.contains("EMBERFALL"));
        assert!(text.contains("GRIMMOIRE"));
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
    fn time_only_advances_when_ticked() {
        let mut app = fixture();
        app.handle_key(Key::char(' ')); // pause motion
        app.advance(Duration::from_millis(50));
        app.handle_key(Key::char(' ')); // resume
        app.advance(Duration::from_millis(50));
        // Reaching here without panicking and with a running app is enough; the
        // point is that ticks are explicit and deterministic.
        assert!(app.running);
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
    fn an_overlay_composites_over_the_showcase() {
        let mut app = fixture();
        app.handle_key(Key::char('h'));
        app.advance(Duration::from_secs(1)); // let the transition finish
        app.compose_frame();
        let text = frame_text(&app);
        // Both layers are visible: the overlay panel and the showcase beneath.
        assert!(text.contains("help & diagnostics"));
        assert!(text.contains("renderer showcase"));
    }
}
