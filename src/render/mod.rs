//! Pure, terminal-agnostic rendering primitives.
//!
//! Everything in this module is independent of any terminal backend and can be
//! exercised without a real terminal. Frame composition happens entirely here;
//! the [`crate::terminal`] module is responsible for turning the resulting
//! [`Canvas`] into escape sequences.
//!
//! # Composition model
//!
//! * [`Canvas`] is a grid of [`Cell`]s with clipping drawing primitives.
//! * [`Frame`] bundles the cell canvas with any [`ImagePlacement`]s a screen
//!   wants drawn, separating cell rendering from graphics-protocol rendering.
//! * [`CanvasView`] is a clipped, local-coordinate window onto a canvas, used to
//!   lay out independent visual regions without leaking between them.
//! * [`Canvas::crop`] and [`Canvas::blit`] compose separate canvases; blitting in
//!   order gives deterministic layering, and a later blit wins where regions
//!   overlap.
//! * [`glyph`] defines the ASCII-first text width policy.

mod canvas;
mod cell;
mod color;
mod frame;
mod image;
mod rect;
mod style;
mod view;

pub mod glyph;

pub use canvas::Canvas;
pub use cell::Cell;
pub use color::Color;
pub use frame::{Frame, ImagePlacement};
pub use image::{Image, Rgb};
pub use rect::Rect;
pub use style::Style;
pub use view::CanvasView;
