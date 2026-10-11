//! Choosing and driving a graphics backend.
//!
//! The renderer records *intent* — "an image belongs in this rectangle of cells"
//! — and this module decides whether that intent can be realised, and emits the
//! protocol bytes if so.
//!
//! # Capability states
//!
//! Detection is deliberately three-valued. Assuming support from an environment
//! variable would risk painting garbage into terminals that cannot interpret it,
//! so a backend is only selected when the evidence points at a protocol this
//! build actually implements (currently sixel) *and* the terminal reports the
//! cell metrics needed to size an image to a rectangle. Everything else falls
//! back to cells, which is always safe.

use super::capabilities::{CellMetrics, GraphicsProtocol, GraphicsSupport};
use super::sixel;
use crate::render::Image;

/// The graphics backend in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageBackend {
    /// No graphics: images are not drawn, and screens must look right without them.
    #[default]
    None,
    /// DEC sixel graphics.
    Sixel,
}

impl ImageBackend {
    /// Selects a backend from the detected support and available cell metrics.
    ///
    /// Only protocols with a real implementation are eligible. A protocol that
    /// was detected but not implemented (kitty, iTerm2) deliberately selects no
    /// backend rather than producing a half-working one.
    #[must_use]
    pub fn select(support: GraphicsSupport, metrics: Option<CellMetrics>) -> ImageBackend {
        match support {
            GraphicsSupport::Likely(GraphicsProtocol::Sixel) if metrics.is_some() => {
                ImageBackend::Sixel
            }
            _ => ImageBackend::None,
        }
    }

    /// Whether images can actually be drawn.
    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, ImageBackend::None)
    }

    /// A short label for diagnostics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ImageBackend::None => "none (cells only)",
            ImageBackend::Sixel => "sixel",
        }
    }

    /// Encodes `image` for this backend, or `None` when unencodable.
    ///
    /// The result is exactly the byte sequence to send, with no positioning; the
    /// presenter decides where the cursor goes first.
    #[must_use]
    pub fn encode(self, image: &Image) -> Option<Vec<u8>> {
        match self {
            ImageBackend::None => None,
            ImageBackend::Sixel => Some(sixel::encode(image)),
        }
    }

    /// Resizes `image` so it exactly fills `columns` by `rows` cells.
    #[must_use]
    pub fn resize_for(
        self,
        image: &Image,
        columns: u16,
        rows: u16,
        metrics: Option<CellMetrics>,
    ) -> Option<Image> {
        let metrics = metrics?;
        let width = u32::from(metrics.width) * u32::from(columns);
        let height = u32::from(metrics.height) * u32::from(rows);
        if width == 0 || height == 0 {
            return None;
        }
        Some(image.scaled(width, height))
    }
}

#[cfg(test)]
mod tests {
    use super::ImageBackend;
    use crate::render::{Image, Rgb};
    use crate::terminal::capabilities::{CellMetrics, GraphicsProtocol, GraphicsSupport};

    #[test]
    fn sixel_is_selected_only_with_metrics() {
        let metrics = CellMetrics::from_window(100, 30, 800, 600);
        assert_eq!(
            ImageBackend::Sixel,
            ImageBackend::select(GraphicsSupport::Likely(GraphicsProtocol::Sixel), metrics)
        );
        assert_eq!(
            ImageBackend::None,
            ImageBackend::select(GraphicsSupport::Likely(GraphicsProtocol::Sixel), None)
        );
    }

    #[test]
    fn unimplemented_protocols_fall_back_to_cells() {
        let metrics = CellMetrics::from_window(100, 30, 800, 600);
        assert_eq!(
            ImageBackend::None,
            ImageBackend::select(GraphicsSupport::Likely(GraphicsProtocol::Kitty), metrics)
        );
        assert_eq!(
            ImageBackend::None,
            ImageBackend::select(GraphicsSupport::Unknown, metrics)
        );
        assert_eq!(
            ImageBackend::None,
            ImageBackend::select(GraphicsSupport::Unsupported, metrics)
        );
    }

    #[test]
    fn resizing_matches_the_cell_rectangle() {
        let metrics = CellMetrics::from_window(100, 30, 800, 600);
        let image = Image::solid(4, 4, Rgb::new(255, 0, 0));
        let resized = ImageBackend::Sixel
            .resize_for(&image, 10, 5, metrics)
            .expect("resized");
        assert_eq!(80, resized.width());
        assert_eq!(100, resized.height());
    }

    #[test]
    fn encoding_is_available_only_for_a_real_backend() {
        let image = Image::solid(2, 2, Rgb::new(0, 255, 0));
        assert!(ImageBackend::None.encode(&image).is_none());
        assert!(ImageBackend::Sixel.encode(&image).is_some());
    }
}
