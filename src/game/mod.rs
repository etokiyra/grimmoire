//! Game state and rules.
//!
//! This module owns the playable world. It is pure: no rendering, no canvas, no
//! terminal I/O. Movement, collision, interaction resolution, and area travel
//! all live here and are testable without a terminal.
//!
//! * [`area`] — bounded tile grids, terrain, doorways, and positions.
//! * [`entity`] — the NPCs, props and braziers placed in an area.
//! * [`dialogue`] — what a character says, including how it changes.
//! * [`discovery`] — the things worth learning about the world.
//! * [`progress`] — what has been learned, and the shrine puzzle's state.
//! * [`world`] — the player's place in the world and the rules that move them.
//! * [`areas`] — the authored region: its maps, cast, and points of interest.

pub mod area;
pub mod areas;
pub mod dialogue;
pub mod discovery;
pub mod entity;
pub mod progress;
pub mod world;

pub use area::{Area, AreaId, Direction, Portal, Position, Tile};
pub use dialogue::{Dialogue, Speech};
pub use discovery::Discovery;
pub use entity::{Entity, EntityKind, Figure, Prop};
pub use progress::{Lighted, MoonPhase, Progress};
pub use world::{Interaction, Prompt, Target, World};
