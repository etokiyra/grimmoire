//! Prints the ASCII world as text, for looking at the scenery without a terminal.
//!
//! ```sh
//! cargo run --release --example preview -- chapel 120 40
//! ```
//!
//! Visual work cannot be checked by compiling, and a terminal screenshot is not
//! always to hand. This renders the same scenery the game does and prints it, so
//! the art can be read, diffed and iterated on directly.

use grimmoire::game::{AreaId, Position, World, areas};
use grimmoire::render::Canvas;
use grimmoire::ui::Palette;
use grimmoire::ui::scenery::Scenery;

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "chapel".to_string());
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
    if area == areas::CHAPEL {
        let _ = world.place_player(Position::new(60, 34));
    } else {
        let _ = world.place_player(Position::new(36, 14));
    }

    let mut canvas = Canvas::new(width, height);
    canvas.clear_with(Palette::EXTENDED.backdrop());
    {
        let mut view = canvas.view(canvas.bounds());
        Scenery {
            world: &world,
            palette: &Palette::EXTENDED,
            phase,
            moving: true,
        }
        .draw(&mut view);
    }

    for y in 0..canvas.height() {
        let mut line = String::new();
        for x in 0..canvas.width() {
            line.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
        }
        println!("{}", line.trim_end());
    }
}
