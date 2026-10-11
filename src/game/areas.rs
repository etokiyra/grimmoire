//! Temporary prototype world content.
//!
//! The names, lines, and layouts here are deliberately provisional. The story,
//! setting, and characters have not been decided, and nothing in this file
//! should be treated as canon. It exists to make the exploration slice playable
//! and to exercise the world model.
//!
//! Areas are authored as fixed-width ASCII rows; see [`super::area::Area::new`]
//! for the characters that are recognised.

use super::area::{Area, AreaId, Portal, Position};
use super::dialogue::Dialogue;
use super::entity::Entity;

/// The id of the area the game starts in.
pub const CHAPEL: AreaId = AreaId::new("ruined-chapel");

/// The id of the second area.
pub const CELLAR: AreaId = AreaId::new("sunken-cellar");

/// Placeholder dialogue for the only NPC in the slice.
const HOODED_FIGURE: Dialogue = Dialogue::new(
    "the Hooded Figure",
    &[
        "You walk where the stones still remember.",
        "The stair below has not been swept in a long while. Mind the water.",
    ],
);

/// The starting area: a small, walled courtyard with a stair down.
#[must_use]
pub fn chapel() -> Area {
    Area::new(
        CHAPEL,
        "Ruined Chapel",
        &[
            "##############################################",
            "#.....................,,,....................#",
            "#...............######,,,#####...............#",
            "#..............#......,,,.....#..............#",
            "#......#..............,,,.............#......#",
            "#.....................,,,....................#",
            "#.....................,,,....................#",
            "#.....................,,,....................#",
            "#.....................,,,....................#",
            "#.....................,,,....................#",
            "#.....................,,,....................#",
            "#......#..............,,,.............#......#",
            "#..............#......,,,.....#..............#",
            "#...............######,,,#####...............#",
            "#.....................,,,....................#",
            "#######################+######################",
        ],
        vec![
            Entity::npc(
                Position::new(21, 8),
                'N',
                "the Hooded Figure",
                HOODED_FIGURE,
            ),
            Entity::object(
                Position::new(25, 8),
                'o',
                "a cracked basin",
                "Still water, cold and dark; it has not rained in a long while.",
            ),
        ],
        vec![Portal::new(
            Position::new(23, 15),
            CELLAR,
            Position::new(17, 1),
            "the Sunken Cellar",
        )],
        Position::new(23, 12),
    )
}

/// The second area: a cramped, flooded cellar with the way back up.
#[must_use]
pub fn cellar() -> Area {
    Area::new(
        CELLAR,
        "Sunken Cellar",
        &[
            "#################+################",
            "#...............,,,..............#",
            "#...............,,,..............#",
            "#...######......,,,..............#",
            "#...#.........~~,,,..............#",
            "#...#.~~........~,,.....######...#",
            "#...#.~.........,,,..........#...#",
            "#...............,,,..........#...#",
            "#...........##########.......#...#",
            "#...............,,,......~~......#",
            "#...............,,,........~.....#",
            "##################################",
        ],
        vec![Entity::object(
            Position::new(24, 3),
            'o',
            "a faded sigil",
            "The carving is worn smooth, but it hums faintly under your fingers.",
        )],
        vec![Portal::new(
            Position::new(17, 0),
            CHAPEL,
            Position::new(23, 14),
            "the Ruined Chapel",
        )],
        Position::new(17, 1),
    )
}

/// Every area in the prototype world, in a stable order.
#[must_use]
pub fn all() -> Vec<Area> {
    vec![chapel(), cellar()]
}

#[cfg(test)]
mod tests {
    use super::{all, cellar, chapel};
    use crate::game::area::Tile;
    use crate::game::entity::EntityKind;

    #[test]
    fn the_world_has_two_areas_with_distinct_ids_and_names() {
        let areas = all();
        assert_eq!(2, areas.len());
        assert_ne!(areas[0].id(), areas[1].id());
        assert_ne!(areas[0].name(), areas[1].name());
    }

    #[test]
    fn the_areas_have_the_expected_dimensions() {
        assert_eq!((46, 16), (chapel().width(), chapel().height()));
        assert_eq!((34, 12), (cellar().width(), cellar().height()));
    }

    #[test]
    fn the_areas_have_different_shapes() {
        let (a, b) = (chapel(), cellar());
        assert_ne!((a.width(), a.height()), (b.width(), b.height()));
    }

    #[test]
    fn each_area_spawns_the_player_on_walkable_ground() {
        for area in all() {
            assert!(
                area.tile_walkable(area.spawn()),
                "{} spawns inside terrain",
                area.name()
            );
            assert!(area.entity_at(area.spawn()).is_none());
        }
    }

    #[test]
    fn every_doorway_sits_on_a_door_and_arrives_on_walkable_ground() {
        for area in all() {
            for portal in area.portals() {
                assert_eq!(
                    Tile::Door,
                    area.tile(portal.at),
                    "{} has a portal that is not on a door",
                    area.name()
                );
                let destination = all()
                    .into_iter()
                    .find(|candidate| candidate.id() == portal.to)
                    .unwrap_or_else(|| panic!("{} leads to a missing area", area.name()));
                assert!(
                    destination.tile_walkable(portal.arrive),
                    "{} arrives inside terrain in {}",
                    area.name(),
                    destination.name()
                );
                assert!(
                    destination.entity_at(portal.arrive).is_none(),
                    "{} arrives on top of an entity",
                    area.name()
                );
            }
        }
    }

    #[test]
    fn the_world_contains_a_doorway_each_way_and_an_npc_and_object() {
        assert_eq!(1, chapel().portals().len());
        assert_eq!(1, cellar().portals().len());

        let has_npc = chapel()
            .entities()
            .iter()
            .any(|entity| matches!(entity.kind, EntityKind::Npc { .. }));
        assert!(has_npc, "the slice needs an NPC");

        let objects = chapel()
            .entities()
            .iter()
            .chain(cellar().entities().iter())
            .filter(|entity| matches!(entity.kind, EntityKind::Object { .. }))
            .count();
        assert!(objects >= 2, "the slice needs discoverable objects");
    }

    #[test]
    fn entity_glyphs_are_printable_ascii() {
        for area in all() {
            for entity in area.entities() {
                assert!(
                    entity.glyph.is_ascii_graphic(),
                    "{} has a non-ASCII glyph",
                    entity.name
                );
            }
        }
    }
}
