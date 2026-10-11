//! Terminal lifecycle, capabilities, input, presentation, and Kitty graphics.
//!
//! This module is the **only** place in the crate that speaks directly to the
//! terminal. It owns raw mode, the alternate screen, the Kitty keyboard and
//! graphics protocols, input polling, capability detection, and the translation
//! of renderer values into escape sequences.
//!
//! * [`Terminal`] manages terminal state and delivers [`crate::input::Event`]s.
//! * [`Presenter`] diffs composed frames and writes the minimum needed output.
//! * [`Capabilities`] reports colour support.
//! * [`Graphics`] owns the image lifecycle: transmission, placement, deletion.

mod backend;
mod capabilities;
mod graphics;
mod kitty;
mod presenter;

pub use backend::{Terminal, install_panic_hook};
pub use capabilities::{Capabilities, ColorSupport};
pub use graphics::{Graphics, GraphicsSupport, SyncReport, classify, detect};
pub use presenter::{PresentStats, Presenter};
