//! Game state and rules.
//!
//! This module deliberately contains almost nothing. Its purpose in this phase
//! is to pin down *where* game state will live so that rendering never reaches
//! into game logic and rules never reach into the renderer. World data, the
//! player's spellbook, and progression arrive in later phases; see `ROADMAP.md`.

/// The root of all Grimmoire game state.
///
/// A marker type for now: it exists to establish the render/game boundary and
/// give future systems a single place to hang off.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GameState;

impl GameState {
    /// Creates a fresh game state.
    #[must_use]
    pub const fn new() -> Self {
        GameState
    }
}
