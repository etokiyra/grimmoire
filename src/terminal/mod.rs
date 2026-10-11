//! Terminal lifecycle, capabilities, input, and frame presentation.
//!
//! This module is the **only** place in the crate that speaks directly to the
//! terminal. It owns raw mode, the alternate screen, input polling, capability
//! detection, and the translation of renderer values into escape sequences.
//!
//! * [`Terminal`] manages terminal state and delivers [`crate::input::Event`]s.
//! * [`Presenter`] diffs composed frames and writes the minimum needed output.
//! * [`Capabilities`] reports what the terminal can do.

mod backend;
mod capabilities;
mod presenter;

pub use backend::{Terminal, install_panic_hook};
pub use capabilities::{Capabilities, ColorSupport, GraphicsSupport};
pub use presenter::{PresentStats, Presenter};
