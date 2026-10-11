//! Entities: the things placed in an area that the player can interact with.
//!
//! Entities carry content (a glyph, a name, a line of description) but no
//! presentation: the UI decides how each kind is coloured. This keeps the game
//! model free of rendering concerns.

use super::area::Position;
use super::dialogue::Dialogue;

/// What an entity is, and therefore what interacting with it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    /// A character who speaks.
    Npc {
        /// What they say.
        dialogue: Dialogue,
    },
    /// Scenery or an object that can be examined.
    Object {
        /// The line shown when the player examines it.
        description: &'static str,
    },
}

/// A placed thing in an area: an NPC or an interactable object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    /// Where the entity stands. Entities block movement.
    pub position: Position,
    /// The glyph used to draw the entity.
    pub glyph: char,
    /// How the entity is referred to in prompts and messages.
    pub name: &'static str,
    /// What the entity is.
    pub kind: EntityKind,
}

impl Entity {
    /// Creates an NPC that speaks.
    #[must_use]
    pub const fn npc(
        position: Position,
        glyph: char,
        name: &'static str,
        dialogue: Dialogue,
    ) -> Self {
        Entity {
            position,
            glyph,
            name,
            kind: EntityKind::Npc { dialogue },
        }
    }

    /// Creates an object that can be examined.
    #[must_use]
    pub const fn object(
        position: Position,
        glyph: char,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Entity {
            position,
            glyph,
            name,
            kind: EntityKind::Object { description },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Entity, EntityKind};
    use crate::game::area::Position;
    use crate::game::dialogue::Dialogue;

    #[test]
    fn npc_entities_carry_dialogue() {
        let dialogue = Dialogue::new("Someone", &["Hello."]);
        let entity = Entity::npc(Position::new(1, 2), 'N', "Someone", dialogue);
        assert_eq!(Position::new(1, 2), entity.position);
        assert_eq!('N', entity.glyph);
        assert_eq!(EntityKind::Npc { dialogue }, entity.kind);
    }

    #[test]
    fn object_entities_carry_a_description() {
        let entity = Entity::object(Position::new(0, 0), 'o', "a thing", "It is a thing.");
        assert_eq!(
            EntityKind::Object {
                description: "It is a thing."
            },
            entity.kind
        );
    }
}
