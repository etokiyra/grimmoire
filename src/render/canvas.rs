//! The cell grid that frames are composed into.

use super::{Cell, Color, Rect, Style, glyph, view::CanvasView};

/// A fixed-size grid of [`Cell`]s.
///
/// The canvas is intentionally dumb: it owns no layout policy and knows nothing
/// about the terminal. Drawing code composes a frame by writing cells, and the
/// terminal backend later diffs and paints that frame. Every write is clipped to
/// the grid, so callers may draw out of bounds safely.
///
/// # Coordinates
///
/// All coordinates are absolute, non-negative cell indices. Out-of-range writes
/// are ignored; arithmetic saturates rather than wrapping or panicking. There is
/// no negative coordinate space, so callers that need to shift a region must
/// clip before drawing.
///
/// # Unicode
///
/// The grid is one glyph per cell. The text helpers enforce the width policy in
/// [`glyph`]: glyphs that are not exactly one column wide are dropped. The raw
/// [`Canvas::set`] and [`Canvas::fill`] primitives store whatever cell they are
/// given, so callers using them directly are responsible for the policy.
#[derive(Debug, Clone)]
pub struct Canvas {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
}

impl Canvas {
    /// Creates a canvas of `width` by `height` blank cells.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Canvas {
            width,
            height,
            cells: vec![Cell::BLANK; cell_count(width, height)],
        }
    }

    /// The canvas width in cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// The canvas height in cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// The full area covered by this canvas.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        Rect::from_size(self.width, self.height)
    }

    /// Translates a coordinate into a linear index, or `None` when out of bounds.
    fn index(&self, x: u16, y: u16) -> Option<usize> {
        if x < self.width && y < self.height {
            Some(usize::from(y) * usize::from(self.width) + usize::from(x))
        } else {
            None
        }
    }

    /// Returns the cell at `(x, y)`, or `None` when out of bounds.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<Cell> {
        self.index(x, y).map(|index| self.cells[index])
    }

    /// Writes `cell` at `(x, y)`, doing nothing when the coordinate is off-canvas.
    ///
    /// # Background inheritance
    ///
    /// A cell whose style leaves the background at [`Color::Default`] inherits the
    /// background already in the grid. This is what makes the viewport opaque:
    /// the application fills the frame with the theme background once, and every
    /// later draw — text, borders, art — keeps it unless it asks for a different
    /// background. Without this, a foreground-only draw would reset the
    /// background to the terminal's own, letting the user's theme or desktop show
    /// through the game.
    ///
    /// The raw cell is otherwise stored exactly as given.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(index) = self.index(x, y) {
            let mut cell = cell;
            if cell.style.bg == Color::Default {
                cell.style.bg = self.cells[index].style.bg;
            }
            self.cells[index] = cell;
        }
    }

    /// Writes a single glyph at `(x, y)`, ignoring non-drawable glyphs.
    ///
    /// See [`glyph::is_drawable`]. Use [`Canvas::set`] to store a cell verbatim.
    pub fn put_char(&mut self, x: u16, y: u16, ch: char, style: Style) {
        if glyph::is_drawable(ch) {
            self.set(x, y, Cell::new(ch, style));
        }
    }

    /// Draws `text` starting at `(x, y)`, returning the number of columns drawn.
    ///
    /// Glyphs that are not exactly one column wide (control characters, combining
    /// marks, double-width glyphs) are dropped. Text that runs past the right edge
    /// is clipped.
    pub fn put_str(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let mut column = x;
        for ch in text.chars() {
            if !glyph::is_drawable(ch) {
                continue;
            }
            self.set(column, y, Cell::new(ch, style));
            column = column.saturating_add(1);
        }
        column.saturating_sub(x)
    }

    /// Draws `text` horizontally centred within `area` on row `y`.
    ///
    /// Returns the number of columns drawn.
    pub fn put_str_centered(&mut self, area: Rect, y: u16, text: &str, style: Style) -> u16 {
        let x = area.centered_x(glyph::display_width(text));
        self.put_str(x, y, text, style)
    }

    /// Fills every cell of `area` with a copy of `cell`, clipped to the canvas.
    pub fn fill(&mut self, area: Rect, cell: Cell) {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                self.set(x, y, cell);
            }
        }
    }

    /// Fills every cell of `area` with a blank cell carrying `style`.
    ///
    /// Useful for painting a background colour behind other content.
    pub fn fill_style(&mut self, area: Rect, style: Style) {
        self.fill(area, Cell::new(' ', style));
    }

    /// Resets every cell to [`Cell::BLANK`].
    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
    }

    /// Resets every cell to a copy of `cell`.
    pub fn clear_with(&mut self, cell: Cell) {
        self.cells.fill(cell);
    }

    /// Reallocates to `width` by `height`, discarding all previous content.
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.cells.clear();
        self.cells.resize(cell_count(width, height), Cell::BLANK);
    }

    /// Copies `other` into this canvas, resizing and reusing the allocation.
    pub fn copy_from(&mut self, other: &Canvas) {
        self.width = other.width;
        self.height = other.height;
        if self.cells.len() == other.cells.len() {
            self.cells.copy_from_slice(&other.cells);
        } else {
            self.cells.clear();
            self.cells.extend_from_slice(&other.cells);
        }
    }

    /// Draws an ASCII border around `area` using `+`, `-` and `|`.
    ///
    /// Grimmoire favours plain ASCII over box-drawing glyphs so that the visuals
    /// survive the most limited terminals and font setups. Areas smaller than
    /// two cells on either axis are left untouched.
    pub fn draw_border(&mut self, area: Rect, style: Style) {
        if area.width < 2 || area.height < 2 {
            return;
        }
        let right = area.right() - 1;
        let bottom = area.bottom() - 1;

        for x in area.x..=right {
            self.put_char(x, area.y, '-', style);
            self.put_char(x, bottom, '-', style);
        }
        for y in area.y..=bottom {
            self.put_char(area.x, y, '|', style);
            self.put_char(right, y, '|', style);
        }
        for (x, y, ch) in [
            (area.x, area.y, '+'),
            (right, area.y, '+'),
            (area.x, bottom, '+'),
            (right, bottom, '+'),
        ] {
            self.put_char(x, y, ch, style);
        }
    }

    /// Draws an ASCII border around `area` with `title` set into the top edge.
    pub fn draw_titled_border(
        &mut self,
        area: Rect,
        title: &str,
        border_style: Style,
        title_style: Style,
    ) {
        self.draw_border(area, border_style);
        let label_width = glyph::display_width(title).saturating_add(2);
        if area.width < label_width.saturating_add(2) {
            return;
        }
        let mut x = area.centered_x(label_width);
        x = x.saturating_add(self.put_str(x, area.y, " ", title_style));
        x = x.saturating_add(self.put_str(x, area.y, title, title_style));
        self.put_str(x, area.y, " ", title_style);
    }

    /// Copies the region `area` into a new canvas.
    ///
    /// The result is sized to `area` clipped to the canvas bounds, so it is
    /// always safe; parts of `area` that fall outside the canvas are omitted.
    #[must_use]
    pub fn crop(&self, area: Rect) -> Canvas {
        let visible = area.intersection(&self.bounds());
        let mut out = Canvas::new(visible.width, visible.height);
        for y in 0..visible.height {
            for x in 0..visible.width {
                if let Some(cell) = self.get(visible.x + x, visible.y + y) {
                    out.set(x, y, cell);
                }
            }
        }
        out
    }

    /// Paints `src` onto this canvas with its origin at `(at_x, at_y)`.
    ///
    /// Composition is paint order: whatever is blitted later wins where regions
    /// overlap. Returns the destination region that was actually written (clipped
    /// to this canvas), which may be empty.
    pub fn blit(&mut self, src: &Canvas, at_x: u16, at_y: u16) -> Rect {
        let dest = Rect::new(at_x, at_y, src.width(), src.height()).intersection(&self.bounds());
        for y in 0..dest.height {
            for x in 0..dest.width {
                if let Some(cell) = src.get(x, y) {
                    self.set(dest.x + x, dest.y + y, cell);
                }
            }
        }
        dest
    }

    /// Creates a clipped, local-coordinate drawing surface for `area`.
    ///
    /// The view never writes outside `area` intersected with the canvas bounds,
    /// so independent UI regions can be drawn without leaking into each other.
    pub fn view(&mut self, area: Rect) -> CanvasView<'_> {
        CanvasView::new(self, area)
    }
}

/// The number of cells for the given dimensions, saturating instead of overflowing.
fn cell_count(width: u16, height: u16) -> usize {
    usize::from(width).saturating_mul(usize::from(height))
}

#[cfg(test)]
mod tests {
    use super::{Canvas, Cell, Rect, Style};
    use crate::render::Color;

    #[test]
    fn new_canvas_is_all_blank() {
        let canvas = Canvas::new(3, 2);
        assert_eq!(3, canvas.width());
        assert_eq!(2, canvas.height());
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
        assert_eq!(Some(Cell::BLANK), canvas.get(2, 1));
    }

    #[test]
    fn get_and_set_are_clipped_to_bounds() {
        let mut canvas = Canvas::new(2, 2);
        let styled = Cell::new('x', Style::new().with_fg(Color::LightMagenta));
        canvas.set(1, 1, styled);
        canvas.set(2, 0, Cell::new('!', Style::PLAIN));

        assert_eq!(Some(styled), canvas.get(1, 1));
        assert_eq!(None, canvas.get(2, 0));
        assert_eq!(None, canvas.get(9, 9));
    }

    #[test]
    fn put_str_writes_left_to_right_and_clips() {
        let mut canvas = Canvas::new(3, 1);
        let used = canvas.put_str(1, 0, "abcd", Style::PLAIN);

        assert_eq!(4, used);
        assert_eq!(Some(' '), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('a'), canvas.get(1, 0).map(|cell| cell.ch));
        assert_eq!(Some('b'), canvas.get(2, 0).map(|cell| cell.ch));
        assert_eq!(None, canvas.get(3, 0));
    }

    #[test]
    fn put_str_drops_control_and_wide_glyphs() {
        let mut canvas = Canvas::new(4, 1);
        let used = canvas.put_str(0, 0, "a\tb\u{4e2d}c", Style::PLAIN);

        assert_eq!(3, used);
        assert_eq!(Some('a'), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('b'), canvas.get(1, 0).map(|cell| cell.ch));
        assert_eq!(Some('c'), canvas.get(2, 0).map(|cell| cell.ch));
    }

    #[test]
    fn centered_text_is_horizontally_balanced() {
        let mut canvas = Canvas::new(10, 1);
        let used = canvas.put_str_centered(canvas.bounds(), 0, "abc", Style::PLAIN);

        assert_eq!(3, used);
        // 10 wide, 3 glyphs => 7 columns of slack, half of which is 3.
        assert_eq!(Some('a'), canvas.get(3, 0).map(|cell| cell.ch));
        assert_eq!(Some('c'), canvas.get(5, 0).map(|cell| cell.ch));
    }

    #[test]
    fn fill_replaces_only_the_requested_region() {
        let mut canvas = Canvas::new(4, 4);
        let target = Cell::new('#', Style::new().with_fg(Color::Green));
        canvas.fill(Rect::new(1, 1, 2, 2), target);

        assert_eq!(Some(target), canvas.get(1, 1));
        assert_eq!(Some(target), canvas.get(2, 2));
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
        assert_eq!(Some(Cell::BLANK), canvas.get(3, 3));
    }

    #[test]
    fn clear_restores_blank_cells() {
        let mut canvas = Canvas::new(2, 2);
        canvas.put_char(0, 0, 'z', Style::new().bold());
        canvas.clear();
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
    }

    #[test]
    fn draw_border_writes_ascii_frame() {
        let mut canvas = Canvas::new(5, 3);
        canvas.draw_border(canvas.bounds(), Style::PLAIN);

        assert_eq!(Some('+'), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('+'), canvas.get(4, 0).map(|cell| cell.ch));
        assert_eq!(Some('+'), canvas.get(0, 2).map(|cell| cell.ch));
        assert_eq!(Some('+'), canvas.get(4, 2).map(|cell| cell.ch));
        assert_eq!(Some('-'), canvas.get(2, 0).map(|cell| cell.ch));
        assert_eq!(Some('|'), canvas.get(0, 1).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(2, 1).map(|cell| cell.ch));
    }

    #[test]
    fn draw_titled_border_sets_the_title_into_the_top_edge() {
        let mut canvas = Canvas::new(12, 3);
        canvas.draw_titled_border(
            canvas.bounds(),
            "hi",
            Style::new().with_fg(Color::DarkGrey),
            Style::new().with_fg(Color::LightCyan),
        );
        let top: String = (0..12)
            .filter_map(|x| canvas.get(x, 0))
            .map(|cell| cell.ch)
            .collect();
        assert!(top.contains(" hi "), "top edge was {top:?}");
    }

    #[test]
    fn draw_border_ignores_areas_smaller_than_two_cells() {
        let mut canvas = Canvas::new(4, 1);
        canvas.draw_border(canvas.bounds(), Style::PLAIN);
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
    }

    #[test]
    fn crop_returns_a_clipped_copy() {
        let mut canvas = Canvas::new(4, 4);
        canvas.put_str(0, 0, "abcd", Style::PLAIN);
        canvas.put_str(0, 1, "efgh", Style::PLAIN);

        let sub = canvas.crop(Rect::new(1, 0, 2, 3));
        assert_eq!(2, sub.width());
        assert_eq!(3, sub.height());
        assert_eq!(Some('b'), sub.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('c'), sub.get(1, 0).map(|cell| cell.ch));
        assert_eq!(Some('f'), sub.get(0, 1).map(|cell| cell.ch));
    }

    #[test]
    fn crop_clamps_to_the_canvas() {
        let canvas = Canvas::new(3, 3);
        let sub = canvas.crop(Rect::new(2, 2, 10, 10));
        assert_eq!(1, sub.width());
        assert_eq!(1, sub.height());
    }

    #[test]
    fn blit_paints_with_paint_order_and_clips() {
        let mut canvas = Canvas::new(5, 3);
        let mut stamp = Canvas::new(3, 2);
        stamp.put_str(0, 0, "XYZ", Style::PLAIN);
        stamp.put_str(0, 1, "xyz", Style::PLAIN);

        let written = canvas.blit(&stamp, 3, 1);
        // Only the first column of the stamp fits to the right of x=3.
        assert_eq!(Rect::new(3, 1, 2, 2), written);
        assert_eq!(Some('X'), canvas.get(3, 1).map(|cell| cell.ch));
        assert_eq!(Some('Y'), canvas.get(4, 1).map(|cell| cell.ch));
        assert_eq!(Some('x'), canvas.get(3, 2).map(|cell| cell.ch));
        assert_eq!(None, canvas.get(5, 1));
    }

    #[test]
    fn blit_later_wins() {
        let mut canvas = Canvas::new(2, 1);
        let mut under = Canvas::new(2, 1);
        under.put_str(0, 0, "aa", Style::PLAIN);
        let mut over = Canvas::new(1, 1);
        over.put_str(0, 0, "b", Style::PLAIN);

        canvas.blit(&under, 0, 0);
        canvas.blit(&over, 1, 0);
        assert_eq!(Some('a'), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('b'), canvas.get(1, 0).map(|cell| cell.ch));
    }

    #[test]
    fn resize_discards_content_and_reuses_space() {
        let mut canvas = Canvas::new(4, 4);
        canvas.put_str(0, 0, "test", Style::PLAIN);
        canvas.resize(2, 2);
        assert_eq!(2, canvas.width());
        assert_eq!(2, canvas.height());
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
    }

    #[test]
    fn copy_from_replaces_dimensions_and_content() {
        let mut canvas = Canvas::new(1, 1);
        let mut other = Canvas::new(2, 1);
        other.put_str(0, 0, "ok", Style::PLAIN);

        canvas.copy_from(&other);
        assert_eq!(2, canvas.width());
        assert_eq!(Some('o'), canvas.get(0, 0).map(|cell| cell.ch));
    }

    #[test]
    fn a_foreground_only_draw_inherits_the_surface_background() {
        let mut canvas = Canvas::new(4, 2);
        let surface = Color::Indexed(233);
        canvas.clear_with(Cell::new(' ', Style::new().with_bg(surface)));

        canvas.put_str(0, 0, "hi", Style::new().with_fg(Color::LightMagenta));

        assert_eq!(Some(surface), canvas.get(0, 0).map(|cell| cell.style.bg));
        assert_eq!(Some(surface), canvas.get(1, 0).map(|cell| cell.style.bg));
    }

    #[test]
    fn an_explicit_background_wins_over_the_surface() {
        let mut canvas = Canvas::new(2, 1);
        canvas.clear_with(Cell::new(' ', Style::new().with_bg(Color::Indexed(233))));

        canvas.put_char(0, 0, 'x', Style::new().with_bg(Color::Red));

        assert_eq!(Some(Color::Red), canvas.get(0, 0).map(|cell| cell.style.bg));
    }

    #[test]
    fn the_whole_viewport_can_be_painted_opaque() {
        let mut canvas = Canvas::new(6, 3);
        let backdrop = Cell::new(' ', Style::new().with_bg(Color::Black));
        canvas.clear_with(backdrop);
        // Foreground-only content must not punch a hole in the background.
        canvas.put_str(1, 1, "abc", Style::new().with_fg(Color::White));
        canvas.draw_border(canvas.bounds(), Style::new().with_fg(Color::DarkGrey));

        for y in 0..3 {
            for x in 0..6 {
                assert_eq!(
                    Some(Color::Black),
                    canvas.get(x, y).map(|cell| cell.style.bg),
                    "cell ({x}, {y}) leaked the terminal's default background"
                );
            }
        }
    }
}
