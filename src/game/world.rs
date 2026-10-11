//! The world: the areas, where the player is, and the rules of movement.
//!
//! This is the game model. It performs no terminal I/O and does no drawing: it
//! answers questions (what is adjacent, where can I step, what does this area
//! look like) and mutates only its own state. Everything here is testable
//! without a terminal.

use std::collections::BTreeSet;

use super::area::{Area, AreaId, Direction, Position};
use super::dialogue::Dialogue;
use super::entity::EntityKind;
use crate::save::SaveData;

/// The thing adjacent to the player that an interaction would affect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The entity at this index in the current area.
    Entity(usize),
    /// The doorway at this index in the current area.
    Portal(usize),
}

/// A short hint about an adjacent target, shown when no message is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    /// An NPC is within reach.
    Speak(&'static str),
    /// An object is within reach.
    Examine(&'static str),
    /// A doorway is within reach.
    Enter(&'static str),
}

/// The outcome of an interaction attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interaction {
    /// Nothing was within reach; a message explains this.
    Nothing,
    /// Feedback was recorded in the message line.
    Message,
    /// A dialogue should be shown.
    Dialogue(Dialogue),
    /// The player stepped through into another area.
    Travelled(AreaId),
}

/// The whole playable world: its areas and the player's place among them.
#[derive(Debug, Clone)]
pub struct World {
    areas: Vec<Area>,
    current: usize,
    player: Position,
    facing: Direction,
    visited: BTreeSet<AreaId>,
    message: String,
}

impl World {
    /// Builds a world from `areas`, starting the player in `start`.
    ///
    /// If `start` is not found the first area is used. If `areas` is empty a
    /// sealed placeholder is substituted, so a world can never be left without a
    /// place for the player to stand.
    #[must_use]
    pub fn new(areas: Vec<Area>, start: AreaId) -> World {
        let mut areas = areas;
        if areas.is_empty() {
            areas.push(Area::void());
        }
        let current = areas
            .iter()
            .position(|area| area.id() == start)
            .unwrap_or(0);
        let player = areas[current].spawn();
        let mut visited = BTreeSet::new();
        visited.insert(areas[current].id());
        World {
            areas,
            current,
            player,
            facing: Direction::South,
            visited,
            message: String::new(),
        }
    }

    /// The area the player is currently in.
    #[must_use]
    pub fn area(&self) -> &Area {
        &self.areas[self.current]
    }

    /// The id of the current area.
    #[must_use]
    pub fn area_id(&self) -> AreaId {
        self.area().id()
    }

    /// The name of the current area.
    #[must_use]
    pub fn area_name(&self) -> &'static str {
        self.area().name()
    }

    /// How many areas the world contains.
    #[must_use]
    pub fn area_count(&self) -> usize {
        self.areas.len()
    }

    /// How many distinct areas the player has visited.
    #[must_use]
    pub fn visited_count(&self) -> usize {
        self.visited.len()
    }

    /// The set of visited areas.
    #[must_use]
    pub fn visited(&self) -> &BTreeSet<AreaId> {
        &self.visited
    }

    /// The player's position.
    #[must_use]
    pub const fn player(&self) -> Position {
        self.player
    }

    /// The direction the player last faced.
    #[must_use]
    pub const fn facing(&self) -> Direction {
        self.facing
    }

    /// The most recent feedback message, possibly empty.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Replaces the feedback message.
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    /// Turns to face `direction`, and steps there if the way is clear.
    ///
    /// The player always turns, even when the step is blocked, so they can face
    /// a wall or doorway in order to interact with it. Returns whether the
    /// player actually moved. A successful step also clears the message line so
    /// the contextual prompt for the new surroundings can show.
    pub fn move_player(&mut self, direction: Direction) -> bool {
        self.facing = direction;
        let area = self.area();
        let Some(next) = self
            .player
            .checked_step(direction, area.width(), area.height())
        else {
            return false;
        };
        if !area.is_walkable(next) {
            return false;
        }
        self.player = next;
        self.message.clear();
        true
    }

    /// The adjacent thing the player would interact with.
    ///
    /// The tile the player faces is checked first, then the remaining neighbours
    /// in [`Direction::ALL`] order, so resolution is deterministic when several
    /// things are within reach.
    #[must_use]
    pub fn interaction_target(&self) -> Option<Target> {
        let area = self.area();
        let width = area.width();
        let height = area.height();

        if let Some(facing) = self.player.checked_step(self.facing, width, height) {
            if let Some(index) = area.entity_index_at(facing) {
                return Some(Target::Entity(index));
            }
            if let Some(index) = area.portal_index_at(facing) {
                return Some(Target::Portal(index));
            }
        }

        for direction in Direction::ALL {
            if let Some(position) = self.player.checked_step(direction, width, height) {
                if let Some(index) = area.entity_index_at(position) {
                    return Some(Target::Entity(index));
                }
            }
        }

        for direction in Direction::ALL {
            if let Some(position) = self.player.checked_step(direction, width, height) {
                if let Some(index) = area.portal_index_at(position) {
                    return Some(Target::Portal(index));
                }
            }
        }

        None
    }

    /// A hint about the adjacent target, for the message line.
    #[must_use]
    pub fn prompt(&self) -> Option<Prompt> {
        match self.interaction_target()? {
            Target::Entity(index) => {
                let entity = self.area().entity(index)?;
                Some(match entity.kind {
                    EntityKind::Npc { .. } => Prompt::Speak(entity.name),
                    EntityKind::Object { .. } => Prompt::Examine(entity.name),
                })
            }
            Target::Portal(index) => {
                let portal = self.area().portal(index)?;
                Some(Prompt::Enter(portal.label))
            }
        }
    }

    /// Interacts with the adjacent target, if there is one.
    ///
    /// Never panics and never leaves the world in an inconsistent state: a
    /// doorway pointing at an unknown area simply records a message and does
    /// nothing else.
    pub fn interact(&mut self) -> Interaction {
        let Some(target) = self.interaction_target() else {
            self.message = "There is nothing within reach.".to_string();
            return Interaction::Nothing;
        };

        match target {
            Target::Entity(index) => match self.area().entity(index) {
                Some(entity) => match entity.kind {
                    EntityKind::Npc { dialogue } => {
                        self.message = format!("You speak with {}.", entity.name);
                        Interaction::Dialogue(dialogue)
                    }
                    EntityKind::Object { description } => {
                        self.message = format!("{}: {description}", entity.name);
                        Interaction::Message
                    }
                },
                None => {
                    self.message = "There is nothing within reach.".to_string();
                    Interaction::Nothing
                }
            },
            Target::Portal(index) => match self.area().portal(index) {
                Some(portal) => match self.index_of(portal.to) {
                    Some(destination) => {
                        self.current = destination;
                        self.player = portal.arrive;
                        self.visited.insert(portal.to);
                        self.message = format!("You step through into {}.", self.area().name());
                        Interaction::Travelled(portal.to)
                    }
                    None => {
                        self.message = "The way is sealed.".to_string();
                        Interaction::Nothing
                    }
                },
                None => {
                    self.message = "There is nothing within reach.".to_string();
                    Interaction::Nothing
                }
            },
        }
    }

    /// Moves the player without applying any rules.
    ///
    /// Test-only: play always goes through [`World::move_player`].
    #[cfg(test)]
    pub fn set_player_for_test(&mut self, position: Position) {
        self.player = position;
    }

    /// Puts the player at `position` without a step.
    ///
    /// For tools, tests and restoring a save: it validates the spot so nothing
    /// can be placed inside a wall, and returns whether it worked.
    pub fn place_player(&mut self, position: Position) -> bool {
        if !self.area().is_valid_standing_spot(position) {
            return false;
        }
        self.player = position;
        true
    }

    /// Finds the index of an area by id.
    fn index_of(&self, id: AreaId) -> Option<usize> {
        self.areas.iter().position(|area| area.id() == id)
    }

    /// Captures the essential exploration state for saving.
    #[must_use]
    pub fn snapshot(&self) -> SaveData {
        SaveData {
            area: self.area_id().as_str().to_string(),
            x: self.player.x,
            y: self.player.y,
            visited: self
                .visited
                .iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        }
    }

    /// Applies saved exploration state.
    ///
    /// Returns whether the save was consistent with this world. Anything
    /// unknown — an area that no longer exists, or a position that is not a
    /// valid standing spot — is rejected wholesale so a stale save can never
    /// place the player inside a wall.
    pub fn restore(&mut self, data: &SaveData) -> bool {
        let Some(destination) = self
            .areas
            .iter()
            .position(|area| area.id().as_str() == data.area)
        else {
            return false;
        };
        let position = Position::new(data.x, data.y);
        if !self.areas[destination].is_valid_standing_spot(position) {
            return false;
        }

        self.current = destination;
        self.player = position;
        self.facing = Direction::South;
        self.message.clear();
        self.visited.clear();
        for name in &data.visited {
            if let Some(area) = self.areas.iter().find(|area| area.id().as_str() == name) {
                self.visited.insert(area.id());
            }
        }
        self.visited.insert(self.area_id());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Interaction, Prompt, Target, World};
    use crate::game::area::{Area, AreaId, Direction, Portal, Position};
    use crate::game::dialogue::Dialogue;
    use crate::game::entity::Entity;

    const A: AreaId = AreaId::new("a");
    const B: AreaId = AreaId::new("b");

    fn areas() -> Vec<Area> {
        let first = Area::new(
            A,
            "Area A",
            &["#####", "#...#", "#..o#", "#...#", "##+##"],
            vec![Entity::object(
                Position::new(3, 2),
                'o',
                "a pedestal",
                "It is worn smooth.",
            )],
            vec![Portal::new(
                Position::new(2, 4),
                B,
                Position::new(1, 1),
                "Area B",
            )],
            Position::new(1, 3),
        );
        let second = Area::new(
            B,
            "Area B",
            &["##+##", "#...#", "#.N.#", "#####"],
            vec![Entity::npc(
                Position::new(2, 2),
                'N',
                "a stranger",
                Dialogue::new("a stranger", &["Keep to the path."]),
            )],
            vec![Portal::new(
                Position::new(2, 0),
                A,
                Position::new(1, 3),
                "Area A",
            )],
            Position::new(1, 1),
        );
        vec![first, second]
    }

    fn world() -> World {
        World::new(areas(), A)
    }

    #[test]
    fn a_new_world_starts_at_the_spawn_of_the_named_area() {
        let world = world();
        assert_eq!(A, world.area_id());
        assert_eq!("Area A", world.area_name());
        assert_eq!(Position::new(1, 3), world.player());
        assert_eq!(2, world.area_count());
        assert_eq!(1, world.visited_count());
    }

    #[test]
    fn an_unknown_start_area_falls_back_to_the_first() {
        let world = World::new(areas(), AreaId::new("missing"));
        assert_eq!(A, world.area_id());
    }

    #[test]
    fn an_empty_world_still_has_somewhere_to_stand() {
        let world = World::new(Vec::new(), A);
        assert_eq!("Nowhere", world.area_name());
        assert!(world.area().tile_walkable(world.player()));
    }

    #[test]
    fn movement_works_in_all_four_directions() {
        let mut world = world();
        assert!(world.move_player(Direction::North));
        assert_eq!(Position::new(1, 2), world.player());
        assert!(world.move_player(Direction::East));
        assert_eq!(Position::new(2, 2), world.player());
        assert!(world.move_player(Direction::South));
        assert_eq!(Position::new(2, 3), world.player());
        assert!(world.move_player(Direction::West));
        assert_eq!(Position::new(1, 3), world.player());
    }

    #[test]
    fn walls_block_movement_but_still_turn_the_player() {
        let mut world = world();
        assert!(!world.move_player(Direction::West));
        assert_eq!(Position::new(1, 3), world.player());
        assert_eq!(Direction::West, world.facing());
    }

    #[test]
    fn the_area_boundary_blocks_movement() {
        let mut world = World::new(areas(), B);
        // (1, 1) in area B: north is the wall row, so the player cannot leave.
        assert!(!world.move_player(Direction::North));
        assert_eq!(Position::new(1, 1), world.player());
    }

    #[test]
    fn entities_block_movement() {
        let mut world = world();
        world.move_player(Direction::North); // (1, 2)
        assert!(world.move_player(Direction::East)); // (2, 2)
        assert!(!world.move_player(Direction::East)); // (3, 2) holds the object
        assert_eq!(Position::new(2, 2), world.player());
    }

    #[test]
    fn a_successful_step_clears_the_previous_message() {
        let mut world = world();
        world.interact();
        assert!(!world.message().is_empty());
        world.move_player(Direction::North);
        assert!(world.message().is_empty());
    }

    #[test]
    fn interacting_with_nothing_reports_it() {
        let mut world = world();
        assert_eq!(Interaction::Nothing, world.interact());
        assert!(world.message().contains("nothing"));
    }

    #[test]
    fn objects_are_examined() {
        let mut world = world();
        world.move_player(Direction::North); // (1, 2)
        world.move_player(Direction::East); // (2, 2), facing the object at (3, 2)
        assert_eq!(Some(Target::Entity(0)), world.interaction_target());
        assert_eq!(Some(Prompt::Examine("a pedestal")), world.prompt());
        assert_eq!(Interaction::Message, world.interact());
        assert!(world.message().contains("worn smooth"));
    }

    #[test]
    fn npcs_give_dialogue() {
        let mut world = World::new(areas(), B);
        world.move_player(Direction::East); // (2, 1)
        assert_eq!(Some(Prompt::Speak("a stranger")), world.prompt());
        match world.interact() {
            Interaction::Dialogue(dialogue) => {
                assert_eq!("a stranger", dialogue.speaker);
                assert_eq!(&["Keep to the path."], dialogue.lines);
            }
            other => panic!("expected dialogue, got {other:?}"),
        }
    }

    #[test]
    fn the_facing_tile_wins_when_several_things_are_in_reach() {
        // In area B the NPC is adjacent to the south and the doorway to the
        // north; facing the doorway picks the doorway.
        let mut world = World::new(areas(), B);
        world.move_player(Direction::East); // (2, 1): NPC south, doorway north
        world.move_player(Direction::North); // blocked by the doorway, facing it
        assert_eq!(Some(Target::Portal(0)), world.interaction_target());
    }

    #[test]
    fn neighbours_are_scanned_when_the_facing_tile_is_empty() {
        let mut world = World::new(areas(), B);
        world.move_player(Direction::East); // (2, 1), facing an empty tile
        assert_eq!(Some(Target::Entity(0)), world.interaction_target());
    }

    #[test]
    fn doors_move_the_player_between_areas_and_keep_visited_state() {
        let mut world = world();
        world.move_player(Direction::North); // (1, 2)
        world.move_player(Direction::South); // (1, 3)
        world.move_player(Direction::East); // (2, 3)
        world.move_player(Direction::South); // blocked by the door, now facing it
        assert_eq!(Some(Prompt::Enter("Area B")), world.prompt());

        assert_eq!(Interaction::Travelled(B), world.interact());
        assert_eq!(B, world.area_id());
        assert_eq!(Position::new(1, 1), world.player());
        assert_eq!(2, world.visited_count());
        assert!(world.message().contains("Area B"));
    }

    #[test]
    fn a_door_to_an_unknown_area_does_nothing_badly() {
        let orphan = Area::new(
            AreaId::new("orphan"),
            "Orphan",
            &["#.#", "#+#", "###"],
            Vec::new(),
            vec![Portal::new(
                Position::new(1, 1),
                AreaId::new("nowhere"),
                Position::new(0, 0),
                "Nowhere",
            )],
            Position::new(1, 0),
        );
        let mut world = World::new(vec![orphan], AreaId::new("orphan"));
        world.move_player(Direction::South); // face the door
        assert_eq!(Interaction::Nothing, world.interact());
        assert_eq!("Orphan", world.area_name());
    }

    #[test]
    fn snapshot_and_restore_round_trip() {
        let mut world = world();
        world.move_player(Direction::North);
        world.move_player(Direction::East);
        let data = world.snapshot();

        let mut restored = World::new(areas(), A);
        assert!(restored.restore(&data));
        assert_eq!(world.player(), restored.player());
        assert_eq!(world.area_id(), restored.area_id());
        assert_eq!(world.visited(), restored.visited());
    }

    #[test]
    fn restore_after_travel_returns_to_the_other_area() {
        let mut world = world();
        world.move_player(Direction::North); // (1, 2)
        world.move_player(Direction::South); // (1, 3)
        world.move_player(Direction::East); // (2, 3)
        world.move_player(Direction::South); // blocked by the door, facing it
        world.interact(); // travel to B
        let data = world.snapshot();

        let mut restored = World::new(areas(), A);
        assert!(restored.restore(&data));
        assert_eq!(B, restored.area_id());
        assert_eq!(2, restored.visited_count());
    }

    #[test]
    fn restore_rejects_an_unknown_area() {
        let mut world = world();
        let mut data = world.snapshot();
        data.area = "does-not-exist".to_string();
        assert!(!world.restore(&data));
        assert_eq!(A, world.area_id());
    }

    #[test]
    fn restore_rejects_a_position_inside_a_wall() {
        let mut world = world();
        let mut data = world.snapshot();
        data.x = 0;
        data.y = 0; // corner wall
        assert!(!world.restore(&data));
        assert_eq!(Position::new(1, 3), world.player());
    }

    #[test]
    fn restore_drops_unknown_areas_from_the_visited_set() {
        let mut world = world();
        let mut data = world.snapshot();
        data.visited = vec!["a".to_string(), "b".to_string(), "ghost".to_string()];
        assert!(world.restore(&data));
        assert_eq!(2, world.visited_count(), "unknown areas must be dropped");
    }
}
