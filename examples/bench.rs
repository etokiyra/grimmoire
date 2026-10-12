//! A benchmark for the exploration path.
//!
//! Reports what the game actually costs, so performance claims in
//! `docs/renderer-design.md` can be re-checked rather than trusted:
//!
//! ```sh
//! cargo run --release --example bench
//! ```
//!
//! Output is written to an in-memory sink, so this measures Grimmoire's work
//! rather than the terminal's.

use std::io::{self, Write};
use std::time::{Duration, Instant};

use grimmoire::assets::Assets;
use grimmoire::game::{Direction, World, areas};
use grimmoire::input::Key;
use grimmoire::render::{Canvas, Frame, ImagePlacement};
use grimmoire::terminal::{Capabilities, GraphicsSupport, PresentStats, Presenter};
use grimmoire::ui::art::Art;
use grimmoire::ui::{ExplorationScreen, FrameContext, Palette, Screen};

/// A sink that discards output while counting it.
#[derive(Default)]
struct Sink {
    bytes: usize,
}

impl Write for Sink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs `body` `iterations` times and reports the mean cost.
fn time(label: &str, iterations: u32, mut body: impl FnMut()) {
    for _ in 0..3 {
        body();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        body();
    }
    let elapsed = start.elapsed().as_secs_f64() / f64::from(iterations);
    if elapsed < 1e-3 {
        println!("{label:52} {:>9.1} µs", elapsed * 1e6);
    } else {
        println!("{label:52} {:>9.3} ms", elapsed * 1e3);
    }
}

fn main() {
    let assets = Assets::default();
    let context = FrameContext {
        capabilities: Capabilities::conservative(),
        palette: Palette::EXTENDED,
        graphics: GraphicsSupport::Confirmed,
        art: Art::none(),
        stats: PresentStats::default(),
        frame_time: Duration::ZERO,
        animating: false,
        key_releases: true,
        motion: grimmoire::ui::Motion::Full,
    };
    let (width, height) = (120u16, 40u16);
    println!(
        "viewport: {width}x{height} cells (the world is ASCII, so cells are the resolution)\n"
    );

    let mut screen = ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
    screen.on_terminal_ready(true);
    screen.on_cell_metrics(Some((9, 20)));

    let mut canvas = Canvas::new(width, height);
    let mut images: Vec<ImagePlacement> = Vec::new();
    let compose =
        |canvas: &mut Canvas, images: &mut Vec<ImagePlacement>, screen: &ExplorationScreen| {
            canvas.clear_with(context.palette.backdrop());
            let mut frame = Frame::begin(canvas, images);
            screen.compose(&mut frame, &context);
        };

    compose(&mut canvas, &mut images, &screen);
    time("compose a frame (scenery + interface)", 400, || {
        compose(&mut canvas, &mut images, &screen);
    });

    let mut presenter = Presenter::with_graphics(GraphicsSupport::Confirmed);
    let mut sink = Sink::default();
    let mut stats = presenter
        .present_with(&canvas, &images, &assets, &mut sink)
        .expect("present");
    println!(
        "\nfull redraw: {} cells in {} runs, {} KiB",
        stats.cells_written,
        stats.runs,
        stats.bytes / 1024
    );

    stats = presenter
        .present_with(&canvas, &images, &assets, &mut sink)
        .expect("present");
    println!(
        "unchanged frame: {} cells, {} bytes",
        stats.cells_written, stats.bytes
    );

    time("\npresent a full redraw", 200, || {
        presenter.invalidate();
        let _ = presenter.present_with(&canvas, &images, &assets, &mut sink);
    });
    time("present an unchanged frame", 400, || {
        let _ = presenter.present_with(&canvas, &images, &assets, &mut sink);
    });

    // A step scrolls the camera, so a band of cells changes.
    let mut stepping = ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
    stepping.on_terminal_ready(true);
    stepping.on_cell_metrics(Some((9, 20)));
    let mut step_stats = PresentStats::default();
    let mut steps = 0u32;
    time("present one movement step", 200, || {
        let key = if steps % 2 == 0 { 'd' } else { 'a' };
        stepping.handle_key(Key::char(key));
        steps += 1;
        compose(&mut canvas, &mut images, &stepping);
        step_stats = presenter
            .present_with(&canvas, &images, &assets, &mut sink)
            .expect("present");
    });
    println!(
        "a step rewrites {} cells in {} runs ({} bytes)",
        step_stats.cells_written, step_stats.runs, step_stats.bytes
    );

    // How the axes compare: the point of the per-axis cadence is that a step
    // coveres the same screen distance per second either way.
    let movement = stepping.movement();
    for direction in Direction::ALL {
        println!(
            "{:5} step: {} px every {:.0} ms = {:.0} px/s",
            direction.name(),
            movement.step_pixels(direction),
            movement.interval(direction).as_secs_f64() * 1000.0,
            f64::from(movement.step_pixels(direction)) / movement.interval(direction).as_secs_f64()
        );
    }
}
