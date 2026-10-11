//! Frame composition: turns state into a [`crate::render::Canvas`].
//!
//! The UI layer decides *what* a screen looks like. It may read capability and
//! frame information, but it performs no terminal I/O and no game rules.

pub mod art;
pub mod help;
pub mod layout;
pub mod screen;
pub mod showcase;
pub mod theme;
pub mod transition;

pub use help::HelpOverlay;
pub use layout::Layout;
pub use screen::{Command, Outcome, Screen, ScreenStack};
pub use showcase::{Scene, Showcase};
pub use theme::Palette;

use std::time::Duration;

use crate::terminal::{Capabilities, PresentStats};

/// Read-only information about the current frame, passed to screens on compose.
///
/// This is how screens learn about the terminal and the renderer without reaching
/// into the application loop: capabilities, the resolved palette, and the
/// statistics of the previously presented frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    /// The terminal's detected capabilities.
    pub capabilities: Capabilities,
    /// The palette resolved for those capabilities.
    pub palette: Palette,
    /// Statistics from the previously presented frame.
    pub stats: PresentStats,
    /// Time elapsed since the previous frame.
    pub frame_time: Duration,
    /// Whether the application currently has motion in progress.
    pub animating: bool,
}
