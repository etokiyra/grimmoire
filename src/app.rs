//! The application coordinator: input, update, compose, present.

use std::io;
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::render::Canvas;
use crate::terminal::TerminalBackend;
use crate::ui::{self, TitleScreen};

/// How long the loop waits for input before composing another frame.
///
/// The short timeout keeps input responsive today and gives future animation a
/// natural tick to run against.
const FRAME_TICK: Duration = Duration::from_millis(50);

/// Top-level application coordinator.
///
/// Owns the interface state and drives the classic loop: wait for input, update,
/// compose a frame, present it. It knows nothing about escape sequences (the
/// backend's job) and keeps drawing decisions in [`crate::ui`]. Game state
/// (see [`crate::game`]) will be owned here once it carries data.
#[derive(Debug)]
pub struct App {
    title: TitleScreen,
    running: bool,
}

impl App {
    /// Creates an application ready to run.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs until the user quits, presenting frames to `backend`.
    ///
    /// The terminal is entered and left here, so the loop restores it on both
    /// the success and the error path.
    pub fn run(&mut self, backend: &mut TerminalBackend) -> io::Result<()> {
        backend.enter()?;

        while self.running {
            if let Some(event) = backend.poll_event(FRAME_TICK)? {
                self.handle_event(event);
            }

            let (width, height) = backend.size()?;
            let mut canvas = Canvas::new(width, height);
            self.compose(&mut canvas);
            backend.present(&canvas)?;
        }

        backend.leave()
    }

    /// Applies a single terminal event.
    fn handle_event(&mut self, event: Event) {
        if let Event::Key(key) = event {
            // Key releases (reported by some terminals) must not double-fire.
            if key.kind == KeyEventKind::Press {
                self.handle_key(key);
            }
        }
    }

    /// Applies a single key press.
    fn handle_key(&mut self, key: KeyEvent) {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.running = false;
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.running = false,
            KeyCode::Up => self.title.select_previous(),
            KeyCode::Down => self.title.select_next(),
            KeyCode::Enter => self.activate_selection(),
            _ => {}
        }
    }

    /// Activates the highlighted title-screen entry, quitting if it asks to.
    fn activate_selection(&mut self) {
        if self.title.activate() {
            self.running = false;
        }
    }

    /// Composes the current frame.
    fn compose(&self, canvas: &mut Canvas) {
        ui::title::compose(canvas, &self.title);
    }
}

impl Default for App {
    fn default() -> Self {
        App {
            title: TitleScreen::new(),
            running: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::render::Canvas;
    use crate::ui::title;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn q_requests_exit() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Char('q')));
        assert!(!app.running);
    }

    #[test]
    fn escape_requests_exit() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Esc));
        assert!(!app.running);
    }

    #[test]
    fn arrow_keys_move_the_highlight_without_quitting() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Down));
        assert_eq!(1, app.title.selected());
        app.handle_key(press(KeyCode::Up));
        assert_eq!(0, app.title.selected());
        assert!(app.running);
    }

    #[test]
    fn ctrl_c_quits_but_plain_c_does_not() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Char('c')));
        assert!(app.running);

        app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.running);
    }

    #[test]
    fn enter_on_quit_requests_exit() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Down));
        app.handle_key(press(KeyCode::Enter));
        assert!(!app.running);
    }

    #[test]
    fn enter_on_unfinished_entry_keeps_running_and_records_status() {
        let mut app = App::new();
        app.handle_key(press(KeyCode::Enter));
        assert!(app.running);
        assert!(app.title.status().is_some());
    }

    #[test]
    fn compose_renders_the_title_screen_into_the_frame() {
        let app = App::new();
        let mut canvas = Canvas::new(80, 24);
        app.compose(&mut canvas);

        // The banner's first ink marker lands where the title screen places it.
        assert_eq!(Some('_'), canvas.get(16, 2).map(|cell| cell.ch));

        // A freshly composed canvas equals one composed directly by the UI.
        let mut expected = Canvas::new(80, 24);
        title::compose(&mut expected, &app.title);
        for y in 0..24 {
            for x in 0..80 {
                assert_eq!(canvas.get(x, y), expected.get(x, y), "cell ({x}, {y})");
            }
        }
    }
}
