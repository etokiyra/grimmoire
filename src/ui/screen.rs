//! The screen trait and the stack that manages screens.
//!
//! Screens are self-contained: each handles its own input, advances its own
//! animations, and composes itself from its own state. The stack routes input to
//! the active screen and composes every screen from the bottom up, so a pushed
//! overlay draws over the screen beneath it rather than replacing it.

use std::time::Duration;

use crate::input::Key;
use crate::render::{Frame, Rect};

use super::FrameContext;

/// What a screen asks the stack to do after handling input.
///
/// Screens never mutate the stack directly; they return a command and the stack
/// applies it. That keeps the borrow checker happy and the control flow explicit.
pub enum Command {
    /// Stay on this screen.
    None,
    /// Quit the application.
    Quit,
    /// Push a new screen on top of this one.
    Push(Box<dyn Screen>),
    /// Return to the screen beneath this one.
    Pop,
}

/// A distinct mode of the application with its own input, state, and visuals.
pub trait Screen {
    /// A short, stable name used for diagnostics.
    fn name(&self) -> &'static str;

    /// Handles a key press, returning the command the stack should apply.
    fn handle_key(&mut self, _key: Key) -> Command {
        Command::None
    }

    /// Advances time-based state by `delta` since the previous update.
    fn update(&mut self, _delta: Duration) {}

    /// Composes this screen into `frame`.
    fn compose(&self, frame: &mut Frame, context: &FrameContext);

    /// Called when the terminal size changes, before the next compose.
    fn on_resize(&mut self, _area: Rect) {}

    /// Called once when the application is shutting down cleanly.
    ///
    /// This is where a screen persists anything it owns, because the stack (not
    /// the screen) decides when the application is finished.
    fn on_exit(&mut self) {}

    /// Called once after the terminal is entered, with what it reported.
    ///
    /// Screens that behave differently depending on the terminal's capabilities
    /// use this rather than reading the environment themselves.
    fn on_terminal_ready(&mut self, _key_releases: bool) {}

    /// Called when the terminal reports how large one character cell is.
    ///
    /// Screens that pace something in screen terms use this; the renderer itself
    /// does not care, because a cell is a cell.
    fn on_cell_metrics(&mut self, _cell_px: Option<(u16, u16)>) {}

    /// Whether this screen currently has continuous motion in progress.
    fn is_animating(&self) -> bool {
        false
    }

    /// How long until this screen wants to be composed again.
    ///
    /// `Some(ZERO)` means "as soon as possible", for continuous motion;
    /// `Some(d)` requests a frame in `d` for slow motion that is not worth a
    /// 60 Hz loop, such as ambient scenery; `None` means the screen is settled
    /// and needs nothing until input arrives. This is what lets a mostly idle
    /// game stay cheap without freezing its animation entirely.
    fn next_frame(&self) -> Option<Duration> {
        None
    }
}

/// The result of routing a key through the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcome {
    /// The application should quit.
    pub quit: bool,
    /// The active screen changed (a screen was pushed or popped).
    pub changed: bool,
}

/// An ordered stack of screens. The last screen is the active one.
#[derive(Default)]
pub struct ScreenStack {
    screens: Vec<Box<dyn Screen>>,
}

impl ScreenStack {
    /// Creates an empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a stack whose root is `root`.
    #[must_use]
    pub fn with_root(root: impl Screen + 'static) -> Self {
        ScreenStack {
            screens: vec![Box::new(root)],
        }
    }

    /// Pushes `screen` on top of the active screen.
    pub fn push(&mut self, screen: impl Screen + 'static) {
        self.screens.push(Box::new(screen));
    }

    /// Pushes an already boxed screen.
    pub fn push_boxed(&mut self, screen: Box<dyn Screen>) {
        self.screens.push(screen);
    }

    /// Pops the active screen, refusing to remove the last (root) screen.
    pub fn pop(&mut self) -> Option<Box<dyn Screen>> {
        if self.screens.len() > 1 {
            self.screens.pop()
        } else {
            None
        }
    }

    /// The number of screens on the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.screens.len()
    }

    /// Whether the stack holds no screens.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.screens.is_empty()
    }

    /// The name of the active screen, or `"(none)"` when empty.
    #[must_use]
    pub fn active_name(&self) -> &'static str {
        self.screens.last().map_or("(none)", |screen| screen.name())
    }

    /// Advances every screen, so background screens keep animating.
    pub fn update(&mut self, delta: Duration) {
        for screen in &mut self.screens {
            screen.update(delta);
        }
    }

    /// Composes every screen from the bottom up; later screens draw on top.
    pub fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        for screen in &self.screens {
            screen.compose(frame, context);
        }
    }

    /// Notifies every screen that the terminal size changed.
    pub fn on_resize(&mut self, area: Rect) {
        for screen in &mut self.screens {
            screen.on_resize(area);
        }
    }

    /// Notifies every screen that the application is shutting down cleanly.
    pub fn on_exit(&mut self) {
        for screen in &mut self.screens {
            screen.on_exit();
        }
    }

    /// Notifies every screen of what the entered terminal reported.
    pub fn on_terminal_ready(&mut self, key_releases: bool) {
        for screen in &mut self.screens {
            screen.on_terminal_ready(key_releases);
        }
    }

    /// Notifies every screen of the terminal's cell size.
    pub fn on_cell_metrics(&mut self, cell_px: Option<(u16, u16)>) {
        for screen in &mut self.screens {
            screen.on_cell_metrics(cell_px);
        }
    }

    /// Whether any screen has continuous motion in progress.
    #[must_use]
    pub fn is_animating(&self) -> bool {
        self.screens.iter().any(|screen| screen.is_animating())
    }

    /// The soonest any screen wants another frame.
    #[must_use]
    pub fn next_frame(&self) -> Option<Duration> {
        self.screens
            .iter()
            .filter_map(|screen| screen.next_frame())
            .min()
    }

    /// Routes `key` to the active screen and applies the returned command.
    pub fn dispatch(&mut self, key: Key) -> Outcome {
        let command = match self.screens.last_mut() {
            Some(screen) => screen.handle_key(key),
            None => {
                return Outcome {
                    quit: true,
                    changed: false,
                };
            }
        };

        match command {
            Command::None => Outcome::default(),
            Command::Quit => Outcome {
                quit: true,
                changed: false,
            },
            Command::Push(screen) => {
                self.screens.push(screen);
                Outcome {
                    quit: false,
                    changed: true,
                }
            }
            Command::Pop => {
                if self.screens.len() > 1 {
                    self.screens.pop();
                    Outcome {
                        quit: false,
                        changed: true,
                    }
                } else {
                    // Popping the root is interpreted as quitting.
                    Outcome {
                        quit: true,
                        changed: false,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use super::{Command, Screen, ScreenStack};
    use crate::input::{Key, KeyCode};
    use crate::render::{Canvas, Frame, Rect};
    use crate::terminal::{Capabilities, PresentStats};
    use crate::ui::{FrameContext, Palette};

    #[derive(Clone, Default)]
    struct Log(Rc<RefCell<Vec<String>>>);

    impl Log {
        fn push(&self, entry: impl Into<String>) {
            self.0.borrow_mut().push(entry.into());
        }

        fn entries(&self) -> Vec<String> {
            self.0.borrow().clone()
        }
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Stay,
        Quit,
        Pop,
        Push(&'static str),
    }

    struct FakeScreen {
        name: &'static str,
        log: Log,
        mode: Mode,
    }

    impl Screen for FakeScreen {
        fn name(&self) -> &'static str {
            self.name
        }

        fn handle_key(&mut self, key: Key) -> Command {
            self.log.push(format!("{}:key:{:?}", self.name, key.code));
            match self.mode {
                Mode::Stay => Command::None,
                Mode::Quit => Command::Quit,
                Mode::Pop => Command::Pop,
                Mode::Push(child) => Command::Push(Box::new(FakeScreen {
                    name: child,
                    log: self.log.clone(),
                    mode: Mode::Stay,
                })),
            }
        }

        fn update(&mut self, _delta: Duration) {
            self.log.push(format!("{}:update", self.name));
        }

        fn compose(&self, _frame: &mut Frame, _context: &FrameContext) {
            self.log.push(format!("{}:compose", self.name));
        }

        fn on_exit(&mut self) {
            self.log.push(format!("{}:exit", self.name));
        }
    }

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            graphics: crate::terminal::GraphicsSupport::Forced,
            art: crate::ui::art::Art::none(),
            key_releases: true,
            palette: Palette::BASIC,
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
        }
    }

    fn stack_with(mode: Mode) -> (ScreenStack, Log) {
        let log = Log::default();
        let stack = ScreenStack::with_root(FakeScreen {
            name: "root",
            log: log.clone(),
            mode,
        });
        (stack, log)
    }

    #[test]
    fn input_is_routed_to_the_active_screen_only() {
        let (mut stack, log) = stack_with(Mode::Stay);
        stack.push(FakeScreen {
            name: "child",
            log: log.clone(),
            mode: Mode::Stay,
        });
        stack.dispatch(Key::char('a'));
        stack.dispatch(Key::char('b'));

        let entries = log.entries();
        assert_eq!(
            0,
            entries.iter().filter(|e| e.starts_with("root:key")).count()
        );
        assert_eq!(
            2,
            entries
                .iter()
                .filter(|e| e.starts_with("child:key"))
                .count()
        );
        assert_eq!("child", stack.active_name());
        assert_eq!(2, stack.depth());
    }

    #[test]
    fn a_key_that_pushes_is_handled_by_the_parent_first() {
        let (mut stack, log) = stack_with(Mode::Push("child"));
        stack.dispatch(Key::char('a'));
        stack.dispatch(Key::char('b'));

        let entries = log.entries();
        assert_eq!(
            1,
            entries.iter().filter(|e| e.starts_with("root:key")).count()
        );
        assert_eq!(
            1,
            entries
                .iter()
                .filter(|e| e.starts_with("child:key"))
                .count()
        );
        assert_eq!("child", stack.active_name());
    }

    #[test]
    fn push_reports_a_change_and_pop_returns_to_the_parent() {
        let (mut stack, _log) = stack_with(Mode::Stay);
        stack.push(FakeScreen {
            name: "child",
            log: Log::default(),
            mode: Mode::Pop,
        });

        let outcome = stack.dispatch(Key::new(KeyCode::Esc));
        assert!(outcome.changed);
        assert!(!outcome.quit);
        assert_eq!("root", stack.active_name());
        assert_eq!(1, stack.depth());
    }

    #[test]
    fn popping_the_root_quits() {
        let (mut stack, _log) = stack_with(Mode::Pop);
        let outcome = stack.dispatch(Key::new(KeyCode::Esc));
        assert!(outcome.quit);
        assert!(!outcome.changed);
    }

    #[test]
    fn quit_command_is_propagated() {
        let (mut stack, _log) = stack_with(Mode::Quit);
        assert!(stack.dispatch(Key::char('q')).quit);
    }

    #[test]
    fn pop_refuses_to_remove_the_root() {
        let (mut stack, _log) = stack_with(Mode::Stay);
        assert!(stack.pop().is_none());
        assert_eq!(1, stack.depth());
    }

    #[test]
    fn compose_runs_bottom_to_top() {
        let (mut stack, log) = stack_with(Mode::Stay);
        stack.push(FakeScreen {
            name: "child",
            log: log.clone(),
            mode: Mode::Stay,
        });

        let mut canvas = Canvas::new(20, 5);
        let mut images = Vec::new();
        let mut frame = Frame::begin(&mut canvas, &mut images);
        stack.compose(&mut frame, &context());

        let composes: Vec<String> = log
            .entries()
            .into_iter()
            .filter(|entry| entry.ends_with(":compose"))
            .collect();
        assert_eq!(
            vec!["root:compose".to_string(), "child:compose".to_string()],
            composes
        );
    }

    #[test]
    fn update_reaches_every_screen() {
        let (mut stack, log) = stack_with(Mode::Stay);
        stack.push(FakeScreen {
            name: "child",
            log: log.clone(),
            mode: Mode::Stay,
        });
        stack.update(Duration::from_millis(16));

        let updates = log
            .entries()
            .into_iter()
            .filter(|e| e.ends_with(":update"))
            .count();
        assert_eq!(2, updates);
    }

    #[test]
    fn an_empty_stack_quits_on_input() {
        let mut stack = ScreenStack::new();
        assert!(stack.is_empty());
        assert!(stack.dispatch(Key::char('a')).quit);
        assert_eq!("(none)", stack.active_name());
    }

    #[test]
    fn resize_reaches_every_screen_without_panicking() {
        let (mut stack, _log) = stack_with(Mode::Stay);
        stack.on_resize(Rect::from_size(80, 24));
    }

    #[test]
    fn exit_reaches_every_screen_once() {
        let (mut stack, log) = stack_with(Mode::Stay);
        stack.push(FakeScreen {
            name: "child",
            log: log.clone(),
            mode: Mode::Stay,
        });
        stack.on_exit();

        let exits: Vec<String> = log
            .entries()
            .into_iter()
            .filter(|entry| entry.ends_with(":exit"))
            .collect();
        assert_eq!(
            vec!["root:exit".to_string(), "child:exit".to_string()],
            exits
        );
    }
}
