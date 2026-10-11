//! Frame composition: the cell canvas plus any image placements.
//!
//! A [`Frame`] is what a screen composes into. It deliberately bundles two kinds
//! of content that are rendered by very different machinery:
//!
//! * **cells** — ordinary character cells, drawn through [`Canvas`] views; and
//! * **images** — rectangles a graphics backend may paint, recorded as intent.
//!
//! A screen never emits graphics bytes and never sees image data: it names an
//! asset handle and a rectangle. Whether those pixels are painted, and how, is
//! the graphics backend's business, which is what keeps protocol details out of
//! the game and out of the UI.

use super::{AssetId, Canvas, CanvasView, Rect};

/// An image to be drawn inside a rectangle of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePlacement {
    /// Where, in cells, the image should appear.
    pub area: Rect,
    /// Which asset to draw.
    pub asset: AssetId,
}

impl ImagePlacement {
    /// Creates a placement.
    #[must_use]
    pub const fn new(area: Rect, asset: AssetId) -> Self {
        ImagePlacement { area, asset }
    }
}

/// The surface a screen composes into during a frame.
pub struct Frame<'a> {
    canvas: &'a mut Canvas,
    images: &'a mut Vec<ImagePlacement>,
}

impl<'a> Frame<'a> {
    /// Starts a frame, clearing any placements from the previous one.
    pub fn begin(canvas: &'a mut Canvas, images: &'a mut Vec<ImagePlacement>) -> Self {
        images.clear();
        Frame { canvas, images }
    }

    /// The frame width in cells.
    #[must_use]
    pub fn width(&self) -> u16 {
        self.canvas.width()
    }

    /// The frame height in cells.
    #[must_use]
    pub fn height(&self) -> u16 {
        self.canvas.height()
    }

    /// The full frame area.
    #[must_use]
    pub fn bounds(&self) -> Rect {
        self.canvas.bounds()
    }

    /// Direct access to the cell canvas.
    pub fn canvas(&mut self) -> &mut Canvas {
        self.canvas
    }

    /// A clipped, local-coordinate view of `area`.
    pub fn view(&mut self, area: Rect) -> CanvasView<'_> {
        self.canvas.view(area)
    }

    /// Records that `asset` belongs at `area`.
    ///
    /// The placement is intent only. A screen must compose the same rectangle
    /// sensibly without the image (see `docs/renderer-design.md`), because the
    /// backend is allowed to paint nothing there at all.
    pub fn place_image(&mut self, area: Rect, asset: AssetId) {
        if area.is_empty() {
            return;
        }
        self.images.push(ImagePlacement::new(area, asset));
    }

    /// The placements recorded so far, in the order they were requested.
    #[must_use]
    pub fn images(&self) -> &[ImagePlacement] {
        self.images
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, ImagePlacement};
    use crate::render::{AssetId, Canvas, Rect, Style};

    #[test]
    fn begin_clears_previous_placements() {
        let mut canvas = Canvas::new(20, 10);
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            frame.place_image(Rect::new(0, 0, 4, 4), AssetId::new(1));
            assert_eq!(1, frame.images().len());
        }
        {
            let frame = Frame::begin(&mut canvas, &mut images);
            assert!(frame.images().is_empty());
        }
    }

    #[test]
    fn cells_and_images_compose_together() {
        let mut canvas = Canvas::new(20, 10);
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            let mut view = frame.view(Rect::new(1, 1, 6, 3));
            view.put_str(0, 0, "hi", Style::PLAIN);
            frame.place_image(Rect::new(10, 2, 6, 3), AssetId::new(2));
        }
        assert_eq!(Some('h'), canvas.get(1, 1).map(|cell| cell.ch));
        assert_eq!(1, images.len());
        assert_eq!(
            ImagePlacement::new(Rect::new(10, 2, 6, 3), AssetId::new(2)),
            images[0]
        );
    }

    #[test]
    fn empty_areas_are_not_placed() {
        let mut canvas = Canvas::new(20, 10);
        let mut images = Vec::new();
        let mut frame = Frame::begin(&mut canvas, &mut images);
        frame.place_image(Rect::new(0, 0, 0, 4), AssetId::new(1));
        assert!(frame.images().is_empty());
    }
}
