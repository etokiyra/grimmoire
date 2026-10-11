//! The cell grid that frames are composed into.

use super::{Cell, Rect, Style};

/// A fixed-size grid of [`Cell`]s.
///
/// The canvas is intentionally dumb: it owns no layout policy and knows nothing
/// about the terminal. Drawing code composes a frame by writing cells, and the
/// terminal backend later diffing/painting that frame is its own concern. Every
/// write is clipped to the grid, so callers may draw out of bounds safely.
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
        let len = usize::from(width) * usize::from(height);
        Canvas {
            width,
            height,
            cells: vec![Cell::BLANK; len],
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
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some(index) = self.index(x, y) {
            self.cells[index] = cell;
        }
    }

    /// Writes a single glyph at `(x, y)` with `style`, clipped to the canvas.
    pub fn put_char(&mut self, x: u16, y: u16, ch: char, style: Style) {
        self.set(x, y, Cell::new(ch, style));
    }

    /// Draws `text` starting at `(x, y)`, returning the number of columns consumed.
    ///
    /// Control characters are replaced with spaces because emitting them would
    /// corrupt the frame.
    pub fn put_str(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let mut column = x;
        for ch in text.chars() {
            let ch = if ch.is_control() { ' ' } else { ch };
            self.put_char(column, y, ch, style);
            column = column.saturating_add(1);
        }
        column.saturating_sub(x)
    }

    /// Draws `text` horizontally centred within `area` on row `y`.
    ///
    /// Returns the number of columns consumed.
    pub fn put_str_centered(&mut self, area: Rect, y: u16, text: &str, style: Style) -> u16 {
        let width = text.chars().count() as u16;
        let x = area.centered_x(width);
        self.put_str(x, y, text, style)
    }

    /// Fills every cell of `area` with a clone of `cell`, clipped to the canvas.
    pub fn fill(&mut self, area: Rect, cell: Cell) {
        for y in area.y..area.y.saturating_add(area.height) {
            for x in area.x..area.x.saturating_add(area.width) {
                self.set(x, y, cell);
            }
        }
    }

    /// Resets every cell to [`Cell::BLANK`].
    pub fn clear(&mut self) {
        self.cells.fill(Cell::BLANK);
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
        let right = area.x + area.width - 1;
        let bottom = area.y + area.height - 1;

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
}

#[cfg(test)]
mod tests {
    use super::{Canvas, Cell, Rect, Style};
    use crate::render::Color;

    #[test]
    fn new_canvas_is_all_blank() {
        let canvas = Canvas::new(3, 2);
        assert_eq!(
            6,
            canvas.bounds().width as usize * canvas.bounds().height as usize
        );
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
    fn put_str_replaces_control_characters() {
        let mut canvas = Canvas::new(2, 1);
        canvas.put_str(0, 0, "\t\n", Style::PLAIN);
        assert_eq!(Some(' '), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(1, 0).map(|cell| cell.ch));
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
    fn draw_border_ignores_areas_smaller_than_two_cells() {
        let mut canvas = Canvas::new(4, 1);
        canvas.draw_border(canvas.bounds(), Style::PLAIN);
        assert_eq!(Some(Cell::BLANK), canvas.get(0, 0));
    }
}
