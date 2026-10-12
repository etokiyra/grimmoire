//! Prints the ASCII world as text, for looking at the art without a terminal.
//!
//! ```sh
//! cargo run --release --example preview -- chapel 120 40
//! cargo run --release --example preview -- figures
//! ```
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
    let area: AreaId = if which == "cellar" {
        areas::CELLAR
    } else {
        areas::CHAPEL
    };
    let mut world = World::new(areas::all(), area);
    // A viewpoint with something to look at, for both areas.
    let at = if area == areas::CHAPEL {
        Position::new(60, 34)
    } else {
        Position::new(36, 14)
    };
    let _ = world.place_player(at);

    print_canvas(&draw(&world, width, height, phase, false));
}

/// Prints every figure, facing every way, in every stage.
fn figures() {
    // A bare stage with one other figure standing clear of the player, so both
    // designs can be read without overlap.
    let area = Area::new(
        AreaId::new("stage"),
        "Stage",
        &[
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
        ],
        vec![Entity::npc(
            Position::new(13, 8),
            'N',
            "Ysolde",
            grimmoire::game::Dialogue::new("Ysolde", &["..."]),
        )],
        Vec::new(),
        Position::new(5, 5),
    );
    let world = World::new(vec![area], AreaId::new("stage"));

    println!("=== Ysolde ===");
    let npc = draw_pose(
        &world,
        grimmoire::ui::animation::Pose::idle(Direction::South),
    );
    print_crop(&npc, 10, 4, 10, 6);

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
            let canvas = draw_pose(&world, pose);
            println!("--- mage {label}, facing {} ---", facing.name());
            print_crop(&canvas, 2, 1, 10, 6);
        }
    }
}

/// Draws a world at a fixed size with a given player pose.
fn draw_pose(world: &World, player: grimmoire::ui::animation::Pose) -> Canvas {
    let mut canvas = Canvas::new(20, 14);
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
