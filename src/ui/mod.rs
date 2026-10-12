//! Frame composition: turns state into a [`crate::render::Canvas`].
//!
//! The UI layer decides *what* a screen looks like. It may read capability and
//! frame information, but it performs no terminal I/O and no game rules.
//!
//! The world is drawn by [`scenery`], a custom ASCII renderer: tiles choose their
//! glyph from what surrounds them, vary by world position, and are shaded by
//! distance from the player's light.

pub mod animation;
pub mod art;
pub mod cast;
pub mod dialogue;
pub mod effects;
pub mod exploration;
pub mod help;
pub mod journal;
pub mod layout;
pub mod motion;
pub mod ornament;
pub mod scenery;
pub mod screen;
pub mod settings;
pub mod showcase;
pub mod stage;
pub mod theme;
pub mod title;
pub mod transition;
pub mod viewport;

pub use dialogue::DialogueScreen;
pub use exploration::ExplorationScreen;
pub use help::HelpOverlay;
pub use journal::JournalScreen;
pub use layout::Layout;
pub use screen::{Command, Outcome, Screen, ScreenStack};
pub use settings::{Motion, Settings, Shared};
pub use showcase::{Scene, Showcase};
pub use stage::StageScreen;
pub use theme::Palette;
pub use title::TitleScreen;

use std::time::Duration;

use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};

use self::art::Art;

/// Read-only information about the current frame, passed to screens on compose.
///
/// This is how screens learn about the terminal, the renderer, and the artwork
/// without reaching into the application loop.
#[derive(Debug, Clone, Copy)]
pub struct FrameContext {
    /// The terminal's detected capabilities.
    pub capabilities: Capabilities,
    /// The palette resolved for those capabilities.
    pub palette: Palette,
    /// Whether terminal graphics are usable, and how confidently that is known.
    pub graphics: GraphicsSupport,
    /// The artwork the game can display.
    pub art: Art,
    /// Statistics from the previously presented frame.
    pub stats: PresentStats,
    /// Time elapsed since the previous frame.
    pub frame_time: Duration,
    /// Whether the application currently has motion in progress.
    pub animating: bool,
    /// Whether the terminal reports key releases, for diagnostics.
    pub key_releases: bool,
    /// How much motion the player has asked for.
    pub motion: Motion,
}
