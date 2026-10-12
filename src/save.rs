//! Persistence for essential exploration state.
//!
//! The save format is a small, versioned, line-oriented text file. It is written
//! by hand rather than through a serialization dependency: the payload is a
//! handful of fields, the format needs to stay readable and stable, and a strict
//! parser makes a corrupt or future save fail safely instead of half-loading.
//!
//! Saving is **fail-soft**: if the file is missing, unreadable, or malformed,
//! loading simply returns `None` and the game starts fresh. Writing never replaces
//! the previous save until the new contents are safely on disk.
//!
//! What is persisted is the exploration state that cannot be reconstructed: the
//! area the player is in, their position, which areas they have visited, what
//! they have discovered, and how far the moonwell puzzle has progressed.
//!
//! # Versions and migration
//!
//! | Version | Fields | Read by |
//! | --- | --- | --- |
//! | 1 | `area`, `pos`, `visited` | every version |
//! | 2 | the above, plus `journal` and `braziers` | 2 and later |
//! | 3 | the above, plus `flags` | 3 and later |
//!
//! Files are always *written* at [`VERSION`]. Older files are still *read*: a
//! version 1 save loads with an empty journal and no braziers lit, which is
//! exactly the state that player was in when it was written. Migration is
//! therefore a widening rather than a conversion — nothing is reinterpreted, and
//! no valid save is ever discarded because the schema grew.
//!
//! A version 1 file that contains a version 2 field is rejected: it is not a file
//! the game ever wrote, and guessing what it meant is worse than starting fresh.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The first token of every save file.
const MAGIC: &str = "grimmoire-save";

/// The save format version. Bump this when the layout changes incompatibly.
pub const VERSION: u32 = 3;

/// The oldest save version this build still understands.
pub const OLDEST_SUPPORTED: u32 = 1;

/// The essential exploration state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveData {
    /// The slug of the area the player is in.
    pub area: String,
    /// The player's column.
    pub x: u16,
    /// The player's row.
    pub y: u16,
    /// The areas the player has visited.
    pub visited: Vec<String>,
    /// The discoveries recorded, in the order they were found.
    pub journal: Vec<String>,
    /// The moonwell braziers lit so far, in the order they were lit.
    pub braziers: Vec<String>,
    /// Story facts conversations established, such as who has been told what.
    pub flags: Vec<String>,
}

impl SaveData {
    /// Creates save data with no progression.
    ///
    /// This is the shape a version 1 save migrates to.
    #[must_use]
    pub fn new(area: impl Into<String>, x: u16, y: u16, visited: Vec<String>) -> Self {
        SaveData {
            area: area.into(),
            x,
            y,
            visited,
            journal: Vec::new(),
            braziers: Vec::new(),
            flags: Vec::new(),
        }
    }

    /// Adds the recorded progression.
    #[must_use]
    pub fn with_progress(
        mut self,
        journal: Vec<String>,
        braziers: Vec<String>,
        flags: Vec<String>,
    ) -> Self {
        self.journal = journal;
        self.braziers = braziers;
        self.flags = flags;
        self
    }
}

/// Renders save data as text, ending with a newline.
#[must_use]
pub fn encode(data: &SaveData) -> String {
    let mut out = String::with_capacity(128);
    out.push_str(MAGIC);
    out.push(' ');
    out.push_str(&VERSION.to_string());
    out.push('\n');
    out.push_str("area=");
    out.push_str(&data.area);
    out.push('\n');
    out.push_str("pos=");
    out.push_str(&data.x.to_string());
    out.push(' ');
    out.push_str(&data.y.to_string());
    out.push('\n');
    out.push_str("visited=");
    out.push_str(&data.visited.join(" "));
    out.push('\n');
    out.push_str("journal=");
    out.push_str(&data.journal.join(" "));
    out.push('\n');
    out.push_str("braziers=");
    out.push_str(&data.braziers.join(" "));
    out.push('\n');
    out.push_str("flags=");
    out.push_str(&data.flags.join(" "));
    out.push('\n');
    out
}

/// Parses save data, returning `None` for anything unexpected.
///
/// The parser is intentionally strict: an unknown field, a malformed number, a
/// missing field, a field the file's own version does not define, or an
/// unexpected trailing token all reject the whole file, so partially understood
/// state can never be loaded. Fields added in a later version are optional when
/// reading that version, so a save written by an older build of the same version
/// still loads.
#[must_use]
pub fn decode(text: &str) -> Option<SaveData> {
    let mut lines = text.lines();

    let mut header = lines.next()?.split_whitespace();
    if header.next()? != MAGIC {
        return None;
    }
    let version = header.next()?.parse::<u32>().ok()?;
    if !(OLDEST_SUPPORTED..=VERSION).contains(&version) {
        return None;
    }
    if header.next().is_some() {
        return None;
    }

    let mut area: Option<String> = None;
    let mut position: Option<(u16, u16)> = None;
    let mut visited: Option<Vec<String>> = None;
    let mut journal: Option<Vec<String>> = None;
    let mut braziers: Option<Vec<String>> = None;
    let mut flags: Option<Vec<String>> = None;

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=')?;
        match key {
            "area" => {
                if value.is_empty() {
                    return None;
                }
                area = Some(value.to_string());
            }
            "pos" => {
                let mut parts = value.split_whitespace();
                let x = parts.next()?.parse::<u16>().ok()?;
                let y = parts.next()?.parse::<u16>().ok()?;
                if parts.next().is_some() {
                    return None;
                }
                position = Some((x, y));
            }
            "visited" => {
                visited = Some(value.split_whitespace().map(str::to_string).collect());
            }
            // Progression fields exist from version 2 onwards. A version 1 file
            // carrying them was not written by this game.
            "journal" if version >= 2 => {
                journal = Some(value.split_whitespace().map(str::to_string).collect());
            }
            "braziers" if version >= 2 => {
                braziers = Some(value.split_whitespace().map(str::to_string).collect());
            }
            "flags" if version >= 3 => {
                flags = Some(value.split_whitespace().map(str::to_string).collect());
            }
            _ => return None,
        }
    }

    let (x, y) = position?;
    Some(SaveData {
        area: area?,
        x,
        y,
        visited: visited.unwrap_or_default(),
        journal: journal.unwrap_or_default(),
        braziers: braziers.unwrap_or_default(),
        flags: flags.unwrap_or_default(),
    })
}

/// A save file location.
///
/// The store is the only part of persistence that touches the filesystem, and it
/// takes an explicit path so tests never need the real user data directory.
#[derive(Debug, Clone)]
pub struct SaveStore {
    path: PathBuf,
}

impl SaveStore {
    /// Uses `path` as the save file.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        SaveStore { path: path.into() }
    }

    /// The conventional save location for the current user, if one can be found.
    ///
    /// Prefers `$XDG_DATA_HOME/grimmoire/save.txt` and falls back to
    /// `$HOME/.local/share/grimmoire/save.txt`. Returns `None` — disabling
    /// persistence rather than guessing — when neither is usable.
    #[must_use]
    pub fn default_location() -> Option<SaveStore> {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME").map(|home| Path::new(&home).join(".local").join("share"))
            })?;
        Some(SaveStore::at(base.join("grimmoire").join("save.txt")))
    }

    /// The file this store reads and writes.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the save, or `None` when it is absent, unreadable, or invalid.
    #[must_use]
    pub fn load(&self) -> Option<SaveData> {
        decode(&fs::read_to_string(&self.path).ok()?)
    }

    /// Writes the save, replacing any previous file.
    ///
    /// The data is written to a sibling temporary file and then renamed over the
    /// target, so an interrupted write cannot leave a half-written save behind.
    pub fn save(&self, data: &SaveData) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, encode(data))?;
        fs::rename(&temporary, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{SaveData, SaveStore, VERSION, decode, encode};

    fn sample() -> SaveData {
        SaveData::new("ruined-chapel", 23, 12, vec!["ruined-chapel".to_string()])
    }

    fn scratch(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("grimmoire-save-{tag}-{unique}"))
    }

    #[test]
    fn encode_and_decode_round_trip() {
        let data = sample();
        assert_eq!(Some(data.clone()), decode(&encode(&data)));
    }

    #[test]
    fn the_encoded_form_is_versioned_and_readable() {
        let text = encode(&sample());
        let mut lines = text.lines();
        assert_eq!(
            Some(format!("grimmoire-save {VERSION}")),
            lines.next().map(str::to_string)
        );
        assert!(text.contains("area=ruined-chapel"));
        assert!(text.contains("pos=23 12"));
    }

    #[test]
    fn decode_rejects_a_wrong_magic_or_version() {
        assert_eq!(None, decode("other-save 1\narea=a\npos=0 0\n"));
        assert_eq!(None, decode("grimmoire-save 999\narea=a\npos=0 0\n"));
        assert_eq!(None, decode("grimmoire-save\narea=a\npos=0 0\n"));
    }

    #[test]
    fn decode_rejects_malformed_fields() {
        assert_eq!(None, decode("grimmoire-save 1\narea=a\n")); // no position
        assert_eq!(None, decode("grimmoire-save 1\narea=\npos=0 0\n")); // empty area
        assert_eq!(None, decode("grimmoire-save 1\narea=a\npos=x 0\n"));
        assert_eq!(None, decode("grimmoire-save 1\narea=a\npos=0 0 0\n"));
        assert_eq!(
            None,
            decode("grimmoire-save 1\narea=a\npos=0 0\nmystery=1\n")
        );
        assert_eq!(
            None,
            decode("grimmoire-save 1\narea=a\npos=0 0\nnonsense\n")
        );
        assert_eq!(None, decode(""));
    }

    #[test]
    fn decode_treats_a_missing_visited_list_as_empty() {
        let data = decode("grimmoire-save 1\narea=a\npos=1 2\n").expect("decoded");
        assert!(data.visited.is_empty());
    }

    #[test]
    fn a_version_one_save_migrates_with_empty_progression() {
        // Exactly what version 1 wrote, from a real play session.
        let legacy = "grimmoire-save 1\narea=sunken-cellar\npos=20 12\nvisited=ruined-chapel sunken-cellar\n";
        let data = decode(legacy).expect("a version 1 save must still load");
        assert_eq!("sunken-cellar", data.area);
        assert_eq!((20, 12), (data.x, data.y));
        assert_eq!(2, data.visited.len());
        assert!(
            data.journal.is_empty(),
            "a version 1 player had discovered nothing"
        );
        assert!(data.braziers.is_empty());
    }

    #[test]
    fn the_newest_version_writes_every_field() {
        let text = encode(&sample());
        assert!(text.starts_with("grimmoire-save 3"));
        assert!(text.contains("\njournal="));
        assert!(text.contains("\nbraziers="));
        assert!(text.contains("\nflags="));
    }

    #[test]
    fn a_version_two_save_migrates_with_no_flags() {
        // Exactly what version 2 wrote.
        let two = "grimmoire-save 2\narea=hollow-village\npos=62 30\nvisited=ruined-chapel whispering-woods hollow-village\njournal=village-well\nbraziers=waning\n";
        let data = decode(two).expect("a version 2 save must still load");
        assert_eq!("hollow-village", data.area);
        assert_eq!(1, data.journal.len());
        assert_eq!(1, data.braziers.len());
        assert!(data.flags.is_empty(), "there were no flags in version 2");
    }

    #[test]
    fn a_version_two_file_carrying_a_flags_field_is_rejected() {
        let forged = "grimmoire-save 2\narea=a\npos=0 0\nflags=cassen-accused\n";
        assert_eq!(None, decode(forged));
    }

    #[test]
    fn progression_round_trips() {
        let data = sample().with_progress(
            vec!["village-well".to_string(), "woods-glade".to_string()],
            vec!["waning".to_string()],
            vec!["cassen-accused".to_string()],
        );
        assert_eq!(Some(data.clone()), decode(&encode(&data)));
    }

    #[test]
    fn a_version_one_file_carrying_a_newer_field_is_rejected() {
        let forged = "grimmoire-save 1\narea=a\npos=0 0\njournal=village-well\n";
        assert_eq!(
            None,
            decode(forged),
            "a file no version of the game could have written must not be guessed at"
        );
    }

    #[test]
    fn a_future_version_is_rejected_rather_than_guessed() {
        assert_eq!(None, decode("grimmoire-save 4\narea=a\npos=0 0\n"));
    }

    #[test]
    fn a_malformed_progression_field_is_rejected() {
        assert_eq!(
            None,
            decode("grimmoire-save 2\narea=a\npos=0 0\njournal=village-well\nmystery=1\n")
        );
        assert_eq!(
            None,
            decode("grimmoire-save 2\narea=a\npos=0 0\nnot a field\n")
        );
    }

    #[test]
    fn loading_a_missing_file_is_not_an_error() {
        let store = SaveStore::at(scratch("missing").join("save.txt"));
        assert_eq!(None, store.load());
    }

    #[test]
    fn saving_then_loading_round_trips_through_the_filesystem() {
        let directory = scratch("roundtrip");
        let store = SaveStore::at(directory.join("save.txt"));
        let data = SaveData::new(
            "sunken-cellar",
            17,
            1,
            vec!["ruined-chapel".to_string(), "sunken-cellar".to_string()],
        );

        store.save(&data).expect("save");
        assert_eq!(Some(data), store.load());

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn saving_replaces_the_previous_save() {
        let directory = scratch("replace");
        let store = SaveStore::at(directory.join("save.txt"));

        store.save(&sample()).expect("first save");
        let second = SaveData::new("sunken-cellar", 4, 5, vec!["sunken-cellar".to_string()]);
        store.save(&second).expect("second save");

        assert_eq!(Some(second), store.load());
        assert!(
            !store.path().with_extension("tmp").exists(),
            "the temporary file should have been renamed away"
        );

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn loading_a_corrupt_file_returns_none() {
        let directory = scratch("corrupt");
        let store = SaveStore::at(directory.join("save.txt"));
        fs::create_dir_all(&directory).expect("mkdir");
        fs::write(store.path(), "not a save at all").expect("write");

        assert_eq!(None, store.load());

        fs::remove_dir_all(&directory).expect("cleanup");
    }
}
