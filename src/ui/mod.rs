//! Frame composition: turns state into a [`crate::render::Canvas`].
//!
//! The UI layer decides *what* a screen looks like. It may read game and
//! interface state, but it never performs game rules and never talks to the
//! terminal.

pub mod theme;
pub mod title;

pub use title::TitleScreen;
