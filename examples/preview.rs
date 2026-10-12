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
use grimmoire::ui::animation::Performance;
use grimmoire::ui::effects::Effects;
use grimmoire::ui::scenery::{Backdrop, Scenery};

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

/// Prints the sprite library: every sprite, and how its glyphs are coloured.
fn figures() {
    use grimmoire::ui::sprite::{SPRITES, part_of};

    println!("=== the sprite library ===");
    for art in SPRITES {
        println!(
            "--- {} ({:?}) {}x{}: {} poses ---",
            art.id,
            art.role,
            art.width,
            art.height,
            art.pose_count()
        );
        for row in art.rows {
            println!("|{row}|");
        }
        // The inks are chosen by what a glyph *is*, so the mapping is part of
        // the art direction and worth looking at directly.
        let mut seen: Vec<String> = Vec::new();
        for ch in art.rows.iter().flat_map(|row| row.chars()) {
            if ch == ' ' {
                continue;
            }
            let entry = format!("{ch}={:?}", part_of(ch));
            if !seen.contains(&entry) {
                seen.push(entry);
            }
        }
        println!("    inks: {}", seen.join(" "));
        for (index, pose) in art.all_poses().iter().enumerate().skip(1) {
            println!("    pose {index}:");
            for row in *pose {
                println!("    |{row}|");
            }
        }
        println!();
    }

    println!("=== the cast, as the world draws them ===");
    let cast = grimmoire::game::areas::roster();
    let entities: Vec<_> = cast
        .iter()
        .enumerate()
        .map(|(index, member)| {
            Entity::npc(
                Position::new(4 + index as u16 * 8, 6),
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
        &["................................."; 12],
        entities,
        Vec::new(),
        Position::new(4, 10),
    );
    let mut world = World::new(vec![area], AreaId::new("stage"));
    let _ = world.place_player(Position::new(4, 10));
    let all = draw_pose(
        &world,
        grimmoire::ui::animation::Pose::idle(Direction::South),
        78,
        12,
    );
    print_crop(&all, 0, 0, 78, 9);
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
            step: 0,
            effects: &effects,
            backdrop: Backdrop::Material,
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
            step: 0,
            effects: &effects,
            backdrop: Backdrop::Material,
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
