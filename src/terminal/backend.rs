//! The concrete terminal implementation, backed by `crossterm`.

use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, Event as TerminalEvent, KeyCode as TerminalKeyCode, KeyEvent, KeyEventKind,
    KeyModifiers as TerminalKeyModifiers,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

use crate::input::{Event, Key, KeyCode, KeyModifiers};

/// Owns the process's terminal state and delivers terminal-independent events.
///
/// The terminal is the single owner of raw mode and the alternate screen. Frame
/// output is written through [`Terminal::writer`] by a
/// [`crate::terminal::Presenter`], which keeps escape sequences in one layer.
///
/// Terminal restoration is layered: [`leave`](Terminal::leave) is idempotent and
/// called on the normal exit path, [`Drop`] restores the terminal if the owner is
/// dropped early (including while unwinding from a panic), and
/// [`install_panic_hook`] restores it before the panic message is printed.
///
/// This does **not** guarantee restoration against forced termination such as
/// `SIGKILL` or a power loss; no user-space program can.
pub struct Terminal {
    stdout: Stdout,
    active: bool,
}

impl Terminal {
    /// Creates a terminal handle without changing terminal state yet.
    pub fn new() -> io::Result<Self> {
        Ok(Terminal {
            stdout: io::stdout(),
            active: false,
        })
    }

    /// Switches to the alternate screen with raw keyboard input enabled.
    ///
    /// Calling this when already active is a no-op, so ownership stays clear.
    pub fn enter(&mut self) -> io::Result<()> {
        if self.active {
            return Ok(());
        }
        enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen, Hide)?;
        self.active = true;
        Ok(())
    }

    /// Restores the terminal to the state it was in before [`Terminal::enter`].
    ///
    /// Idempotent, so calling it twice is harmless.
    pub fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let restored = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        restored
    }

    /// The current terminal size as `(columns, rows)`.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }

    /// A mutable handle to the output stream, for the presenter to write to.
    pub fn writer(&mut self) -> &mut Stdout {
        &mut self.stdout
    }

    /// Waits up to `timeout` for a supported input event.
    ///
    /// Events the application does not use (mouse, paste, key releases) are
    /// skipped rather than surfaced. Returns `Ok(None)` when the timeout expires,
    /// which lets the caller keep animating while remaining input-driven.
    pub fn poll_event(&self, timeout: Duration) -> io::Result<Option<Event>> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !event::poll(remaining)? {
                return Ok(None);
            }
            if let Some(mapped) = translate(event::read()?) {
                return Ok(Some(mapped));
            }
        }
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        // Best effort: Drop cannot report failures, and `leave` is idempotent.
        let _ = self.leave();
    }
}

/// Installs a panic hook that restores the terminal before the panic message is
/// printed, so a crash never strands the user inside the alternate screen.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        previous(info);
    }));
}

/// Translates a terminal library event, dropping events the app does not use.
fn translate(event: TerminalEvent) -> Option<Event> {
    match event {
        TerminalEvent::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
        {
            map_key(key).map(Event::Key)
        }
        TerminalEvent::Resize(width, height) => Some(Event::Resize { width, height }),
        _ => None,
    }
}

/// Maps a terminal key event onto the app's key model.
fn map_key(key: KeyEvent) -> Option<Key> {
    let code = match key.code {
        TerminalKeyCode::Char(ch) => KeyCode::Char(ch),
        TerminalKeyCode::Enter => KeyCode::Enter,
        TerminalKeyCode::Esc => KeyCode::Esc,
        TerminalKeyCode::Backspace => KeyCode::Backspace,
        TerminalKeyCode::Tab | TerminalKeyCode::BackTab => KeyCode::Tab,
        TerminalKeyCode::Delete => KeyCode::Delete,
        TerminalKeyCode::Insert => KeyCode::Insert,
        TerminalKeyCode::Home => KeyCode::Home,
        TerminalKeyCode::End => KeyCode::End,
        TerminalKeyCode::PageUp => KeyCode::PageUp,
        TerminalKeyCode::PageDown => KeyCode::PageDown,
        TerminalKeyCode::Up => KeyCode::Up,
        TerminalKeyCode::Down => KeyCode::Down,
        TerminalKeyCode::Left => KeyCode::Left,
        TerminalKeyCode::Right => KeyCode::Right,
        TerminalKeyCode::F(number) => KeyCode::Function(number),
        _ => return None,
    };

    Some(Key {
        code,
        modifiers: KeyModifiers {
            ctrl: key.modifiers.contains(TerminalKeyModifiers::CONTROL),
            alt: key.modifiers.contains(TerminalKeyModifiers::ALT),
            shift: key.modifiers.contains(TerminalKeyModifiers::SHIFT),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{TerminalEvent, map_key, translate};
    use crate::input::{Event, Key, KeyCode};
    use crossterm::event::{
        KeyCode as TerminalKeyCode, KeyEvent, KeyEventKind, KeyModifiers as TerminalKeyModifiers,
    };

    fn key(code: TerminalKeyCode, modifiers: TerminalKeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    #[test]
    fn character_keys_keep_modifiers() {
        let mapped = map_key(key(
            TerminalKeyCode::Char('c'),
            TerminalKeyModifiers::CONTROL,
        ))
        .expect("mapped");
        assert_eq!(KeyCode::Char('c'), mapped.code);
        assert!(mapped.modifiers.ctrl);
    }

    #[test]
    fn navigation_keys_are_mapped() {
        assert_eq!(
            Some(KeyCode::Up),
            map_key(key(TerminalKeyCode::Up, TerminalKeyModifiers::NONE)).map(|k| k.code)
        );
        assert_eq!(
            Some(KeyCode::Function(5)),
            map_key(key(TerminalKeyCode::F(5), TerminalKeyModifiers::NONE)).map(|k| k.code)
        );
    }

    #[test]
    fn unused_keys_are_dropped() {
        assert_eq!(
            None,
            map_key(key(TerminalKeyCode::CapsLock, TerminalKeyModifiers::NONE))
        );
    }

    #[test]
    fn resize_events_pass_through() {
        assert_eq!(
            Some(Event::Resize {
                width: 80,
                height: 24
            }),
            translate(TerminalEvent::Resize(80, 24))
        );
    }

    #[test]
    fn key_release_events_are_ignored() {
        let mut release = key(TerminalKeyCode::Char('a'), TerminalKeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(None, translate(TerminalEvent::Key(release)));
    }

    #[test]
    fn presses_become_key_events() {
        assert_eq!(
            Some(Event::Key(Key::char('a'))),
            translate(TerminalEvent::Key(key(
                TerminalKeyCode::Char('a'),
                TerminalKeyModifiers::NONE
            )))
        );
    }
}
