//! Software pixel rendering, for optional interface art.
//!
//! This is **not** how the game is drawn. Grimmoire's world is ASCII: it is
//! rendered into the cell grid by [`crate::ui::scenery`]. The pixel layer exists
//! only so that interface pieces can be drawn as real artwork when the
//! development flag asks for it (see `GRIMMOIRE_PIXEL_UI`), and it is off by
//! default.
//!
//! It is a small, honest toolkit — a flat RGB canvas, sprites authored as ASCII,
//! and a PNG encoder — rather than a graphics framework. Nothing here knows
//! about the terminal, the game, or the world.

pub mod art;
pub mod canvas;
pub mod sprite;

pub use art::art;
pub use canvas::{PixelCanvas, Rgb};
pub use sprite::Sprite;
