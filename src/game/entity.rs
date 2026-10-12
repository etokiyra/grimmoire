//! Entities: the things placed in an area that the player can interact with.
//!
//! Entities carry content — a name, a description, dialogue, and the *role* that
//! says what they look like — but no presentation: the interface decides how each
//! role is coloured and drawn. This keeps the game model free of rendering
//! concerns while still letting a figure be more than one letter.
//!
//! # Figures and props
//!
//! A character names a [`Figure`] and an object names a [`Prop`]. Both are roles,
//! not pictures: "a lantern keeper", not "a pink hood". The cast's ASCII art and
//! inks live in [`crate::ui::cast`], and the props live in `ui::scenery`, so a
//! character or an artefact can be redrawn without touching the world model — and
//! the compiler points at every place that has to know about a new one.

use super::area::{Direction, Position};
use super::dialogue::Speech;
use super::discovery::Discovery;
use super::progress::MoonPhase;

/// Which silhouette and inks a character is drawn with.
///
/// The model names the role; the interface chooses what it looks like. Every
/// variant has an entry in the cast table, and a test asserts it, so a character
/// can never be placed with no way to draw them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Figure {
    /// The player: a robed mage with two long ears and a wand.
    Mage,
    /// A deep hood with no face, only two cold lights.
    Veiled,
    /// A small keeper carrying a lantern.
    Lantern,
    /// A hunched scavenger with a bundle and a stick.
    Scavenger,
    /// A tall priestess under a crescent crown.
    Priestess,
    /// A bound revenant, wrapped and chained.
    Revenant,
}

impl Figure {
    /// Every figure a character can be drawn as.
    pub const ALL: [Figure; 6] = [
        Figure::Mage,
        Figure::Veiled,
        Figure::Lantern,
        Figure::Scavenger,
        Figure::Priestess,
        Figure::Revenant,
    ];
}

/// What an interactable object looks like.
///
/// The interface owns the art; this is the role. Objects used to be told apart by
/// their single drawing character, which meant adding one meant editing a match
/// in the renderer and hoping no two shared a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Prop {
    /// A stone basin holding still water.
    Basin,
    /// A sigil cut into the floor.
    Sigil,
    /// A village well, its windlass broken.
    Well,
    /// A bundle of letters, tied with string.
    Letters,
    /// A cairn of stacked stones.
    Cairn,
    /// A standing stone.
    StandingStone,
    /// A dial cut into the stone, marked with the moon's phases.
    Dial,
    /// A stone coffin.
    Sarcophagus,
}

impl Prop {
    /// Every prop the world can place.
    pub const ALL: [Prop; 8] = [
        Prop::Basin,
        Prop::Sigil,
        Prop::Well,
        Prop::Letters,
        Prop::Cairn,
        Prop::StandingStone,
        Prop::Dial,
        Prop::Sarcophagus,
    ];
}

/// What an entity is, and therefore what interacting with it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    /// A character who speaks, from a fixed direction.
    Npc {
        /// What they say, and how that changes as the player learns.
        speech: Speech,
        /// Which silhouette and inks to draw them with.
        figure: Figure,
        /// The way they are standing, so they can face what they are about.
        facing: Direction,
    },
    /// Scenery or an object that can be examined.
    Object {
        /// Which art to draw it with.
        prop: Prop,
        /// The line shown when the player examines it.
        description: &'static str,
        /// Something learned the first time it is examined, if anything.
        discovery: Option<Discovery>,
    },
    /// One of the moonwell's three braziers.
    ///
    /// Its own kind rather than a prop, because it is not scenery: lighting the
    /// braziers in the order the dial shows is the shrine's puzzle, and that
    /// needs a state the world can hold. It is the one stateful interaction in
    /// the game, and it is deliberately one kind rather than a general
    /// "condition system" nothing else would use yet.
    Brazier {
        /// Which phase of the moon this brazier is marked with.
        phase: MoonPhase,
        /// The line shown when it is examined.
        description: &'static str,
    },
}

/// A placed thing in an area: an NPC, a prop, or a brazier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    /// Where the entity stands. Entities block movement.
    pub position: Position,
    /// How the entity is referred to in prompts and messages.
    pub name: &'static str,
    /// What the entity is.
    pub kind: EntityKind,
}

impl Entity {
    /// Creates an NPC who speaks from `facing`.
    #[must_use]
    pub const fn npc(
        position: Position,
        name: &'static str,
        speech: Speech,
        figure: Figure,
        facing: Direction,
    ) -> Self {
        Entity {
            position,
            name,
            kind: EntityKind::Npc {
                speech,
                figure,
                facing,
            },
        }
    }

    /// Creates a prop that can be examined.
    #[must_use]
    pub const fn object(
        position: Position,
        name: &'static str,
        prop: Prop,
        description: &'static str,
    ) -> Self {
        Entity {
            position,
            name,
            kind: EntityKind::Object {
                prop,
                description,
                discovery: None,
            },
        }
    }

    /// Creates a prop that also teaches the player something.
    #[must_use]
    pub const fn relic(
        position: Position,
        name: &'static str,
        prop: Prop,
        description: &'static str,
        discovery: Discovery,
    ) -> Self {
        Entity {
            position,
            name,
            kind: EntityKind::Object {
                prop,
                description,
                discovery: Some(discovery),
            },
        }
    }

    /// Creates one of the moonwell's braziers.
    #[must_use]
    pub const fn brazier(
        position: Position,
        name: &'static str,
        phase: MoonPhase,
        description: &'static str,
    ) -> Self {
        Entity {
            position,
            name,
            kind: EntityKind::Brazier { phase, description },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Entity, EntityKind, Figure, Prop};
    use crate::game::area::{Direction, Position};
    use crate::game::dialogue::{Dialogue, Speech};
    use crate::game::discovery::Discovery;
    use crate::game::progress::MoonPhase;

    #[test]
    fn npc_entities_carry_speech_figure_and_facing() {
        let speech = Speech::always(Dialogue::new("Someone", &["Hello."]));
        let entity = Entity::npc(
            Position::new(1, 2),
            "Someone",
            speech,
            Figure::Lantern,
            Direction::West,
        );
        assert_eq!(Position::new(1, 2), entity.position);
        assert_eq!(
            EntityKind::Npc {
                speech,
                figure: Figure::Lantern,
                facing: Direction::West,
            },
            entity.kind
        );
    }

    #[test]
    fn plain_objects_teach_nothing() {
        let entity = Entity::object(
            Position::new(0, 0),
            "a thing",
            Prop::Basin,
            "It is a thing.",
        );
        assert_eq!(
            EntityKind::Object {
                prop: Prop::Basin,
                description: "It is a thing.",
                discovery: None,
            },
            entity.kind
        );
    }

    #[test]
    fn relics_teach_something() {
        let entity = Entity::relic(
            Position::new(0, 0),
            "a well",
            Prop::Well,
            "It is dry.",
            Discovery::VillageWell,
        );
        assert!(matches!(
            entity.kind,
            EntityKind::Object {
                discovery: Some(Discovery::VillageWell),
                ..
            }
        ));
    }

    #[test]
    fn braziers_carry_their_phase() {
        let entity = Entity::brazier(
            Position::new(3, 4),
            "a waning brazier",
            MoonPhase::Waning,
            "A bronze bowl.",
        );
        assert!(matches!(
            entity.kind,
            EntityKind::Brazier {
                phase: MoonPhase::Waning,
                ..
            }
        ));
    }
}
