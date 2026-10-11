//! Optional image assets for the interface.
//!
//! Grimmoire's world is ASCII and needs no images at all. When the development
//! flag asks for pixel interface art, the pictures are drawn from the
//! source-controlled art in [`crate::pixel::art`] and encoded to PNG in memory —
//! there are no image files, so there is no third-party imagery to track and no
//! missing file to break the game.
//!
//! This module is the only place that turns a sprite into bytes. The renderer
//! handles sprites as [`crate::render::AssetId`]s and never sees data, and the
//! terminal layer owns transmission; nothing here knows about the protocol.

use crate::pixel::Sprite;
use crate::pixel::art;
use crate::render::{AssetId, png};

/// A generated image, ready to transmit.
#[derive(Debug, Clone)]
pub struct Asset {
    id: AssetId,
    name: String,
    png: Vec<u8>,
}

impl Asset {
    /// The handle used to refer to this asset.
    #[must_use]
    pub const fn id(&self) -> AssetId {
        self.id
    }

    /// A human-readable name, used in diagnostics and lookups.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The PNG bytes.
    #[must_use]
    pub fn png(&self) -> &[u8] {
        &self.png
    }
}

/// Every asset available to the application.
#[derive(Debug, Clone, Default)]
pub struct Assets {
    items: Vec<Asset>,
}

impl Assets {
    /// Renders every image the game can display.
    ///
    /// Called once at startup. The scene itself is not an asset — it is produced
    /// fresh each frame and travels through [`crate::render::SceneFrame`] — so
    /// what is built here is the small set of pictures that never change.
    #[must_use]
    pub fn built() -> Assets {
        Assets {
            items: vec![Asset {
                id: AssetId::new(0),
                name: "portrait-hooded".to_string(),
                png: encode(art().portrait()),
            }],
        }
    }

    /// The asset with this id, if it exists.
    #[must_use]
    pub fn get(&self, id: AssetId) -> Option<&Asset> {
        self.items.iter().find(|asset| asset.id == id)
    }

    /// The asset with this name, if it exists.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&Asset> {
        self.items.iter().find(|asset| asset.name == name)
    }

    /// Every loaded asset.
    #[must_use]
    pub fn all(&self) -> &[Asset] {
        &self.items
    }

    /// How many assets there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether there are no assets.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The total size of the encoded data in bytes.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.items.iter().map(|asset| asset.png.len()).sum()
    }

    /// Builds a store from raw payloads, for tests that need distinguishable
    /// images without rendering real art.
    #[cfg(test)]
    pub(crate) fn from_raw(items: Vec<(&str, Vec<u8>)>) -> Assets {
        Assets {
            items: items
                .into_iter()
                .enumerate()
                .map(|(index, (name, png))| Asset {
                    id: AssetId::new(index as u32),
                    name: name.to_string(),
                    png,
                })
                .collect(),
        }
    }
}

/// Encodes a sprite as PNG.
///
/// Transparent pixels become a near-black ground, so the image is opaque and can
/// sit under the interface without the terminal's own background showing.
fn encode(sprite: &Sprite) -> Vec<u8> {
    const GROUND: crate::pixel::Rgb = crate::pixel::Rgb::new(0x0a, 0x09, 0x10);
    let mut canvas = crate::pixel::PixelCanvas::new(sprite.width(), sprite.height(), GROUND);
    sprite.draw(&mut canvas, 0, 0);
    png::encode_rgb(canvas.width(), canvas.height(), &canvas.as_rgb_bytes())
}

#[cfg(test)]
mod tests {
    use super::{Asset, Assets};
    use crate::render::{AssetId, png};

    #[test]
    fn the_built_assets_include_a_portrait() {
        let assets = Assets::built();
        assert!(!assets.is_empty());
        let portrait = assets.named("portrait-hooded").expect("portrait");
        assert!(portrait.png().starts_with(&png::SIGNATURE));
        assert!(
            portrait.png().len() > 100,
            "the portrait should have content"
        );
        assert_eq!(
            portrait.id(),
            assets.get(portrait.id()).expect("by id").id()
        );
        assert!(assets.total_bytes() > 0);
    }

    #[test]
    fn ids_are_stable_and_lookups_can_miss() {
        let assets = Assets::built();
        assert_eq!(
            AssetId::new(0),
            assets.named("portrait-hooded").expect("portrait").id()
        );
        assert!(assets.get(AssetId::new(99)).is_none());
        assert!(assets.named("nothing").is_none());
    }

    #[test]
    fn an_empty_store_is_valid() {
        let assets = Assets::default();
        assert!(assets.is_empty());
        assert_eq!(0, assets.len());
        assert_eq!(0, assets.total_bytes());
    }

    #[test]
    fn the_asset_type_carries_its_handle() {
        let assets = Assets::built();
        let asset: &Asset = &assets.all()[0];
        assert_eq!(asset.id(), assets.get(asset.id()).expect("by id").id());
        assert!(!asset.name().is_empty());
    }
}
