//! Frame presentation: turning a composed frame into terminal output.
//!
//! Presentation is separated from terminal lifecycle and from frame composition.
//! The presenter keeps the previous frame and writes only the cells that changed,
//! grouped into contiguous runs and positioned with absolute cursor moves. It
//! writes through any [`Write`] sink, so its output can be captured and asserted
//! in tests without a real terminal.
//!
//! Images are handed to [`Graphics`], which owns the image lifecycle. This module
//! only decides *when* the graphics state must be re-established: after a full
//! redraw (the clear-screen escape code is specified to clear images too) or when
//! the set of placements changed. Ordinary text and UI updates therefore cost no
//! image traffic at all.
//!
//! # Avoiding scroll and corruption
//!
//! Every run is preceded by an absolute cursor move, and cells are never written
//! with a cursor-relative advance across a run boundary. Writing to the last cell
//! of the last row only arms the terminal's pending-wrap flag; the next cursor
//! move clears it, so no scrolling occurs. A full redraw first clears the screen
//! so stale content from a larger previous frame cannot linger.

use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::{
    Attribute, Color as TerminalColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
    SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType};

use crate::assets::Assets;
use crate::render::{AssetId, Canvas, Color, ImagePlacement, Rect, Style, glyph};

use super::graphics::{Graphics, GraphicsSupport};

/// Statistics describing a single presented frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PresentStats {
    /// Width of the presented frame in cells.
    pub width: u16,
    /// Height of the presented frame in cells.
    pub height: u16,
    /// Number of cells actually written to the terminal.
    pub cells_written: u32,
    /// Number of contiguous runs of changed cells.
    pub runs: u32,
    /// Approximate number of bytes written.
    pub bytes: u32,
    /// Whether the frame required clearing the whole screen first.
    pub full_redraw: bool,
    /// Images whose data was transmitted for the first time.
    pub images_transmitted: u32,
    /// Placements written or replaced.
    pub images_placed: u32,
    /// Placements removed.
    pub images_removed: u32,
    /// The rate at which frames have actually been presented, smoothed.
    ///
    /// Filled in by the application loop, which is the only thing that knows how
    /// often it has drawn; the presenter itself is not a clock.
    pub frames_per_second: f32,
}

impl PresentStats {
    /// The total number of cells in the frame.
    #[must_use]
    pub fn total_cells(&self) -> u32 {
        u32::from(self.width) * u32::from(self.height)
    }

    /// The number of cells that matched the previous frame and were skipped.
    #[must_use]
    pub fn cells_skipped(&self) -> u32 {
        self.total_cells().saturating_sub(self.cells_written)
    }
}

/// Diffs frames against the last presented frame and writes the difference.
#[derive(Debug)]
pub struct Presenter {
    previous: Canvas,
    last: PresentStats,
    force_full: bool,
    graphics: Graphics,
    placed_key: Vec<(Rect, AssetId)>,
}

impl Presenter {
    /// Creates a presenter with no graphics, for tests and headless use.
    #[must_use]
    pub fn new() -> Self {
        Self::with_graphics(GraphicsSupport::Unsupported)
    }

    /// Creates a presenter that drives images at the given support level.
    #[must_use]
    pub fn with_graphics(support: GraphicsSupport) -> Self {
        Presenter {
            previous: Canvas::new(0, 0),
            last: PresentStats::default(),
            force_full: true,
            graphics: Graphics::new(support),
            placed_key: Vec::new(),
        }
    }

    /// The graphics state, for diagnostics.
    #[must_use]
    pub const fn graphics(&self) -> &Graphics {
        &self.graphics
    }

    /// Deletes every image and placement this application created.
    ///
    /// Called on shutdown and before a resize, so the terminal is never left
    /// holding stale graphics.
    pub fn clear_graphics<W: Write>(&mut self, out: &mut W) -> io::Result<()> {
        self.placed_key.clear();
        self.graphics.clear(out)
    }

    /// Forces the next presentation to clear and repaint the whole screen.
    ///
    /// Use this after anything writes to the terminal outside the presenter, or
    /// after a resize event, so the diff baseline cannot be stale.
    pub fn invalidate(&mut self) {
        self.force_full = true;
    }

    /// Statistics for the most recently presented frame.
    #[must_use]
    pub const fn last_stats(&self) -> PresentStats {
        self.last
    }

    /// Presents a frame of cells with no images.
    pub fn present<W: Write>(&mut self, frame: &Canvas, out: &mut W) -> io::Result<PresentStats> {
        let assets = Assets::default();
        self.present_with(frame, &[], &assets, out)
    }

    /// Presents `frame`, then makes the terminal's images match `images`.
    ///
    /// Cells covered by an image are never written, so the two layers cannot
    /// fight over the same pixels. When graphics are unavailable the image
    /// rectangles are ignored entirely and the cell layer owns the whole frame.
    pub fn present_with<W: Write>(
        &mut self,
        frame: &Canvas,
        images: &[ImagePlacement],
        assets: &Assets,
        out: &mut W,
    ) -> io::Result<PresentStats> {
        // With no graphics support the cell layer must render the whole frame.
        let drawable: &[ImagePlacement] = if self.graphics.support().is_available() {
            images
        } else {
            &[]
        };

        let mut sink = CountingWriter::new(out);
        let mut stats = PresentStats {
            width: frame.width(),
            height: frame.height(),
            ..PresentStats::default()
        };

        let resized =
            self.previous.width() != frame.width() || self.previous.height() != frame.height();
        let key: Vec<(Rect, AssetId)> = drawable.iter().map(|p| (p.area, p.asset)).collect();
        let images_changed = key != self.placed_key;

        if resized || self.force_full || images_changed {
            queue!(sink, Clear(ClearType::All), MoveTo(0, 0))?;
            self.previous = Canvas::new(frame.width(), frame.height());
            stats.full_redraw = true;
            self.force_full = false;
        }

        // A clean attribute state is guaranteed at the start of every frame.
        queue!(sink, SetAttribute(Attribute::Reset), ResetColor)?;
        let mut current: Option<Style> = None;
        let previous = &self.previous;

        for y in 0..frame.height() {
            let mut x = 0;
            while x < frame.width() {
                if !is_changed(previous, frame, drawable, x, y) {
                    x += 1;
                    continue;
                }
                let run_start = x;
                while x < frame.width() && is_changed(previous, frame, drawable, x, y) {
                    x += 1;
                }

                queue!(sink, MoveTo(run_start, y))?;
                for column in run_start..x {
                    let cell = frame.get(column, y).unwrap_or_default();
                    if current != Some(cell.style) {
                        queue_style(&mut sink, cell.style)?;
                        current = Some(cell.style);
                    }
                    // Enforce one column per cell at the output boundary too, so
                    // a raw `set` can never desynchronise the cursor.
                    let ch = if glyph::is_drawable(cell.ch) {
                        cell.ch
                    } else {
                        ' '
                    };
                    queue!(sink, Print(ch))?;
                    stats.cells_written = stats.cells_written.saturating_add(1);
                }
                stats.runs = stats.runs.saturating_add(1);
            }
        }

        // Images last, so they paint over the rectangles reserved for them and
        // never over text we just drew. `stats.full_redraw` means the screen was
        // cleared, which also clears images, so the graphics state is rebuilt.
        let report = self
            .graphics
            .sync(&mut sink, drawable, assets, stats.full_redraw)?;
        stats.images_transmitted = report.transmitted;
        stats.images_placed = report.placed;
        stats.images_removed = report.removed;

        queue!(sink, SetAttribute(Attribute::Reset), ResetColor)?;
        sink.flush()?;
        stats.bytes = sink.written();
        self.previous.copy_from(frame);
        self.placed_key = key;
        self.last = stats;
        Ok(stats)
    }
}

impl Default for Presenter {
    fn default() -> Self {
        Self::new()
    }
}

/// Whether the cell at `(x, y)` should be written.
///
/// A cell inside an image rectangle is never written: the image owns those
/// pixels, and writing cells over them is how a graphics layer gets shredded by
/// the text layer.
fn is_changed(
    previous: &Canvas,
    frame: &Canvas,
    images: &[ImagePlacement],
    x: u16,
    y: u16,
) -> bool {
    if images.iter().any(|placement| placement.area.contains(x, y)) {
        return false;
    }
    previous.get(x, y) != frame.get(x, y)
}

/// A writer that counts the bytes it forwards to the inner sink.
#[derive(Debug)]
struct CountingWriter<W> {
    inner: W,
    written: u32,
}

impl<W> CountingWriter<W> {
    fn new(inner: W) -> Self {
        CountingWriter { inner, written: 0 }
    }

    fn written(&self) -> u32 {
        self.written
    }
}

impl<W: Write> Write for CountingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let count = self.inner.write(buf)?;
        self.written = self
            .written
            .saturating_add(u32::try_from(count).unwrap_or(u32::MAX));
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Emits the escape sequences that reproduce `style` from a clean state.
fn queue_style<W: Write>(out: &mut W, style: Style) -> io::Result<()> {
    queue!(out, SetAttribute(Attribute::Reset))?;
    if style.bold {
        queue!(out, SetAttribute(Attribute::Bold))?;
    }
    if style.dim {
        queue!(out, SetAttribute(Attribute::Dim))?;
    }
    if style.italic {
        queue!(out, SetAttribute(Attribute::Italic))?;
    }
    if style.underline {
        queue!(out, SetAttribute(Attribute::Underlined))?;
    }
    queue!(
        out,
        SetForegroundColor(to_terminal_color(style.fg)),
        SetBackgroundColor(to_terminal_color(style.bg)),
    )
}

/// Maps a renderer [`Color`] onto the equivalent terminal colour.
fn to_terminal_color(color: Color) -> TerminalColor {
    match color {
        Color::Default => TerminalColor::Reset,
        // The terminal library's bare names are the bright variants; the base 16
        // colours live under the `Dark*` names.
        Color::Black => TerminalColor::Black,
        Color::Red => TerminalColor::DarkRed,
        Color::Green => TerminalColor::DarkGreen,
        Color::Yellow => TerminalColor::DarkYellow,
        Color::Blue => TerminalColor::DarkBlue,
        Color::Magenta => TerminalColor::DarkMagenta,
        Color::Cyan => TerminalColor::DarkCyan,
        Color::White => TerminalColor::Grey,
        Color::DarkGrey => TerminalColor::DarkGrey,
        Color::LightRed => TerminalColor::Red,
        Color::LightGreen => TerminalColor::Green,
        Color::LightYellow => TerminalColor::Yellow,
        Color::LightBlue => TerminalColor::Blue,
        Color::LightMagenta => TerminalColor::Magenta,
        Color::LightCyan => TerminalColor::Cyan,
        Color::Indexed(index) => TerminalColor::AnsiValue(index),
        Color::Rgb(r, g, b) => TerminalColor::Rgb { r, g, b },
    }
}

#[cfg(test)]
mod tests {
    use super::{PresentStats, Presenter};
    use crate::assets::Assets;
    use crate::render::{AssetId, Canvas, ImagePlacement, Rect, Style};
    use crate::terminal::GraphicsSupport;

    fn render(presenter: &mut Presenter, canvas: &Canvas) -> (Vec<u8>, PresentStats) {
        let mut out = Vec::new();
        let stats = presenter.present(canvas, &mut out).expect("present");
        (out, stats)
    }

    fn text_of(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn empty_assets() -> Assets {
        Assets::default()
    }

    fn present_images(
        presenter: &mut Presenter,
        canvas: &Canvas,
        images: &[ImagePlacement],
    ) -> (Vec<u8>, PresentStats) {
        let mut out = Vec::new();
        let assets = empty_assets();
        let stats = presenter
            .present_with(canvas, images, &assets, &mut out)
            .expect("present");
        (out, stats)
    }

    #[test]
    fn first_frame_is_a_full_redraw() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(4, 2);
        canvas.put_str(0, 0, "hi", Style::PLAIN);

        let (bytes, stats) = render(&mut presenter, &canvas);
        assert!(stats.full_redraw);
        assert_eq!(2, stats.cells_written);
        assert!(text_of(&bytes).contains("hi"));
    }

    #[test]
    fn an_unchanged_frame_writes_no_cells() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(8, 4);
        canvas.put_str(1, 1, "steady", Style::PLAIN);

        render(&mut presenter, &canvas);
        let (bytes, stats) = render(&mut presenter, &canvas);

        assert_eq!(0, stats.cells_written);
        assert_eq!(0, stats.runs);
        assert!(!stats.full_redraw);
        // Only the attribute/colour resets around the (empty) frame remain.
        assert!(stats.bytes < 40, "unexpected output: {}", text_of(&bytes));
    }

    #[test]
    fn only_changed_cells_are_rewritten() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(10, 3);
        canvas.put_str(0, 0, "abcdefghij", Style::PLAIN);
        render(&mut presenter, &canvas);

        canvas.put_char(4, 0, 'X', Style::PLAIN);
        let (bytes, stats) = render(&mut presenter, &canvas);

        assert_eq!(1, stats.cells_written);
        assert_eq!(1, stats.runs);
        assert!(text_of(&bytes).contains('X'));
        assert_eq!(29, stats.cells_skipped());
    }

    #[test]
    fn contiguous_changes_form_a_single_run() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 2);
        render(&mut presenter, &canvas);

        canvas.put_str(3, 1, "run", Style::PLAIN);
        let (_, stats) = render(&mut presenter, &canvas);
        assert_eq!(3, stats.cells_written);
        assert_eq!(1, stats.runs);
    }

    #[test]
    fn separated_changes_form_separate_runs() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 1);
        render(&mut presenter, &canvas);

        canvas.put_char(0, 0, 'a', Style::PLAIN);
        canvas.put_char(10, 0, 'b', Style::PLAIN);
        let (_, stats) = render(&mut presenter, &canvas);
        assert_eq!(2, stats.cells_written);
        assert_eq!(2, stats.runs);
    }

    #[test]
    fn resizing_forces_a_clear_and_full_redraw() {
        let mut presenter = Presenter::new();
        let canvas = Canvas::new(4, 2);
        render(&mut presenter, &canvas);

        let mut bigger = Canvas::new(10, 5);
        bigger.put_str(0, 0, "big", Style::PLAIN);
        let (bytes, stats) = render(&mut presenter, &bigger);

        assert!(stats.full_redraw);
        assert!(text_of(&bytes).contains("big"));
    }

    #[test]
    fn the_bottom_right_cell_is_written() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(3, 2);
        canvas.put_char(2, 1, 'Z', Style::PLAIN);
        let (bytes, stats) = render(&mut presenter, &canvas);

        assert_eq!(1, stats.cells_written);
        assert!(text_of(&bytes).contains('Z'));
    }

    #[test]
    fn styling_is_emitted_and_reset() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(2, 1);
        canvas.put_char(0, 0, 'x', Style::new().bold().underline());
        let (bytes, _) = render(&mut presenter, &canvas);
        let output = text_of(&bytes);
        assert!(output.contains("[1m"), "missing bold: {output:?}");
        assert!(output.contains("[4m"), "missing underline: {output:?}");
    }

    #[test]
    fn output_is_deterministic() {
        let mut a = Presenter::new();
        let mut b = Presenter::new();
        let mut canvas = Canvas::new(12, 4);
        canvas.put_str(2, 1, "grimmoire", Style::new().bold());

        let (first, _) = render(&mut a, &canvas);
        let (second, _) = render(&mut b, &canvas);
        assert_eq!(first, second);
    }

    #[test]
    fn cells_under_an_image_are_never_written() {
        let mut presenter = Presenter::with_graphics(GraphicsSupport::Confirmed);
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = [ImagePlacement::new(Rect::new(3, 0, 4, 1), AssetId::new(0))];

        let (_, stats) = present_images(&mut presenter, &canvas, &images);

        // Ten glyphs were composed; the four inside the image are reserved.
        assert_eq!(6, stats.cells_written);
    }

    #[test]
    fn without_graphics_the_reserved_cells_are_drawn_as_text() {
        let mut presenter = Presenter::new(); // no graphics support
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = [ImagePlacement::new(Rect::new(3, 0, 4, 1), AssetId::new(0))];

        let (bytes, stats) = present_images(&mut presenter, &canvas, &images);

        assert_eq!(10, stats.cells_written, "the cell layer must own the area");
        assert_eq!(0, stats.images_placed);
        assert!(!bytes.windows(2).any(|window| window == b"\x1b_G"));
        assert!(text_of(&bytes).contains('3'));
    }

    #[test]
    fn removing_an_image_forces_a_full_repaint_of_its_region() {
        let mut presenter = Presenter::with_graphics(GraphicsSupport::Confirmed);
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = [ImagePlacement::new(Rect::new(3, 0, 4, 1), AssetId::new(0))];

        let (_, first) = present_images(&mut presenter, &canvas, &images);
        assert!(first.full_redraw);

        let (_, second) = present_images(&mut presenter, &canvas, &[]);
        assert!(
            second.full_redraw,
            "the region the image covered must be repainted"
        );
        assert_eq!(10, second.cells_written);
    }

    #[test]
    fn an_unchanged_image_is_not_re_emitted() {
        let mut presenter = Presenter::with_graphics(GraphicsSupport::Confirmed);
        let mut canvas = Canvas::new(20, 4);
        let images = [ImagePlacement::new(Rect::new(3, 0, 4, 1), AssetId::new(0))];
        present_images(&mut presenter, &canvas, &images);

        canvas.put_char(18, 0, 'x', Style::PLAIN);
        let (bytes, stats) = present_images(&mut presenter, &canvas, &images);

        assert!(!stats.full_redraw);
        assert_eq!(0, stats.images_placed, "the image did not change");
        assert_eq!(0, stats.images_transmitted);
        assert!(text_of(&bytes).contains('x'));
        assert!(!bytes.windows(2).any(|window| window == b"\x1b_G"));
    }
}
