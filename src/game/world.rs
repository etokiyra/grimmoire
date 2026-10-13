//! The world: the areas, where the player is, and the rules of movement.
//!
//! This is the game model. It performs no terminal I/O and does no drawing: it
//! answers questions (what is adjacent, where can I step, what does this area
//! look like) and mutates only its own state. Everything here is testable
//! without a terminal.

use std::collections::BTreeSet;

use super::area::{Area, AreaId, Direction, Position};
use super::conversation::{Conversation, Effect, Flag, Known};
use super::discovery::Discovery;
use super::entity::{EntityKind, Figure};
use super::progress::{Lighted, MoonPhase, Progress};
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
    /// A doorway is within reach, but shut until something is discovered.
    Sealed(&'static str),
}

/// Everything a conversation screen needs to begin, and nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TalkStart {
    /// The conversation to walk.
    pub talk: &'static Conversation,
    /// Who is speaking, for the nameplate and the prompt.
    pub name: &'static str,
    /// Which silhouette to draw them with.
    pub figure: Figure,
    /// What the player already knows, so the graph can be resolved.
    pub known: Known,
}

/// The outcome of an interaction attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Interaction {
    /// Nothing was within reach; a message explains this.
    Nothing,
    /// Feedback was recorded in the message line.
    Message,
    /// A conversation should begin.
    Talk(TalkStart),
    /// Something was learned for the first time, and recorded in the journal.
    Discovered(Discovery),
    /// The player stepped through into another area.
    Travelled(AreaId),
}

/// The whole playable world: its areas, the player's place among them, and what
/// they have learned.
#[derive(Debug, Clone)]
pub struct World {
    areas: Vec<Area>,
    current: usize,
    player: Position,
    facing: Direction,
    visited: BTreeSet<AreaId>,
    progress: Progress,
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
            progress: Progress::new(),
            message: String::new(),
        }
    }

    /// What the player has learned, and what it has changed.
    #[must_use]
    pub const fn progress(&self) -> &Progress {
        &self.progress
    }

    /// The discoveries recorded, in the order they were found.
    #[must_use]
    pub fn journal(&self) -> &[Discovery] {
        self.progress.journal()
    }

    /// Whether the player has learned `discovery`.
    #[must_use]
    pub fn knows(&self, discovery: Discovery) -> bool {
        self.progress.knows(discovery)
    }

    /// Applies what a conversation did, in order.
    ///
    /// Returns the discoveries that were genuinely new, so the interface can make
    /// a moment of the first one and stay quiet about the rest.
    pub fn apply(&mut self, effects: &[Effect]) -> Vec<Discovery> {
        let mut learned = Vec::new();
        for effect in effects {
            if self.progress.apply(*effect) {
                if let Effect::Learn(discovery) = effect {
                    learned.push(*discovery);
                }
            }
        }
        learned
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
                    EntityKind::Object { .. } | EntityKind::Brazier { .. } => {
                        Prompt::Examine(entity.name)
                    }
                })
            }
            Target::Portal(index) => {
                let portal = self.area().portal(index)?;
                if portal.is_open(|discovery| self.progress.knows(discovery)) {
                    Some(Prompt::Enter(portal.label))
                } else {
                    Some(Prompt::Sealed(portal.label))
                }
            }
        }
    }

    /// Where the adjacent interaction target stands, if there is one.
    ///
    /// Used by the interface to point at what an interaction affected; the game
    /// itself has no opinion about that.
    #[must_use]
    pub fn target_position(&self, target: Target) -> Option<Position> {
        match target {
            Target::Entity(index) => self.area().entity(index).map(|entity| entity.position),
            Target::Portal(index) => self.area().portal(index).map(|portal| portal.at),
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
                Some(entity) => self.use_entity(entity.name, entity.kind),
                None => {
                    self.message = "There is nothing within reach.".to_string();
                    Interaction::Nothing
                }
            },
            Target::Portal(index) => match self.area().portal(index) {
                Some(portal) => {
                    if !portal.is_open(|discovery| self.progress.knows(discovery)) {
                        self.message = format!(
                            "The way to {} is sealed. Something is missing.",
                            portal.label
                        );
                        return Interaction::Message;
                    }
                    match self.index_of(portal.to) {
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
                    }
                }
                None => {
                    self.message = "There is nothing within reach.".to_string();
                    Interaction::Nothing
                }
            },
        }
    }

    /// Applies an interaction with one entity.
    ///
    /// One match over the entity kind, in one place: a new kind of thing the
    /// player can use is a new arm here and a new arm in the renderer, and the
    /// compiler says so.
    fn use_entity(&mut self, name: &'static str, kind: EntityKind) -> Interaction {
        match kind {
            EntityKind::Npc { talk, figure, .. } => {
                self.message = format!("You speak with {name}.");
                Interaction::Talk(TalkStart {
                    talk,
                    name,
                    figure,
                    known: self.progress.snapshot_known(),
                })
            }
            EntityKind::Object {
                description,
                discovery,
                ..
            } => {
                self.message = format!("{name}: {description}");
                match discovery {
                    Some(discovery) if self.progress.record(discovery) => {
                        Interaction::Discovered(discovery)
                    }
                    _ => Interaction::Message,
                }
            }
            EntityKind::Brazier { phase, description } => match self.progress.light(phase) {
                Lighted::Lit => {
                    let lit = self.progress.braziers().len();
                    self.message = format!(
                        "The {} brazier catches. {lit} of 3 are burning.",
                        phase.name()
                    );
                    Interaction::Message
                }
                Lighted::Guttered => {
                    self.message =
                        "The flame gutters out, and takes the others with it. The order matters."
                            .to_string();
                    Interaction::Message
                }
                Lighted::Answered => {
                    let discovery = Discovery::MoonwellAnswered;
                    self.progress.record(discovery);
                    self.message = "All three burn, and the water answers.".to_string();
                    Interaction::Discovered(discovery)
                }
                Lighted::Already => {
                    self.message = format!("{name}: {description}");
                    Interaction::Message
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

    /// Records a discovery without walking to it.
    ///
    /// Test-only: used to set up "and then the player learned something", which is
    /// otherwise a walk across the map.
    #[cfg(test)]
    pub fn record_for_test(&mut self, discovery: Discovery) {
        self.progress.record(discovery);
    }

    /// Puts the player at an area's spawn without travelling there.
    ///
    /// Test-only, and it validates the destination the same way [`World::restore`]
    /// does, so a test cannot start somewhere impossible.
    #[cfg(test)]
    pub fn goto_for_test(&mut self, id: AreaId) {
        let index = self.index_of(id).expect("a known area");
        self.current = index;
        self.player = self.areas[index].spawn();
        assert!(self.area().tile_walkable(self.player));
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
            journal: self.progress.journal_ids(),
            braziers: self.progress.brazier_ids(),
            flags: self.progress.flag_ids(),
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

        // Progression is validated the same way the visited set is: a discovery or
        // a phase this build does not know is dropped on its own, and the rest of
        // the save is kept. Losing a name the game no longer has must never cost
        // the player the progress it *does* still understand.
        let journal = data
            .journal
            .iter()
            .filter_map(|id| Discovery::from_id(id))
            .collect();
        let braziers = data
            .braziers
            .iter()
            .filter_map(|id| MoonPhase::from_id(id))
            .collect();
        let flags = data
            .flags
            .iter()
            .filter_map(|id| Flag::from_id(id))
            .collect();
        self.progress = Progress::restored(journal, flags, braziers);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::{Interaction, Prompt, Target, World};
    use crate::game::area::{Area, AreaId, Direction, Portal, Position};
    use crate::game::conversation::{Conversation, Effect, Flag, Node, NodeId};
    use crate::game::discovery::Discovery;
    use crate::game::entity::{Entity, EntityKind, Figure, Prop};
    use crate::game::progress::MoonPhase;

    const A: AreaId = AreaId::new("a");
    const B: AreaId = AreaId::new("b");

    static STRANGER_NODES: [Node; 1] = [Node::saying(
        NodeId::new("greeting"),
        "a stranger",
        &["Keep to the path."],
    )];
    static STRANGER: Conversation = Conversation {
        nodes: &STRANGER_NODES,
        entries: &[],
        fallback: NodeId::new("greeting"),
    };

    fn areas() -> Vec<Area> {
        let first = Area::new(
            A,
            "Area A",
            &["#####", "#...#", "#..o#", "#...#", "##+##"],
            vec![Entity::object(
                Position::new(3, 2),
                "a pedestal",
                Prop::Basin,
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
                "a stranger",
                Figure::Veiled,
                Direction::South,
                &STRANGER,
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
            Interaction::Talk(start) => {
                assert_eq!("a stranger", start.name);
                let node = start
                    .talk
                    .entry(|condition| start.known.holds(condition))
                    .expect("an entry node");
                assert_eq!(&["Keep to the path."], node.lines);
            }
            other => panic!("expected a conversation, got {other:?}"),
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

    // --- progression --------------------------------------------------------

    /// The real region, starting in the chapel.
    fn region() -> World {
        World::new(crate::game::areas::all(), crate::game::areas::CHAPEL)
    }

    /// Puts the player next to `target` in the current area and faces it, the way
    /// walking into it would.
    fn stand_facing(world: &mut World, at: Position, facing: Direction) {
        world.place_player(at);
        // A step into an occupied tile turns without moving, which is exactly the
        // gesture a player makes before pressing `e`.
        world.move_player(facing);
        assert_eq!(facing, world.facing());
    }

    #[test]
    fn examining_a_relic_records_a_discovery_once() {
        let mut world = region();
        stand_facing(&mut world, Position::new(76, 7), Direction::North);
        assert_eq!(Some(Prompt::Examine("a cracked basin")), world.prompt());

        assert_eq!(
            Interaction::Discovered(Discovery::ChapelBasin),
            world.interact()
        );
        assert!(world.knows(Discovery::ChapelBasin));
        assert_eq!(&[Discovery::ChapelBasin], world.journal());

        // Looking again is a plain examination: the journal does not repeat it,
        // and the interface is not told to celebrate a second time.
        assert_eq!(Interaction::Message, world.interact());
        assert_eq!(1, world.journal().len());
    }

    #[test]
    fn an_item_that_teaches_nothing_stays_a_plain_examination() {
        // Every prop that grants no discovery, approached from whichever side is
        // open floor: examining it says its piece and records nothing. Positions
        // and areas come from the model, so a map edit cannot quietly hollow this
        // out, and the areas are reached the way a save reaches them.
        let mut examined = 0;
        for area in crate::game::areas::all() {
            for entity in area.entities() {
                let EntityKind::Object { discovery, .. } = &entity.kind else {
                    continue;
                };
                if discovery.is_some() {
                    continue;
                }
                for (dx, dy, facing) in [
                    (-1i32, 0i32, Direction::East),
                    (1, 0, Direction::West),
                    (0, -1, Direction::South),
                    (0, 1, Direction::North),
                ] {
                    let at = Position::new(
                        (i32::from(entity.position.x) + dx) as u16,
                        (i32::from(entity.position.y) + dy) as u16,
                    );
                    let mut world = region();
                    let data = crate::save::SaveData::new(area.id().as_str(), at.x, at.y, vec![]);
                    world.restore(&data);
                    world.move_player(facing);
                    if matches!(world.interact(), Interaction::Message) {
                        assert!(
                            world.journal().is_empty(),
                            "{} taught something without a discovery",
                            entity.name
                        );
                        examined += 1;
                        break;
                    }
                }
            }
        }
        assert!(examined > 0, "no plain prop could be examined at all");
    }

    #[test]
    fn a_discovery_changes_what_a_character_says() {
        let mut world = region();
        // Ysolde, in the chapel, before anything has been found.
        stand_facing(&mut world, Position::new(55, 33), Direction::East);
        let Interaction::Talk(start) = world.interact() else {
            panic!("expected a conversation");
        };
        assert_eq!("Ysolde", start.name);
        assert_eq!(Figure::Veiled, start.figure);
        let opening = start
            .talk
            .entry(|condition| start.known.holds(condition))
            .expect("an entry node");
        assert!(
            opening.lines.iter().any(|line| line.contains("remember")),
            "her greeting changed unexpectedly: {:?}",
            opening.lines
        );

        // And once something is found, she has something else to say first.
        world.record_for_test(Discovery::ChapelBasin);
        let Interaction::Talk(start) = world.interact() else {
            panic!("expected a conversation");
        };
        let reacted = start
            .talk
            .entry(|condition| start.known.holds(condition))
            .expect("an entry node");
        assert!(
            reacted.lines.iter().any(|line| line.contains("basin")),
            "she should react to what was found: {:?}",
            reacted.lines
        );
        assert_ne!(opening.id, reacted.id, "the entry should have moved");
    }

    /// The node a named character would start a conversation at right now.
    fn entry_of(world: &World, name: &str) -> crate::game::NodeId {
        let entity = world
            .area()
            .entities()
            .iter()
            .find(|entity| entity.name == name)
            .unwrap_or_else(|| panic!("{name} is not here"));
        let EntityKind::Npc { talk, .. } = entity.kind else {
            panic!("{name} does not talk");
        };
        let known = world.progress().snapshot_known();
        talk.entry(|condition| known.holds(condition))
            .expect("an entry node")
            .id
    }

    #[test]
    fn what_one_conversation_does_changes_what_another_says() {
        // The point of a flag: accusing Cassen is something the *world* keeps,
        // and Maren has an opinion about it that she did not have before.
        let mut world = region();
        world.goto_for_test(crate::game::areas::VILLAGE);
        let cassen_before = entry_of(&world, "Cassen");
        let maren_before = entry_of(&world, "Maren");

        world.apply(&[Effect::Set(Flag::CassenAccused)]);

        assert_ne!(
            cassen_before,
            entry_of(&world, "Cassen"),
            "Cassen should answer an accusation differently"
        );
        assert_ne!(
            maren_before,
            entry_of(&world, "Maren"),
            "and Maren should have something to say about it"
        );
    }

    #[test]
    fn a_conversation_effect_that_teaches_something_joins_the_journal() {
        let mut world = region();
        let learned = world.apply(&[Effect::Learn(Discovery::VillageWell)]);
        assert_eq!(vec![Discovery::VillageWell], learned);
        assert!(world.journal().contains(&Discovery::VillageWell));
        // Applying it again is not news.
        assert!(
            world
                .apply(&[Effect::Learn(Discovery::VillageWell)])
                .is_empty()
        );
    }

    #[test]
    fn the_moonwell_must_be_answered_in_the_order_the_dial_shows() {
        let mut world = region();
        world.goto_for_test(crate::game::areas::SHRINE);
        let braziers = [
            (
                Position::new(78, 17),
                Position::new(78, 16),
                MoonPhase::Waning,
            ),
            (
                Position::new(78, 21),
                Position::new(78, 20),
                MoonPhase::Full,
            ),
            (
                Position::new(78, 25),
                Position::new(78, 24),
                MoonPhase::Waxing,
            ),
        ];

        // Light the last one first: the whole sequence guttered, and nothing is
        // lit afterwards.
        stand_facing(&mut world, braziers[2].0, Direction::North);
        assert_eq!(Interaction::Message, world.interact());
        assert!(world.progress().braziers().is_empty());
        assert!(!world.knows(Discovery::MoonwellAnswered));

        // In order, it answers.
        for (at, _, phase) in braziers {
            stand_facing(&mut world, at, Direction::North);
            world.interact();
            assert!(world.progress().is_lit(phase) || world.progress().answered());
        }
        assert!(world.progress().answered());
        assert!(world.knows(Discovery::MoonwellAnswered));
        assert!(world.journal().contains(&Discovery::MoonwellAnswered));
    }

    #[test]
    fn a_sealed_doorway_refuses_until_the_moonwell_is_answered() {
        let mut world = region();
        world.goto_for_test(crate::game::areas::SHRINE);
        stand_facing(&mut world, Position::new(118, 20), Direction::East);
        assert_eq!(
            Some(Prompt::Sealed("the Forgotten Catacombs")),
            world.prompt()
        );

        assert_eq!(Interaction::Message, world.interact());
        assert_eq!(
            crate::game::areas::SHRINE,
            world.area_id(),
            "a sealed door must not move the player"
        );
        assert!(world.message().contains("sealed"));

        world.record_for_test(Discovery::MoonwellAnswered);
        assert_eq!(
            Some(Prompt::Enter("the Forgotten Catacombs")),
            world.prompt()
        );
        assert_eq!(
            Interaction::Travelled(crate::game::areas::CATACOMBS),
            world.interact()
        );
        assert_eq!(crate::game::areas::CATACOMBS, world.area_id());
    }

    #[test]
    fn every_area_can_be_walked_between_without_a_key() {
        // The region's shape, checked through the *world* rather than the area
        // graph: each hop must land the player somewhere they can actually stand.
        let mut world = region();
        let route = [
            // Each hop stands *next to* the doorway it uses: a portal is used
            // from the tile you face it from, not from across the room.
            (
                Position::new(1, 25),
                Direction::West,
                crate::game::areas::WOODS,
            ),
            (
                Position::new(62, 1),
                Direction::North,
                crate::game::areas::VILLAGE,
            ),
            (
                Position::new(1, 20),
                Direction::West,
                crate::game::areas::SHRINE,
            ),
        ];
        for (at, facing, destination) in route {
            stand_facing(&mut world, at, facing);
            assert_eq!(Interaction::Travelled(destination), world.interact());
            assert!(
                world.area().tile_walkable(world.player()),
                "arrived inside terrain in {}",
                world.area_name()
            );
        }
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
    fn restore_migrates_a_save_from_before_progression_existed() {
        // Exactly what version 1 wrote: no journal, no braziers. The player must
        // keep their place, and simply have nothing discovered yet.
        let legacy = crate::save::SaveData::new(
            "sunken-cellar",
            20,
            12,
            vec!["ruined-chapel".to_string(), "sunken-cellar".to_string()],
        );
        let mut world = region();
        assert!(world.restore(&legacy));
        assert_eq!(crate::game::areas::CELLAR, world.area_id());
        assert_eq!(Position::new(20, 12), world.player());
        assert!(world.journal().is_empty());
        assert!(!world.progress().answered());
        assert_eq!(2, world.visited_count());
    }

    #[test]
    fn restore_drops_unknown_progression_but_keeps_the_rest() {
        let mut world = region();
        let mut data = world.snapshot();
        data.journal = vec![
            "village-well".to_string(),
            "a-discovery-that-was-removed".to_string(),
            "moonwell-answered".to_string(),
        ];
        data.braziers = vec!["waning".to_string(), "gibbous".to_string()];
        assert!(world.restore(&data));
        assert_eq!(
            &[Discovery::VillageWell, Discovery::MoonwellAnswered],
            world.journal(),
            "a name this build does not know must cost only itself"
        );
        assert_eq!(
            &[crate::game::progress::MoonPhase::Waning],
            world.progress().braziers()
        );
    }

    #[test]
    fn restore_rejects_a_save_for_a_place_that_no_longer_exists() {
        let mut world = region();
        let mut data = world.snapshot();
        data.area = "a-place-that-was-cut".to_string();
        assert!(!world.restore(&data));
        assert_eq!(crate::game::areas::CHAPEL, world.area_id());
    }

    #[test]
    fn the_story_the_player_made_survives_a_save_and_a_reload() {
        // The consequences in this build are world state, so they have to be in
        // the save: a chapel lit on Tuesday is still lit on Wednesday.
        let mut world = region();
        world.apply(&[
            crate::game::Effect::Set(crate::game::Flag::YsoldeDeparted),
            crate::game::Effect::Set(crate::game::Flag::RinLantern),
            crate::game::Effect::Learn(Discovery::FirstGiven),
        ]);
        let saved = world.snapshot();

        let mut reloaded = region();
        assert!(reloaded.restore(&saved));
        assert!(reloaded.progress().has(crate::game::Flag::YsoldeDeparted));
        assert!(reloaded.progress().has(crate::game::Flag::RinLantern));
        assert!(reloaded.knows(Discovery::FirstGiven));
        assert_eq!(world.journal(), reloaded.journal());
    }

    #[test]
    fn a_save_from_before_this_stage_loads_with_nothing_new_invented() {
        // What the previous build could have written: the progression as it stood
        // then, and none of this stage's facts. An older journey has to continue
        // exactly as it was, and not be given consequences it never earned.
        let old =
            crate::save::SaveData::new("ruined-chapel", 70, 44, vec!["ruined-chapel".to_string()])
                .with_progress(
                    vec![
                        "chapel-basin".to_string(),
                        "village-well".to_string(),
                        "catacomb-names".to_string(),
                    ],
                    vec![],
                    vec!["ysolde-released".to_string(), "vesper-told".to_string()],
                );
        let mut world = region();
        assert!(world.restore(&old));
        assert!(world.knows(Discovery::CatacombNames));
        assert!(world.progress().has(crate::game::Flag::YsoldeReleased));
        assert!(!world.knows(Discovery::FirstGiven), "invented a discovery");
        assert!(
            !world.progress().has(crate::game::Flag::YsoldeDeparted),
            "invented the chapel being let go"
        );
        assert!(!world.progress().has(crate::game::Flag::RinLantern));
    }

    #[test]
    fn a_fact_from_a_later_build_costs_only_itself() {
        // Forward compatibility by construction: a save naming a flag or a
        // discovery this build has never heard of keeps everything else, so a
        // later phase can add a fact without invalidating anyone's journey.
        let mut world = region();
        let mut data = world.snapshot();
        data.journal = vec![
            "chapel-basin".to_string(),
            "the-name-said-out-loud".to_string(),
        ];
        data.flags = vec![
            "ysolde-released".to_string(),
            "the-bargain-ended".to_string(),
        ];
        assert!(world.restore(&data));
        assert_eq!(&[Discovery::ChapelBasin], world.journal());
        assert!(world.progress().has(crate::game::Flag::YsoldeReleased));
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
