//! Game state and rules.
//!
//! This module owns the playable world. It is pure: no rendering, no canvas, no
//! terminal I/O. Movement, collision, interaction resolution, and area travel
//! all live here and are testable without a terminal.
//!
//! * [`area`] — bounded tile grids, terrain, doorways, and positions.
//! * [`entity`] — the NPCs, props and braziers placed in an area.
//! * [`conversation`] — branching dialogue: nodes, choices, conditions, effects.
//! * [`discovery`] — the things worth learning about the world.
//! * [`progress`] — what has been learned, and the shrine puzzle's state.
//! * [`world`] — the player's place in the world and the rules that move them.
//! * [`areas`] — the authored region: its maps, cast, and points of interest.

pub mod area;
pub mod areas;
pub mod conversation;
pub mod discovery;
pub mod entity;
pub mod progress;
pub mod world;

pub use area::{Area, AreaId, Direction, Portal, Position, Tile};
pub use conversation::{Choice, Condition, Conversation, Effect, Flag, Known, Mood, Node, NodeId};
pub use discovery::Discovery;
pub use entity::{Entity, EntityKind, Figure, Prop};
pub use progress::{Lighted, MoonPhase, Progress};
pub use world::{Interaction, Prompt, TalkStart, Target, World};
