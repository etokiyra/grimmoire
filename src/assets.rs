//! Image assets.
//!
//! The only module in the crate that reads image files. Assets are loaded once at
//! startup, validated, and handed out as [`AssetId`] handles so that the renderer
//! and the graphics state never carry image bytes around.
//!
//! The artwork itself is original: `tools/generate-assets.sh` produces every PNG
//! in `assets/` procedurally, so the repository contains no third-party imagery
//! and no image decoder is needed. See `assets/README.md`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::render::AssetId;

/// The eight-byte signature every PNG file starts with.
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// A loaded image.
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

    /// A human-readable name, used in diagnostics and error messages.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The PNG bytes, exactly as they were read.
    #[must_use]
    pub fn png(&self) -> &[u8] {
        &self.png
    }
}

/// Every asset available to the application.
///
/// Ids are assigned in filename order so a given installation is stable.
#[derive(Debug, Clone, Default)]
pub struct Assets {
    items: Vec<Asset>,
}

impl Assets {
    /// Loads every `.png` in `dir`, in filename order.
    ///
    /// Fails on an unreadable file or one that is not a PNG, naming the file, so
    /// a broken installation is reported rather than rendering blank frames.
    pub fn load_dir(dir: &Path) -> io::Result<Assets> {
        let entries = fs::read_dir(dir).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "could not read the asset directory {}: {error}",
                    dir.display()
                ),
            )
        })?;

        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
            })
            .collect();
        paths.sort();

        let mut items = Vec::with_capacity(paths.len());
        for (index, path) in paths.iter().enumerate() {
            let png = fs::read(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("could not read the asset {}: {error}", path.display()),
                )
            })?;
            if !png.starts_with(&PNG_SIGNATURE) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("the asset {} is not a PNG file", path.display()),
                ));
            }
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            items.push(Asset {
                id: AssetId::new(index as u32),
                name,
                png,
            });
        }

        Ok(Assets { items })
    }

    /// The asset with this id, if it was loaded.
    #[must_use]
    pub fn get(&self, id: AssetId) -> Option<&Asset> {
        self.items.iter().find(|asset| asset.id == id)
    }

    /// The asset with this name, if it was loaded.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&Asset> {
        self.items.iter().find(|asset| asset.name == name)
    }

    /// Every loaded asset.
    #[must_use]
    pub fn all(&self) -> &[Asset] {
        &self.items
    }

    /// How many assets were loaded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether no assets were loaded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// The total size of the loaded PNG data in bytes.
    #[must_use]
    pub fn total_bytes(&self) -> usize {
        self.items.iter().map(|asset| asset.png.len()).sum()
    }

    /// Where the assets live, if a directory can be found.
    ///
    /// Prefers `$GRIMMOIRE_ASSETS`, then `assets/` beside the executable, then
    /// `assets/` in the working directory. Returns `None` when none exists.
    #[must_use]
    pub fn default_dir() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("GRIMMOIRE_ASSETS") {
            let dir = PathBuf::from(dir);
            if dir.is_dir() {
                return Some(dir);
            }
        }
        if let Ok(executable) = std::env::current_exe() {
            if let Some(parent) = executable.parent() {
                let candidate = parent.join("assets");
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }
        let candidate = PathBuf::from("assets");
        candidate.is_dir().then_some(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::{Assets, PNG_SIGNATURE};
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("grimmoire-assets-{tag}-{unique}"))
    }

    fn tiny_png() -> Vec<u8> {
        let mut png = PNG_SIGNATURE.to_vec();
        png.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D']);
        png
    }

    #[test]
    fn assets_are_loaded_in_filename_order_with_stable_ids() {
        let dir = scratch("load");
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("bravo.png"), tiny_png()).expect("write");
        fs::write(dir.join("alpha.png"), tiny_png()).expect("write");
        fs::write(dir.join("notes.txt"), b"ignored").expect("write");

        let assets = Assets::load_dir(&dir).expect("load");
        assert_eq!(2, assets.len(), "non-PNG files are ignored");
        assert_eq!("alpha", assets.all()[0].name());
        assert_eq!("bravo", assets.all()[1].name());
        assert_eq!(
            assets.all()[0].id(),
            assets.named("alpha").expect("named").id()
        );
        assert!(assets.get(assets.all()[0].id()).is_some());

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_file_that_is_not_a_png_is_rejected_by_name() {
        let dir = scratch("bad");
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("broken.png"), b"definitely not a png").expect("write");

        let error = Assets::load_dir(&dir).expect_err("should reject");
        assert!(error.to_string().contains("broken.png"));

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn a_missing_directory_is_an_error_naming_the_path() {
        let error = Assets::load_dir(&scratch("missing")).expect_err("should reject");
        assert!(error.to_string().contains("asset directory"));
    }
}
