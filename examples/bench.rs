//! A small benchmark for the exploration path.
//!
//! Reports the cost of composing and presenting the game, so performance claims
//! in `docs/renderer-design.md` can be re-checked rather than trusted.
//!
//!     cargo run --release --example bench
//!
//! Cells are written into an in-memory sink, so this measures Grimmoire's work
//! rather than the terminal's.

use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use grimmoire::assets::Assets;
use grimmoire::game::{World, areas};
use grimmoire::render::{Canvas, Frame, ImagePlacement, Rect};
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

fn time(label: &str, iterations: u32, mut body: impl FnMut()) {
    // Warm up, then measure.
    for _ in 0..3 {
        body();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        body();
    }
    let elapsed = start.elapsed().as_secs_f64() / f64::from(iterations);
    if elapsed < 1e-3 {
        println!("{label:52} {:>10.3} µs", elapsed * 1e6);
    } else {
        println!("{label:52} {:>10.3} ms", elapsed * 1e3);
    }
}

fn main() {
    let assets = match Assets::default_dir() {
        Some(dir) => Assets::load_dir(&dir).expect("assets"),
        None => {
            eprintln!("no assets directory; run from the repository root");
            std::process::exit(1);
        }
    };
    let art = Art::resolve(&assets);
    let portrait = assets.named("portrait").expect("portrait").png().len();
    println!(
        "assets: {} files, {} KiB ({portrait} bytes portrait)\n",
        assets.len(),
        assets.total_bytes() / 1024
    );

    let palette = Palette::EXTENDED;
    let context = FrameContext {
        capabilities: Capabilities::conservative(),
        palette,
        graphics: GraphicsSupport::Forced,
        art,
        stats: PresentStats::default(),
        frame_time: Duration::ZERO,
        animating: false,
        key_releases: true,
    };

    let mut screen = ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
    screen.on_terminal_ready(true);

    let (width, height) = (120u16, 40u16);
    let mut canvas = Canvas::new(width, height);
    let mut images: Vec<ImagePlacement> = Vec::new();

    fn compose(
        screen: &ExplorationScreen,
        canvas: &mut Canvas,
        images: &mut Vec<ImagePlacement>,
        context: &FrameContext,
    ) {
        canvas.clear_with(context.palette.backdrop());
        let mut frame = Frame::begin(canvas, images);
        screen.compose(&mut frame, context);
    }

    compose(&screen, &mut canvas, &mut images, &context);
    time("compose the world (120x40)", 200, || {
        compose(&screen, &mut canvas, &mut images, &context);
    });

    // The arrival plate, so a large image placement is included.
    let plate = Rect::new(12, 10, 96, 20);
    let plate_images = vec![ImagePlacement::new(
        plate,
        assets.named("cellar").expect("cellar").id(),
    )];

    let mut presenter = Presenter::with_graphics(GraphicsSupport::Forced);
    let mut sink = Sink::default();
    let mut stats = presenter
        .present_with(&canvas, &plate_images, &assets, &mut sink)
        .expect("present");
    println!(
        "\nfull redraw with an image: {} cells, {} runs, {} KiB, transmitted {} image(s) of {} KiB",
        stats.cells_written,
        stats.runs,
        stats.bytes / 1024,
        stats.images_transmitted,
        assets.total_bytes() / 1024
    );
    stats = presenter
        .present_with(&canvas, &plate_images, &assets, &mut sink)
        .expect("present");
    println!(
        "unchanged frame:           {} cells, {} runs, {} bytes, transmitted {} image(s)",
        stats.cells_written, stats.runs, stats.bytes, stats.images_transmitted
    );

    time("\npresent a full redraw (120x40, one image)", 30, || {
        presenter.invalidate();
        let _ = presenter.present_with(&canvas, &plate_images, &assets, &mut sink);
    });

    time("present an unchanged frame", 200, || {
        let _ = presenter.present_with(&canvas, &plate_images, &assets, &mut sink);
    });

    // A single step scrolls the camera, so a band of the frame changes.
    let mut stepping = ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
    stepping.on_terminal_ready(true);
    let mut step_stats = PresentStats::default();
    let mut steps = 0u32;
    time("present a movement step (scrolled camera)", 200, || {
        // East then west: every step shifts the camera, so the whole band of
        // visible cells is rewritten - the worst case for a scrolling camera.
        let key = if steps % 2 == 0 { 'd' } else { 'a' };
        stepping.handle_key(grimmoire::input::Key::char(key));
        steps += 1;
        compose(&stepping, &mut canvas, &mut images, &context);
        step_stats = presenter
            .present_with(&canvas, &plate_images, &assets, &mut sink)
            .expect("present");
    });
    println!(
        "a step rewrites {} cells in {} runs ({} bytes)",
        step_stats.cells_written, step_stats.runs, step_stats.bytes
    );

    let path = Path::new("assets/portrait.png");
    let bytes = std::fs::read(path).map(|data| data.len()).unwrap_or(0);
    println!(
        "\nportrait on disk: {bytes} bytes; base64 in flight: ~{} KiB",
        bytes * 4 / 3 / 1024
    );
}
