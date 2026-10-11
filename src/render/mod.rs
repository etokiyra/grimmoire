//! Pure, terminal-agnostic rendering primitives.
//!
//! Everything in this module is independent of any terminal backend and can be
//! exercised without a real terminal. Frame composition happens entirely here;
//! the [`crate::terminal`] module is responsible for turning the resulting
//! [`Canvas`] into escape sequences.

mod canvas;
mod cell;
mod color;
mod rect;
mod style;

pub use canvas::Canvas;
pub use cell::Cell;
pub use color::Color;
pub use rect::Rect;
pub use style::Style;
