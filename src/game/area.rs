//! Areas: bounded tile grids that the player explores.
//!
//! An [`Area`] is authored as fixed-width ASCII rows (so the content reads like
//! the game it produces) plus explicit lists of entities and doorways. This
//! module is pure data and geometry: it knows nothing about rendering, input, or
//! terminal I/O.

use super::entity::Entity;

/// A stable identifier for an area, used in save data and lookups.
///
/// Ids are static strings rather than indices so that saving and reordering the
/// area list can never silently move the player to the wrong place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AreaId(&'static str);

impl AreaId {
    /// Creates an id from a stable, static slug such as `"ruined-chapel"`.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        AreaId(id)
    }

    /// The slug for this id.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A grid position within an area, measured in cells from the top-left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    /// Column, increasing to the right.
    pub x: u16,
    /// Row, increasing downwards.
    pub y: u16,
}

impl Position {
    /// Creates a position.
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Position { x, y }
    }

    /// The neighbouring position one step in `direction`.
    ///
    /// Returns `None` when the step would leave a `width` by `height` grid, so
    /// boundary handling never relies on unsigned subtraction.
    #[must_use]
    pub fn checked_step(self, direction: Direction, width: u16, height: u16) -> Option<Position> {
        let (dx, dy) = direction.delta();
        let x = i32::from(self.x) + dx;
        let y = i32::from(self.y) + dy;
        if x < 0 || y < 0 || x >= i32::from(width) || y >= i32::from(height) {
            None
        } else {
            Some(Position {
                x: x as u16,
                y: y as u16,
            })
        }
    }
}

/// A cardinal direction on the grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    /// Upwards (decreasing `y`).
    North,
    /// Rightwards (increasing `x`).
    East,
    /// Downwards (increasing `y`).
    South,
    /// Leftwards (decreasing `x`).
    West,
}

impl Direction {
    /// Every direction in a stable order, used for deterministic resolution.
    pub const ALL: [Direction; 4] = [
        Direction::North,
        Direction::East,
        Direction::South,
        Direction::West,
    ];

    /// The `(dx, dy)` step for this direction.
    #[must_use]
    pub const fn delta(self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::East => (1, 0),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
        }
    }

    /// A lower-case name, for player-facing readouts.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Direction::North => "north",
            Direction::East => "east",
            Direction::South => "south",
            Direction::West => "west",
        }
    }
}

/// The terrain of a single cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tile {
    /// Bare floor: walkable.
    Floor,
    /// A worn path: walkable, and visually distinct from floor.
    Path,
    /// A solid wall: blocks movement.
    Wall,
    /// Water: blocks movement.
    Water,
    /// A tree or other standing growth: blocks movement.
    Tree,
    /// A doorway to another area: blocks movement until interacted with.
    Door,
}

impl Tile {
    /// Whether the player may stand on this terrain, ignoring entities.
    #[must_use]
    pub const fn is_walkable(self) -> bool {
        matches!(self, Tile::Floor | Tile::Path)
    }

    /// Maps an authoring character onto terrain. Unknown characters are floor.
    fn from_char(ch: char) -> Tile {
        match ch {
            '#' => Tile::Wall,
            '~' => Tile::Water,
            'T' => Tile::Tree,
            ',' => Tile::Path,
            '+' => Tile::Door,
            _ => Tile::Floor,
        }
    }
}

/// A one-way doorway leading to another area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Portal {
    /// Where the doorway is drawn, and where the player must stand adjacent to.
    pub at: Position,
    /// The area this doorway leads to.
    pub to: AreaId,
    /// Where the player appears in the destination area.
    pub arrive: Position,
    /// A short player-facing label for the destination.
    pub label: &'static str,
}

impl Portal {
    /// Creates a doorway.
    #[must_use]
    pub const fn new(at: Position, to: AreaId, arrive: Position, label: &'static str) -> Self {
        Portal {
            at,
            to,
            arrive,
            label,
        }
    }
}

/// A bounded tile grid with entities and doorways.
#[derive(Debug, Clone)]
pub struct Area {
    id: AreaId,
    name: &'static str,
    width: u16,
    height: u16,
    tiles: Vec<Tile>,
    entities: Vec<Entity>,
    portals: Vec<Portal>,
    spawn: Position,
}

impl Area {
    /// Builds an area from ASCII rows plus explicit entities and doorways.
    ///
    /// The width is taken from the first row. Rows shorter than that are padded
    /// with wall, and longer rows are truncated, so authoring mistakes cannot
    /// produce an out-of-bounds grid.
    #[must_use]
    pub fn new(
        id: AreaId,
        name: &'static str,
        rows: &[&str],
        entities: Vec<Entity>,
        portals: Vec<Portal>,
        spawn: Position,
    ) -> Area {
        let width = rows.first().map_or(0, |row| row.chars().count() as u16);
        let height = u16::try_from(rows.len()).unwrap_or(u16::MAX);
        let mut tiles = Vec::with_capacity(usize::from(width) * usize::from(height));
        for row in rows {
            let mut chars = row.chars();
            for _ in 0..width {
                tiles.push(Tile::from_char(chars.next().unwrap_or('#')));
            }
        }
        Area {
            id,
            name,
            width,
            height,
            tiles,
            entities,
            portals,
            spawn,
        }
    }

    /// A minimal, sealed placeholder area.
    ///
    /// Only used as a defensive fallback when a [`super::world::World`] is built
    /// with no areas at all, so the game can never be left without a place to be.
    #[must_use]
    pub fn void() -> Area {
        Area::new(
            AreaId::new("void"),
            "Nowhere",
            &["###", "#.#", "###"],
            Vec::new(),
            Vec::new(),
            Position::new(1, 1),
        )
    }

    /// This area's stable id.
    #[must_use]
    pub const fn id(&self) -> AreaId {
        self.id
    }

    /// The player-facing name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The grid width in cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// The grid height in cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// The position the player starts at when the game begins here.
    #[must_use]
    pub const fn spawn(&self) -> Position {
        self.spawn
    }

    /// Placements in this area.
    #[must_use]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    /// Doorways in this area.
    #[must_use]
    pub fn portals(&self) -> &[Portal] {
        &self.portals
    }

    /// Whether `position` lies inside the grid.
    #[must_use]
    pub fn in_bounds(&self, position: Position) -> bool {
        position.x < self.width && position.y < self.height
    }

    /// The terrain at `position`. Out-of-bounds positions read as wall, so they
    /// are never walkable and never panic.
    #[must_use]
    pub fn tile(&self, position: Position) -> Tile {
        if !self.in_bounds(position) {
            return Tile::Wall;
        }
        let index = usize::from(position.y) * usize::from(self.width) + usize::from(position.x);
        self.tiles.get(index).copied().unwrap_or(Tile::Wall)
    }

    /// Whether the terrain at `position` is walkable.
    #[must_use]
    pub fn tile_walkable(&self, position: Position) -> bool {
        self.in_bounds(position) && self.tile(position).is_walkable()
    }

    /// The entity at `position`, if any.
    #[must_use]
    pub fn entity_at(&self, position: Position) -> Option<Entity> {
        self.entity_index_at(position)
            .and_then(|index| self.entities.get(index).copied())
    }

    /// The index of the entity at `position`, if any.
    #[must_use]
    pub fn entity_index_at(&self, position: Position) -> Option<usize> {
        self.entities
            .iter()
            .position(|entity| entity.position == position)
    }

    /// The entity at `index`, if it exists.
    #[must_use]
    pub fn entity(&self, index: usize) -> Option<Entity> {
        self.entities.get(index).copied()
    }

    /// The index of the portal at `position`, if any.
    #[must_use]
    pub fn portal_index_at(&self, position: Position) -> Option<usize> {
        self.portals.iter().position(|portal| portal.at == position)
    }

    /// The portal at `index`, if it exists.
    #[must_use]
    pub fn portal(&self, index: usize) -> Option<Portal> {
        self.portals.get(index).copied()
    }

    /// Whether the player may stand on `position`, accounting for entities.
    #[must_use]
    pub fn is_walkable(&self, position: Position) -> bool {
        self.tile_walkable(position) && self.entity_at(position).is_none()
    }

    /// Whether `position` is within this area's grid and reachable in principle.
    ///
    /// Used when validating restored save data.
    #[must_use]
    pub fn is_valid_standing_spot(&self, position: Position) -> bool {
        self.tile_walkable(position)
    }
}

#[cfg(test)]
mod tests {
    use super::{Area, AreaId, Direction, Portal, Position, Tile};

    fn sample() -> Area {
        Area::new(
            AreaId::new("sample"),
            "Sample",
            &["###", "#.#", "###"],
            Vec::new(),
            Vec::new(),
            Position::new(1, 1),
        )
    }

    #[test]
    fn rows_become_terrain() {
        let area = sample();
        assert_eq!(3, area.width());
        assert_eq!(3, area.height());
        assert_eq!(Tile::Wall, area.tile(Position::new(0, 0)));
        assert_eq!(Tile::Floor, area.tile(Position::new(1, 1)));
    }

    #[test]
    fn known_characters_map_to_terrain() {
        let area = Area::new(
            AreaId::new("t"),
            "T",
            &["#~T,+@"],
            Vec::new(),
            Vec::new(),
            Position::new(0, 0),
        );
        assert_eq!(Tile::Wall, area.tile(Position::new(0, 0)));
        assert_eq!(Tile::Water, area.tile(Position::new(1, 0)));
        assert_eq!(Tile::Tree, area.tile(Position::new(2, 0)));
        assert_eq!(Tile::Path, area.tile(Position::new(3, 0)));
        assert_eq!(Tile::Door, area.tile(Position::new(4, 0)));
        assert_eq!(Tile::Floor, area.tile(Position::new(5, 0)));
    }

    #[test]
    fn ragged_rows_are_padded_and_truncated_safely() {
        let area = Area::new(
            AreaId::new("ragged"),
            "Ragged",
            &["....", "..", "......"],
            Vec::new(),
            Vec::new(),
            Position::new(0, 0),
        );
        assert_eq!(4, area.width());
        assert_eq!(3, area.height());
        // The short middle row is padded with wall.
        assert_eq!(Tile::Wall, area.tile(Position::new(3, 1)));
        // The long final row is truncated, not extended.
        assert_eq!(Tile::Wall, area.tile(Position::new(4, 2)));
    }

    #[test]
    fn out_of_bounds_reads_as_wall() {
        let area = sample();
        assert!(!area.in_bounds(Position::new(9, 9)));
        assert_eq!(Tile::Wall, area.tile(Position::new(9, 9)));
        assert!(!area.tile_walkable(Position::new(9, 9)));
    }

    #[test]
    fn checked_step_stops_at_boundaries() {
        let area = sample();
        assert_eq!(
            Some(Position::new(1, 0)),
            Position::new(1, 1).checked_step(Direction::North, area.width(), area.height())
        );
        assert_eq!(
            None,
            Position::new(0, 1).checked_step(Direction::West, area.width(), area.height())
        );
        assert_eq!(
            None,
            Position::new(1, 0).checked_step(Direction::North, area.width(), area.height())
        );
    }

    #[test]
    fn entities_block_movement_and_are_found_by_position() {
        let entity = super::super::entity::Entity::object(
            Position::new(1, 1),
            'o',
            "an object",
            "nothing much",
        );
        let area = Area::new(
            AreaId::new("e"),
            "E",
            &["###", "#.#", "###"],
            vec![entity],
            Vec::new(),
            Position::new(1, 1),
        );
        assert_eq!(Some(entity), area.entity_at(Position::new(1, 1)));
        assert!(!area.is_walkable(Position::new(1, 1)));
        assert!(area.tile_walkable(Position::new(1, 1)));
    }

    #[test]
    fn portals_are_found_by_position() {
        let portal = Portal::new(
            Position::new(2, 2),
            AreaId::new("elsewhere"),
            Position::new(0, 0),
            "Elsewhere",
        );
        let area = Area::new(
            AreaId::new("p"),
            "P",
            &["###", "#.#", "###"],
            Vec::new(),
            vec![portal],
            Position::new(1, 1),
        );
        assert_eq!(
            Some(portal),
            area.portal(area.portal_index_at(Position::new(2, 2)).expect("portal"))
        );
        assert_eq!(None, area.portal_index_at(Position::new(0, 0)));
    }
}
