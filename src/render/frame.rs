//! Frame composition: the cell canvas plus any image placements.
//!
//! A [`Frame`] is what a screen composes into. It deliberately bundles two kinds
//! of content that are rendered by very different machinery:
//!
//! * **cells** — ordinary character cells, drawn through [`Canvas`] views; and
//! * **images** — rectangles a graphics backend may paint, recorded as intent.
//!
//! A screen never emits graphics-protocol bytes; it only says "an image of this
//! size belongs here". Deciding whether and how to realise that is the terminal
//! layer's job, which is what keeps protocol details out of the game and out of
//! the UI.

use std::sync::Arc;

use super::{Canvas, CanvasView, Image, Rect};

/// An image to be drawn inside a rectangle of cells.
#[derive(Debug, Clone)]
pub struct ImagePlacement {
    /// Where, in cells, the image should appear.
    pub area: Rect,
    /// The image itself. Shared so a screen can hold one and place it repeatedly.
    pub image: Arc<Image>,
}

impl ImagePlacement {
    /// Creates a placement.
    #[must_use]
    pub fn new(area: Rect, image: Arc<Image>) -> Self {
        ImagePlacement { area, image }
    }

    /// A cheap identity for change detection.
    ///
    /// Two placements are considered the same content when they cover the same
    /// rectangle and share the same image allocation, which is the common case
    /// for a screen that keeps its art around between frames.
    #[must_use]
    pub fn key(&self) -> u64 {
        let ptr = Arc::as_ptr(&self.image) as usize as u64;
        let (x, y, w, h) = (
            u64::from(self.area.x),
            u64::from(self.area.y),
            u64::from(self.area.width),
            u64::from(self.area.height),
        );
        ptr ^ (x << 8) ^ (y << 24) ^ (w << 40) ^ (h << 48)
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

    /// Records that `image` belongs at `area`.
    ///
    /// The placement is intent only: whether anything is drawn depends on the
    /// terminal's capabilities, and the screen must compose the same rectangle
    /// sensibly with or without it (see `docs/renderer-design.md`).
    pub fn place_image(&mut self, area: Rect, image: Arc<Image>) {
        if area.is_empty() {
            return;
        }
        self.images.push(ImagePlacement::new(area, image));
    }

    /// The placements recorded so far.
    #[must_use]
    pub fn images(&self) -> &[ImagePlacement] {
        self.images
    }
}

#[cfg(test)]
mod tests {
    use super::{Frame, ImagePlacement};
    use crate::render::{Canvas, Image, Rect, Style};
    use std::sync::Arc;

    #[test]
    fn begin_clears_previous_placements() {
        let mut canvas = Canvas::new(20, 10);
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            frame.place_image(Rect::new(0, 0, 4, 4), Arc::new(Image::new(8, 8)));
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
            frame.place_image(Rect::new(10, 2, 6, 3), Arc::new(Image::new(48, 60)));
        }
        assert_eq!(Some('h'), canvas.get(1, 1).map(|cell| cell.ch));
        assert_eq!(1, images.len());
        assert_eq!(Rect::new(10, 2, 6, 3), images[0].area);
    }

    #[test]
    fn empty_areas_are_not_placed() {
        let mut canvas = Canvas::new(20, 10);
        let mut images = Vec::new();
        let mut frame = Frame::begin(&mut canvas, &mut images);
        frame.place_image(Rect::new(0, 0, 0, 4), Arc::new(Image::new(8, 8)));
        assert!(frame.images().is_empty());
    }

    #[test]
    fn placement_keys_track_rectangle_and_image_identity() {
        let image = Arc::new(Image::new(8, 8));
        let a = ImagePlacement::new(Rect::new(1, 1, 4, 4), image.clone());
        let same = ImagePlacement::new(Rect::new(1, 1, 4, 4), image.clone());
        let moved = ImagePlacement::new(Rect::new(2, 1, 4, 4), image.clone());
        let other = ImagePlacement::new(Rect::new(1, 1, 4, 4), Arc::new(Image::new(8, 8)));

        assert_eq!(a.key(), same.key());
        assert_ne!(a.key(), moved.key());
        assert_ne!(a.key(), other.key());
    }
}
