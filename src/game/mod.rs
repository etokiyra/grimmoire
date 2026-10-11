//! Game state and rules.
//!
//! This module owns the playable world. It is pure: no rendering, no canvas, no
//! terminal I/O. Movement, collision, interaction resolution, and area travel
//! all live here and are testable without a terminal.
//!
//! * [`area`] — bounded tile grids, terrain, doorways, and positions.
//! * [`entity`] — the NPCs and objects placed in an area.
//! * [`dialogue`] — the (deliberately minimal) dialogue content model.
//! * [`world`] — the player's place in the world and the rules that move them.
//! * [`areas`] — temporary prototype content.

pub mod area;
pub mod areas;
pub mod dialogue;
pub mod entity;
pub mod world;

pub use area::{Area, AreaId, Direction, Portal, Position, Tile};
pub use dialogue::Dialogue;
pub use entity::{Entity, EntityKind};
pub use world::{Interaction, Prompt, Target, World};
