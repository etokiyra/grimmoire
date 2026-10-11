//! Terminal lifecycle management and frame presentation.
//!
//! This module is the **only** place in the crate that speaks directly to the
//! terminal. It owns raw mode, the alternate screen, input polling, and the
//! translation of renderer values into escape sequences. Game and UI code never
//! import a terminal library.

mod backend;

pub use backend::{TerminalBackend, install_panic_hook};
