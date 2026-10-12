//! Prints the ASCII world as text, for looking at the art without a terminal.
//!
//! ```sh
//! cargo run --release --example preview -- chapel 120 40
//! cargo run --release --example preview -- woods
//! cargo run --release --example preview -- figures
//! ```
//!
//! Any area can be named by any word of its id: `chapel`, `cellar`, `woods`,
//! `village`, `shrine`, `catacombs`.
//!
//! Visual work cannot be checked by compiling, and a terminal screenshot is not
//! always to hand. This renders through the same scenery renderer the game uses
//! and prints the result, so art can be read, diffed and iterated on directly.

use grimmoire::game::{Area, AreaId, Direction, Entity, Position, World, areas};
use grimmoire::render::Canvas;
use grimmoire::ui::Palette;
use grimmoire::ui::animation::{Performance, Stage};
use grimmoire::ui::effects::Effects;
use grimmoire::ui::scenery::Scenery;

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "chapel".to_string());
    if which == "figures" {
        figures();
        return;
    }

    let width: u16 = args.next().and_then(|v| v.parse().ok()).unwrap_or(120);
    let height: u16 = args.next().and_then(|v| v.parse().ok()).unwrap_or(40);
    let phase: f32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let area = area_named(&which).unwrap_or(areas::CHAPEL);
    let mut world = World::new(areas::all(), area);
    let _ = world.place_player(viewpoint(area));

    print_canvas(&draw(&world, width, height, phase, false));
}

/// Finds an area by any word of its id, so `woods` finds the Whispering Woods.
fn area_named(name: &str) -> Option<AreaId> {
    let wanted = name.to_ascii_lowercase();
    areas::all()
        .into_iter()
        .map(|area| area.id())
        .find(|id| id.as_str() == wanted || id.as_str().split('-').any(|word| word == wanted))
}

/// A viewpoint in an area with something worth looking at.
fn viewpoint(area: AreaId) -> Position {
    if area == areas::CHAPEL {
        Position::new(60, 34)
    } else if area == areas::CELLAR {
        Position::new(36, 14)
    } else if area == areas::WOODS {
        Position::new(60, 30)
    } else if area == areas::VILLAGE {
        Position::new(62, 26)
    } else if area == areas::SHRINE {
        Position::new(78, 26)
    } else {
        Position::new(66, 21)
    }
}

/// Prints every figure, facing every way, in every stage.
fn figures() {
    // A bare stage with the whole cast standing clear of the player and of each
    // other, so every design can be read without overlap.
    // The sheet reads the real cast, so art can never drift from the game.
    let cast = grimmoire::game::areas::roster();
    let entities = cast
        .iter()
        .enumerate()
        .map(|(index, member)| {
            Entity::npc(
                Position::new(3 + index as u16 * 6, 5),
                member.name,
                member.figure,
                Direction::South,
                member.talk,
            )
        })
        .collect();
    let area = Area::new(
        AreaId::new("stage"),
        "Stage",
        &[
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
            "..............................",
        ],
        entities,
        Vec::new(),
        Position::new(5, 5),
    );
    let mut world = World::new(vec![area], AreaId::new("stage"));
    let _ = world.place_player(Position::new(13, 10));

    println!("=== the cast, facing south ===");
    let all = draw_pose(
        &world,
        grimmoire::ui::animation::Pose::idle(Direction::South),
        42,
        12,
    );
    print_crop(&all, 6, 3, 34, 4);

    // An NPC's facing is content, not something the viewer can pose: each one is
    // built standing the way it is asked about, so the sheet shows what the world
    // would really draw.
    for member in cast {
        println!("=== {} ===", member.name);
        for facing in Direction::ALL {
            let mut world = single(member, facing);
            let _ = world.place_player(Position::new(13, 10));
            let canvas = draw_pose(&world, grimmoire::ui::animation::Pose::idle(facing), 20, 14);
            println!("--- {}, facing {} ---", member.name, facing.name());
            print_crop(&canvas, 8, 3, 10, 5);
        }
    }

    // And the player, in every stage of the walk.
    let mut world = single(&MAGE_MEMBER, Direction::South);
    let _ = world.place_player(Position::new(13, 10));

    for (label, stage) in [
        ("idle", Stage::Idle),
        ("walk", Stage::Walk),
        ("sprint", Stage::Sprint),
    ] {
        for facing in Direction::ALL {
            let mut performance = Performance::new();
            if stage != Stage::Idle {
                performance.stepped(std::time::Duration::from_millis(120));
            }
            let mut pose = performance.pose(facing, stage != Stage::Idle, stage == Stage::Sprint);
            pose.stage = stage;
            pose.blinking = false;
            let canvas = draw_pose(&world, pose, 20, 14);
            println!("--- mage {label}, facing {} ---", facing.name());
            print_crop(&canvas, 8, 3, 10, 5);
        }
    }
}

/// A stand-in cast member for the player, who is not part of the cast.
static MAGE_MEMBER: grimmoire::game::areas::CastMember = grimmoire::game::areas::CastMember {
    name: "the mage",
    figure: grimmoire::game::Figure::Mage,
    facing: Direction::South,
    talk: &grimmoire::game::areas::YOADLE_TALK,
};

/// A stage holding one figure, so it can be read at every facing without its
/// neighbours in the way.
fn single(member: &'static grimmoire::game::areas::CastMember, facing: Direction) -> World {
    let area = Area::new(
        AreaId::new("stage"),
        "Stage",
        &[".............................."; 12],
        vec![Entity::npc(
            Position::new(13, 5),
            member.name,
            member.figure,
            facing,
            member.talk,
        )],
        Vec::new(),
        Position::new(5, 5),
    );
    World::new(vec![area], AreaId::new("stage"))
}

/// Draws a world at a fixed size with a given player pose.
fn draw_pose(
    world: &World,
    player: grimmoire::ui::animation::Pose,
    width: u16,
    height: u16,
) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    canvas.clear_with(Palette::EXTENDED.backdrop());
    let effects = Effects::new();
    {
        let mut view = canvas.view(canvas.bounds());
        Scenery {
            world,
            palette: &Palette::EXTENDED,
            phase: 0.4,
            player,
            effects: &effects,
        }
        .draw(&mut view);
    }
    canvas
}

/// Draws a world at a given size and phase.
fn draw(world: &World, width: u16, height: u16, phase: f32, stepping: bool) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    canvas.clear_with(Palette::EXTENDED.backdrop());
    let mut performance = Performance::new();
    if stepping {
        performance.stepped(std::time::Duration::from_millis(120));
    }
    let effects = Effects::new();
    {
        let mut view = canvas.view(canvas.bounds());
        Scenery {
            world,
            palette: &Palette::EXTENDED,
            phase,
            player: performance.pose(world.facing(), stepping, false),
            effects: &effects,
        }
        .draw(&mut view);
    }
    canvas
}

/// Prints a window of a canvas.
fn print_crop(canvas: &Canvas, x: u16, y: u16, width: u16, height: u16) {
    for row in y..y.saturating_add(height).min(canvas.height()) {
        let mut line = String::new();
        for column in x..x.saturating_add(width).min(canvas.width()) {
            line.push(canvas.get(column, row).map_or(' ', |cell| cell.ch));
        }
        println!("|{line}|");
    }
}

/// Prints a whole canvas.
fn print_canvas(canvas: &Canvas) {
    for y in 0..canvas.height() {
        let mut line = String::new();
        for x in 0..canvas.width() {
            line.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
        }
        println!("{}", line.trim_end());
    }
}
