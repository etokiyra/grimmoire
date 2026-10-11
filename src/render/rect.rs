//! Axis-aligned rectangular regions used for layout.

/// An axis-aligned region of a canvas, measured in cells.
///
/// `Rect` carries layout intent only; it knows nothing about what is drawn.
/// All operations saturate instead of panicking, so malformed or tiny regions
/// degrade gracefully rather than crashing the renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    /// Left-most column, inclusive.
    pub x: u16,
    /// Top-most row, inclusive.
    pub y: u16,
    /// Width in cells.
    pub width: u16,
    /// Height in cells.
    pub height: u16,
}

impl Rect {
    /// Creates a rectangle from its origin and size.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a rectangle anchored at the origin.
    #[must_use]
    pub const fn from_size(width: u16, height: u16) -> Self {
        Rect {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Whether the given cell coordinate lies inside this rectangle.
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }

    /// Whether the rectangle encloses no cells.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The exclusive right edge (`x + width`), saturating at `u16::MAX`.
    #[must_use]
    pub const fn right(&self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// The exclusive bottom edge (`y + height`), saturating at `u16::MAX`.
    #[must_use]
    pub const fn bottom(&self) -> u16 {
        self.y.saturating_add(self.height)
    }

    /// The overlapping region shared with `other`, which may be empty.
    #[must_use]
    pub const fn intersection(&self, other: &Rect) -> Rect {
        let x = if self.x > other.x { self.x } else { other.x };
        let y = if self.y > other.y { self.y } else { other.y };
        let right = if self.right() < other.right() {
            self.right()
        } else {
            other.right()
        };
        let bottom = if self.bottom() < other.bottom() {
            self.bottom()
        } else {
            other.bottom()
        };
        Rect {
            x,
            y,
            width: right.saturating_sub(x),
            height: bottom.saturating_sub(y),
        }
    }

    /// Returns a copy shrunk by `margin` cells on every side.
    #[must_use]
    pub const fn inset(&self, margin: u16) -> Self {
        let double = margin.saturating_mul(2);
        Rect {
            x: self.x.saturating_add(margin),
            y: self.y.saturating_add(margin),
            width: self.width.saturating_sub(double),
            height: self.height.saturating_sub(double),
        }
    }

    /// The column at which `width` cells are centred horizontally within this rect.
    #[must_use]
    pub const fn centered_x(&self, width: u16) -> u16 {
        self.x.saturating_add(self.width.saturating_sub(width) / 2)
    }

    /// The row at which `height` cells are centred vertically within this rect.
    #[must_use]
    pub const fn centered_y(&self, height: u16) -> u16 {
        self.y
            .saturating_add(self.height.saturating_sub(height) / 2)
    }
}

#[cfg(test)]
mod tests {
    use super::Rect;

    #[test]
    fn contains_respects_bounds() {
        let rect = Rect::new(2, 3, 4, 5);
        assert!(rect.contains(2, 3));
        assert!(rect.contains(5, 7));
        assert!(!rect.contains(6, 7));
        assert!(!rect.contains(1, 3));
        assert!(!rect.contains(2, 8));
    }

    #[test]
    fn contains_is_false_for_empty_rect() {
        let rect = Rect::new(0, 0, 0, 0);
        assert!(!rect.contains(0, 0));
    }

    #[test]
    fn inset_shrinks_and_moves_origin() {
        assert_eq!(Rect::new(1, 1, 18, 8), Rect::new(0, 0, 20, 10).inset(1));
    }

    #[test]
    fn inset_saturates_for_oversized_margin() {
        assert_eq!(Rect::new(3, 3, 0, 0), Rect::new(0, 0, 2, 3).inset(3));
    }

    #[test]
    fn centering_places_odd_slack_on_the_left() {
        let rect = Rect::from_size(10, 10);
        assert_eq!(2, rect.centered_x(6));
        assert_eq!(4, rect.centered_y(2));
        // An item wider than the rect is clamped to the origin column.
        assert_eq!(0, rect.centered_x(99));
    }

    #[test]
    fn edges_are_exclusive() {
        let rect = Rect::new(2, 3, 4, 5);
        assert_eq!(6, rect.right());
        assert_eq!(8, rect.bottom());
        assert!(!rect.is_empty());
        assert!(Rect::new(2, 3, 0, 5).is_empty());
    }

    #[test]
    fn intersection_is_the_shared_region() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(4, 4, 10, 10);
        assert_eq!(Rect::new(4, 4, 6, 6), a.intersection(&b));
        assert_eq!(a.intersection(&b), b.intersection(&a));
    }

    #[test]
    fn intersection_of_disjoint_rects_is_empty() {
        let a = Rect::new(0, 0, 2, 2);
        let b = Rect::new(5, 5, 2, 2);
        assert!(a.intersection(&b).is_empty());
    }

    #[test]
    fn intersection_clips_an_oversized_rect() {
        let clip = Rect::new(1, 1, 3, 3);
        let big = Rect::new(0, 0, 100, 100);
        assert_eq!(clip, big.intersection(&clip));
    }
}
