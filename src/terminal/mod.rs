//! Terminal lifecycle, capabilities, input, frame presentation, and graphics.
//!
//! This module is the **only** place in the crate that speaks directly to the
//! terminal. It owns raw mode, the alternate screen, input polling, capability
//! detection, image encoding, and the translation of renderer values into escape
//! sequences.
//!
//! * [`Terminal`] manages terminal state and delivers [`crate::input::Event`]s.
//! * [`Presenter`] diffs composed frames and writes the minimum needed output.
//! * [`Capabilities`] reports what the terminal can do.
//! * [`ImageBackend`] decides whether, and how, images can be drawn.

mod backend;
mod capabilities;
mod graphics;
mod presenter;
mod sixel;

pub use backend::{Terminal, install_panic_hook};
pub use capabilities::{
    Capabilities, CellMetrics, ColorSupport, GraphicsProtocol, GraphicsSupport,
};
pub use graphics::ImageBackend;
pub use presenter::{PresentStats, Presenter};
