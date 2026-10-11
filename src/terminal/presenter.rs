//! Frame presentation: turning a composed [`Canvas`] into terminal output.
//!
//! Presentation is separated from terminal lifecycle and from frame composition.
//! The presenter keeps the previous frame and writes only the cells that changed,
//! grouped into contiguous runs and positioned with absolute cursor moves. It
//! writes through any [`Write`] sink, so its output can be captured and asserted
//! in tests without a real terminal.
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

use crate::render::{Canvas, Color, ImagePlacement, Style, glyph};

use super::capabilities::CellMetrics;
use super::graphics::ImageBackend;

/// Statistics describing a single presented frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    /// Number of images emitted.
    pub images_drawn: u32,
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
///
/// Graphics are handled here too, because only this layer knows what the terminal
/// was last told. Image rectangles are *reserved*: no cell inside one is ever
/// written, and the images themselves are re-emitted whenever the set of
/// placements changes or after a full redraw. That keeps ordinary UI text and
/// graphics from fighting over the same pixels.
#[derive(Debug)]
pub struct Presenter {
    previous: Canvas,
    last: PresentStats,
    force_full: bool,
    image_keys: Vec<u64>,
}

impl Presenter {
    /// Creates a presenter that will perform a full redraw on its first frame.
    #[must_use]
    pub fn new() -> Self {
        Presenter {
            previous: Canvas::new(0, 0),
            last: PresentStats::default(),
            force_full: true,
            image_keys: Vec::new(),
        }
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
        self.present_with(frame, &[], ImageBackend::None, None, out)
    }

    /// Presents `frame`, its `images`, writing only what changed.
    ///
    /// Cells covered by an image are never written. If the set of images differs
    /// from the previous frame — including when one disappears — the whole screen
    /// is repainted first, so nothing can be left behind in a region that is no
    /// longer covered.
    pub fn present_with<W: Write>(
        &mut self,
        frame: &Canvas,
        images: &[ImagePlacement],
        backend: ImageBackend,
        metrics: Option<CellMetrics>,
        out: &mut W,
    ) -> io::Result<PresentStats> {
        // A backend that cannot draw is treated as having no images at all, so
        // the cell layer stays fully responsible for the rectangle.
        let drawable: &[ImagePlacement] = if backend.is_available() { images } else { &[] };
        let keys: Vec<u64> = drawable.iter().map(ImagePlacement::key).collect();
        let images_changed = keys != self.image_keys;

        let mut sink = CountingWriter::new(out);
        let mut stats = PresentStats {
            width: frame.width(),
            height: frame.height(),
            ..PresentStats::default()
        };

        let resized =
            self.previous.width() != frame.width() || self.previous.height() != frame.height();
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

        // Images last, so they paint over the (unwritten) rectangles reserved for
        // them, and never over text we just drew.
        if backend.is_available() && (images_changed || stats.full_redraw) {
            for placement in drawable {
                let Some(scaled) = backend.resize_for(
                    &placement.image,
                    placement.area.width,
                    placement.area.height,
                    metrics,
                ) else {
                    continue;
                };
                let Some(bytes) = backend.encode(&scaled) else {
                    continue;
                };
                queue!(sink, MoveTo(placement.area.x, placement.area.y))?;
                sink.write_all(&bytes)?;
                stats.images_drawn = stats.images_drawn.saturating_add(1);
            }
            // A sixel leaves the cursor somewhere unhelpful; put it back.
            queue!(sink, MoveTo(0, 0))?;
        }

        queue!(sink, SetAttribute(Attribute::Reset), ResetColor)?;
        sink.flush()?;
        stats.bytes = sink.written();
        self.previous.copy_from(frame);
        self.image_keys = keys;
        self.last = stats;
        Ok(stats)
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

impl Default for Presenter {
    fn default() -> Self {
        Self::new()
    }
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
    use std::sync::Arc;

    use super::{ImageBackend, PresentStats, Presenter};
    use crate::render::{Canvas, Image, ImagePlacement, Rect, Rgb, Style};
    use crate::terminal::CellMetrics;

    fn render(presenter: &mut Presenter, canvas: &Canvas) -> (Vec<u8>, PresentStats) {
        let mut out = Vec::new();
        let stats = presenter.present(canvas, &mut out).expect("present");
        (out, stats)
    }

    fn metrics() -> Option<CellMetrics> {
        Some(CellMetrics {
            width: 8,
            height: 16,
        })
    }

    fn red_image() -> Arc<Image> {
        Arc::new(Image::solid(32, 16, Rgb::new(255, 0, 0)))
    }

    fn present_images(
        presenter: &mut Presenter,
        canvas: &Canvas,
        images: &[ImagePlacement],
        backend: ImageBackend,
    ) -> (Vec<u8>, PresentStats) {
        let mut out = Vec::new();
        let stats = presenter
            .present_with(canvas, images, backend, metrics(), &mut out)
            .expect("present");
        (out, stats)
    }

    fn text_of(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
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
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = vec![ImagePlacement::new(Rect::new(3, 0, 4, 1), red_image())];

        let (bytes, stats) = present_images(&mut presenter, &canvas, &images, ImageBackend::Sixel);

        // Ten glyphs were composed; the four inside the image are reserved.
        assert_eq!(6, stats.cells_written);
        assert_eq!(1, stats.images_drawn);
        assert!(
            bytes.windows(2).any(|window| window == b"\x1bP"),
            "expected a sixel sequence"
        );
    }

    #[test]
    fn without_a_backend_the_reserved_cells_are_drawn_as_text() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = vec![ImagePlacement::new(Rect::new(3, 0, 4, 1), red_image())];

        let (bytes, stats) = present_images(&mut presenter, &canvas, &images, ImageBackend::None);

        assert_eq!(10, stats.cells_written, "the cell layer must own the area");
        assert_eq!(0, stats.images_drawn);
        assert!(!bytes.windows(2).any(|window| window == b"\x1bP"));
        assert!(text_of(&bytes).contains('3'));
    }

    #[test]
    fn removing_an_image_forces_a_full_repaint() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 4);
        canvas.put_str(0, 0, "0123456789", Style::PLAIN);
        let images = vec![ImagePlacement::new(Rect::new(3, 0, 4, 1), red_image())];

        let (_, first) = present_images(&mut presenter, &canvas, &images, ImageBackend::Sixel);
        assert!(first.full_redraw);

        let (_, second) = present_images(&mut presenter, &canvas, &[], ImageBackend::Sixel);
        assert!(
            second.full_redraw,
            "the region the image covered must be repainted"
        );
        // And the cells that were hidden are now written again.
        assert_eq!(10, second.cells_written);
    }

    #[test]
    fn unchanged_images_are_not_re_emitted() {
        let mut presenter = Presenter::new();
        let mut canvas = Canvas::new(20, 4);
        let images = vec![ImagePlacement::new(Rect::new(3, 0, 4, 1), red_image())];
        present_images(&mut presenter, &canvas, &images, ImageBackend::Sixel);

        canvas.put_char(18, 0, 'x', Style::PLAIN);
        let (bytes, stats) = present_images(&mut presenter, &canvas, &images, ImageBackend::Sixel);

        assert!(!stats.full_redraw);
        assert_eq!(0, stats.images_drawn, "the image did not change");
        assert!(text_of(&bytes).contains('x'));
        assert!(!bytes.windows(2).any(|window| window == b"\x1bP"));
    }
}
