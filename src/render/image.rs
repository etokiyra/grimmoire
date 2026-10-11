//! Handles for image assets.
//!
//! The renderer describes *which* image belongs in a rectangle, never the image
//! bytes. Those live in [`crate::assets`], and only the graphics backend turns a
//! handle into protocol output.

/// Identifies a loaded image asset without carrying its data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(u32);

impl AssetId {
    /// Creates a handle. Loaders assign these; nothing else should.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        AssetId(index)
    }

    /// The underlying index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::AssetId;

    #[test]
    fn handles_compare_by_index() {
        assert_eq!(AssetId::new(3), AssetId::new(3));
        assert_ne!(AssetId::new(3), AssetId::new(4));
        assert_eq!(3, AssetId::new(3).index());
    }
}
