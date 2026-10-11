//! The concrete crossterm-backed terminal implementation.

use std::io::{self, Stdout, Write};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event};
use crossterm::style::{
    Attribute, Color as TerminalColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

use crate::render::{Canvas, Color, Style};

/// Owns the process's terminal state and turns composed frames into output.
///
/// The backend sits behind a narrow interface so the rest of Grimmoire can stay
/// terminal-agnostic. Terminal restoration is guaranteed twice over: [`leave`]
/// is idempotent and explicitly called on the normal exit path, and [`Drop`]
/// performs the same restore if the backend is dropped early (for example while
/// unwinding from a panic).
///
/// [`leave`]: TerminalBackend::leave
pub struct TerminalBackend {
    stdout: Stdout,
    active: bool,
}

impl TerminalBackend {
    /// Creates a backend without touching terminal state yet.
    ///
    /// Call [`TerminalBackend::enter`] before drawing.
    pub fn new() -> io::Result<Self> {
        Ok(TerminalBackend {
            stdout: io::stdout(),
            active: false,
        })
    }

    /// Switches into the alternate screen with raw keyboard input enabled.
    ///
    /// Calling this when the terminal is already active is a no-op.
    pub fn enter(&mut self) -> io::Result<()> {
        if self.active {
            return Ok(());
        }
        terminal::enable_raw_mode()?;
        execute!(self.stdout, EnterAlternateScreen, Hide)?;
        self.active = true;
        Ok(())
    }

    /// Restores the terminal to the state it was in before [`TerminalBackend::enter`].
    ///
    /// Idempotent: calling it more than once (or after a panic) is harmless.
    pub fn leave(&mut self) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let restored = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        restored
    }

    /// The current terminal size as `(columns, rows)`.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        terminal::size()
    }

    /// Waits up to `timeout` for an input event.
    ///
    /// Returns `Ok(None)` when the timeout elapses without input, which lets the
    /// caller keep animating or ticking while remaining input-driven.
    pub fn poll_event(&self, timeout: Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    /// Paints an entire canvas to the terminal.
    ///
    /// Colours and attributes are only re-emitted when they change between
    /// adjacent cells, which keeps output small. An initial reset guarantees a
    /// predictable starting state even if a previous frame left styling active.
    pub fn present(&mut self, canvas: &Canvas) -> io::Result<()> {
        let width = canvas.width();
        let height = canvas.height();

        queue!(self.stdout, SetAttribute(Attribute::Reset), ResetColor)?;

        let mut current: Option<Style> = None;
        for y in 0..height {
            queue!(self.stdout, MoveTo(0, y))?;

            // Printing into the bottom-right corner makes a terminal scroll, so
            // the final cell of the final row is skipped. It is almost always
            // blank and the alternative (a disabled autowrap mode) is more
            // invasive than it is worth here.
            let row_width = if y + 1 == height {
                width.saturating_sub(1)
            } else {
                width
            };

            for x in 0..row_width {
                let cell = canvas.get(x, y).unwrap_or_default();
                if current != Some(cell.style) {
                    queue_style(&mut self.stdout, cell.style)?;
                    current = Some(cell.style);
                }
                queue!(self.stdout, Print(cell.ch))?;
            }
        }

        queue!(self.stdout, SetAttribute(Attribute::Reset), ResetColor)?;
        self.stdout.flush()
    }
}

impl Drop for TerminalBackend {
    fn drop(&mut self) {
        // Best effort: Drop cannot report the failure, and `leave` on an
        // already-restored terminal is a no-op.
        let _ = self.leave();
    }
}

/// Installs a panic hook that restores the terminal before the panic message is
/// printed, so a crash never strands the user inside the alternate screen.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        previous(info);
    }));
}

/// Emits the escape sequences that reproduce `style` from a clean attribute state.
fn queue_style(out: &mut Stdout, style: Style) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reset))?;
    if style.bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if style.dim {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    if style.italic {
        queue!(out, SetAttribute(Attribute::Italic))?;
    }
    if style.underline {
        queue!(out, SetAttribute(Attribute::Underlined))?;
    }
    queue!(
        out,
        SetForegroundColor(to_terminal_color(style.fg)),
        SetBackgroundColor(to_terminal_color(style.bg)),
    )
}

/// Maps a renderer [`Color`] onto the equivalent crossterm colour.
fn to_terminal_color(color: Color) -> TerminalColor {
    match color {
        Color::Default => TerminalColor::Reset,
        // crossterm's bare names are the bright variants; the base 16 colours
        // live under the `Dark*` names.
        Color::Black => TerminalColor::Black,
        Color::Red => TerminalColor::DarkRed,
        Color::Green => TerminalColor::DarkGreen,
        Color::Yellow => TerminalColor::DarkYellow,
        Color::Blue => TerminalColor::DarkBlue,
        Color::Magenta => TerminalColor::DarkMagenta,
        Color::Cyan => TerminalColor::DarkCyan,
        Color::White => TerminalColor::Grey,
        Color::DarkGrey => TerminalColor::DarkGrey,
        Color::LightRed => TerminalColor::Red,
        Color::LightGreen => TerminalColor::Green,
        Color::LightYellow => TerminalColor::Yellow,
        Color::LightBlue => TerminalColor::Blue,
        Color::LightMagenta => TerminalColor::Magenta,
        Color::LightCyan => TerminalColor::Cyan,
        Color::Indexed(index) => TerminalColor::AnsiValue(index),
        Color::Rgb(r, g, b) => TerminalColor::Rgb { r, g, b },
    }
}
