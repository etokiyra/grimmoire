//! Clipped, local-coordinate drawing surfaces.
//!
//! A [`CanvasView`] is a borrow of a region of a [`Canvas`] with its own
//! origin at `(0, 0)`. It is the mechanism for laying out independent visual
//! regions: each panel or overlay draws in local coordinates and can never write
//! outside its own rectangle, even if the drawing code is sloppy.

use super::{Canvas, Cell, Rect, Style, glyph};

/// A clipped, local-coordinate window onto a [`Canvas`].
pub struct CanvasView<'a> {
    canvas: &'a mut Canvas,
    origin_x: u16,
    origin_y: u16,
    width: u16,
    height: u16,
}

impl<'a> CanvasView<'a> {
    /// Creates a view of `area` within `canvas`, clipped to its bounds.
    pub(super) fn new(canvas: &'a mut Canvas, area: Rect) -> Self {
        let visible = area.intersection(&canvas.bounds());
        CanvasView {
            canvas,
            origin_x: area.x,
            origin_y: area.y,
            width: visible.width,
            height: visible.height,
        }
    }

    /// The view width in local cells.
    #[must_use]
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// The view height in local cells.
    #[must_use]
    pub const fn height(&self) -> u16 {
        self.height
    }

    /// The full local area of the view, anchored at the origin.
    #[must_use]
    pub const fn bounds(&self) -> Rect {
        Rect::from_size(self.width, self.height)
    }

    /// Maps a local coordinate to the canvas, or `None` when outside the view.
    fn translate(&self, x: u16, y: u16) -> Option<(u16, u16)> {
        if x < self.width && y < self.height {
            Some((
                self.origin_x.saturating_add(x),
                self.origin_y.saturating_add(y),
            ))
        } else {
            None
        }
    }

    /// Maps a local rectangle to the canvas coordinate space.
    fn absolute(&self, area: Rect) -> Rect {
        Rect::new(
            self.origin_x.saturating_add(area.x),
            self.origin_y.saturating_add(area.y),
            area.width,
            area.height,
        )
    }

    /// Returns the cell at local `(x, y)`, or `None` when outside the view.
    #[must_use]
    pub fn get(&self, x: u16, y: u16) -> Option<Cell> {
        self.translate(x, y)
            .and_then(|(ax, ay)| self.canvas.get(ax, ay))
    }

    /// Writes `cell` at local `(x, y)`, doing nothing when outside the view.
    pub fn set(&mut self, x: u16, y: u16, cell: Cell) {
        if let Some((ax, ay)) = self.translate(x, y) {
            self.canvas.set(ax, ay, cell);
        }
    }

    /// Writes a drawable glyph at local `(x, y)`, ignoring other glyphs.
    pub fn put_char(&mut self, x: u16, y: u16, ch: char, style: Style) {
        if glyph::is_drawable(ch) {
            self.set(x, y, Cell::new(ch, style));
        }
    }

    /// Draws `text` from local `(x, y)`, returning the number of columns drawn.
    pub fn put_str(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16 {
        let mut column = x;
        for ch in text.chars() {
            if !glyph::is_drawable(ch) {
                continue;
            }
            self.put_char(column, y, ch, style);
            column = column.saturating_add(1);
        }
        column.saturating_sub(x)
    }

    /// Draws `text` horizontally centred within local `area` on row `y`.
    pub fn put_str_centered(&mut self, area: Rect, y: u16, text: &str, style: Style) -> u16 {
        let x = area.centered_x(glyph::display_width(text));
        self.put_str(x, y, text, style)
    }

    /// Fills local `area` with a copy of `cell`, clipped to the view.
    pub fn fill(&mut self, area: Rect, cell: Cell) {
        let local = area.intersection(&self.bounds());
        if local.is_empty() {
            return;
        }
        self.canvas.fill(self.absolute(local), cell);
    }

    /// Fills local `area` with a blank cell carrying `style`.
    pub fn fill_style(&mut self, area: Rect, style: Style) {
        self.fill(area, Cell::new(' ', style));
    }

    /// Draws an ASCII border around local `area`, clipped to the view.
    pub fn draw_border(&mut self, area: Rect, style: Style) {
        let local = area.intersection(&self.bounds());
        if local.is_empty() {
            return;
        }
        self.canvas.draw_border(self.absolute(local), style);
    }

    /// Draws an ASCII border around local `area` with `title` set into the top edge.
    pub fn draw_titled_border(
        &mut self,
        area: Rect,
        title: &str,
        border_style: Style,
        title_style: Style,
    ) {
        let local = area.intersection(&self.bounds());
        if local.is_empty() {
            return;
        }
        self.canvas
            .draw_titled_border(self.absolute(local), title, border_style, title_style);
    }

    /// Creates a nested view of `area`, expressed in this view's coordinates.
    ///
    /// The child is clipped to the parent, so nesting can only ever shrink the
    /// drawable region.
    pub fn sub(&mut self, area: Rect) -> CanvasView<'_> {
        let clipped = area.intersection(&self.bounds());
        CanvasView {
            canvas: &mut *self.canvas,
            origin_x: self.origin_x.saturating_add(clipped.x),
            origin_y: self.origin_y.saturating_add(clipped.y),
            width: clipped.width,
            height: clipped.height,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::render::{Canvas, Cell, Rect, Style};

    #[test]
    fn writes_are_translated_by_the_origin() {
        let mut canvas = Canvas::new(10, 4);
        {
            let mut view = canvas.view(Rect::new(2, 1, 4, 2));
            view.put_str(0, 0, "hi", Style::PLAIN);
        }
        assert_eq!(Some('h'), canvas.get(2, 1).map(|cell| cell.ch));
        assert_eq!(Some('i'), canvas.get(3, 1).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(0, 0).map(|cell| cell.ch));
    }

    #[test]
    fn drawing_outside_the_view_is_ignored() {
        let mut canvas = Canvas::new(10, 4);
        {
            let mut view = canvas.view(Rect::new(2, 1, 3, 2));
            // Column 3 is one past the view's width.
            view.put_str(3, 0, "X", Style::PLAIN);
            view.put_str(0, 2, "Y", Style::PLAIN);
        }
        assert_eq!(Some(' '), canvas.get(5, 1).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(2, 3).map(|cell| cell.ch));
    }

    #[test]
    fn view_is_clipped_to_the_canvas() {
        let mut canvas = Canvas::new(4, 2);
        let view = canvas.view(Rect::new(5, 5, 10, 10));
        assert_eq!(0, view.width());
        assert_eq!(0, view.height());
    }

    #[test]
    fn fill_is_clipped_to_the_view() {
        let mut canvas = Canvas::new(6, 3);
        let target = Cell::new('#', Style::PLAIN);
        {
            let mut view = canvas.view(Rect::new(1, 1, 2, 1));
            view.fill(Rect::new(0, 0, 100, 100), target);
        }
        assert_eq!(Some(target), canvas.get(1, 1));
        assert_eq!(Some(target), canvas.get(2, 1));
        assert_eq!(Some(' '), canvas.get(3, 1).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(1, 0).map(|cell| cell.ch));
    }

    #[test]
    fn nested_views_compose_offsets_and_clip() {
        let mut canvas = Canvas::new(10, 5);
        {
            let mut outer = canvas.view(Rect::new(1, 1, 6, 3));
            let mut inner = outer.sub(Rect::new(2, 1, 10, 10));
            assert_eq!(4, inner.width());
            assert_eq!(2, inner.height());
            // "inner" is clipped to the inner view's four columns.
            inner.put_str(0, 0, "inner", Style::PLAIN);
        }
        assert_eq!(Some('i'), canvas.get(3, 2).map(|cell| cell.ch));
        assert_eq!(Some('e'), canvas.get(6, 2).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(7, 2).map(|cell| cell.ch));
    }

    #[test]
    fn bordered_view_stays_inside_its_region() {
        let mut canvas = Canvas::new(8, 4);
        {
            let mut view = canvas.view(Rect::new(1, 1, 4, 2));
            view.draw_titled_border(Rect::new(0, 0, 10, 10), "t", Style::PLAIN, Style::PLAIN);
        }
        assert_eq!(Some('+'), canvas.get(1, 1).map(|cell| cell.ch));
        assert_eq!(Some('+'), canvas.get(4, 2).map(|cell| cell.ch));
        // Nothing leaked beyond the view region.
        assert_eq!(Some(' '), canvas.get(5, 1).map(|cell| cell.ch));
        assert_eq!(Some(' '), canvas.get(1, 0).map(|cell| cell.ch));
    }
}
