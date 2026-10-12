//! Player preferences, and the small file they live in.
//!
//! Deliberately **not** part of the save. A save is progress: where the player is
//! and what they have learned. Preferences belong to the person at the keyboard,
//! survive starting a new game, and are read before any save is touched. Keeping
//! them apart means clearing a save can never lose someone's accessibility
//! settings, and neither file can corrupt the other.
//!
//! Loading is fail-soft in the same way the save is: anything unrecognised means
//! "use the defaults", never a half-applied preference.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The first token of every settings file.
const MAGIC: &str = "grimmoire-settings";
/// The settings format version.
const VERSION: u32 = 1;

/// How much motion the player wants.
///
/// One setting, three honest answers. "Reduced" is not "off with a caveat": it
/// keeps the things that *tell the player something* — a pulse on the tile that
/// refused a step — and drops the things that are only atmosphere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Motion {
    /// Everything moves.
    #[default]
    Full,
    /// No drifting ambience and no long transitions; feedback still shows.
    Reduced,
    /// Nothing animates at all. The game is entirely playable.
    Off,
}

impl Motion {
    /// Every setting, in the order the menu cycles them.
    pub const ALL: [Motion; 3] = [Motion::Full, Motion::Reduced, Motion::Off];

    /// The stable id written to the settings file.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Motion::Full => "full",
            Motion::Reduced => "reduced",
            Motion::Off => "off",
        }
    }

    /// The setting with this id, if there is one.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Motion> {
        Motion::ALL.into_iter().find(|motion| motion.id() == id)
    }

    /// How the setting is named in the menu.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Motion::Full => "full",
            Motion::Reduced => "reduced",
            Motion::Off => "off",
        }
    }

    /// What it means, in one line.
    #[must_use]
    pub const fn note(self) -> &'static str {
        match self {
            Motion::Full => "everything moves",
            Motion::Reduced => "no drifting air, no long fades",
            Motion::Off => "nothing animates; still playable",
        }
    }

    /// The next setting, for a menu that cycles.
    #[must_use]
    pub const fn next(self) -> Motion {
        match self {
            Motion::Full => Motion::Reduced,
            Motion::Reduced => Motion::Off,
            Motion::Off => Motion::Full,
        }
    }

    /// Whether continuous animation runs at all.
    #[must_use]
    pub const fn animates(self) -> bool {
        matches!(self, Motion::Full)
    }

    /// Whether the world's ambient detail (drifting motes, candles, water)
    /// advances.
    #[must_use]
    pub const fn ambience(self) -> bool {
        matches!(self, Motion::Full | Motion::Reduced)
    }

    /// Whether a slow ambient frame is worth asking for.
    ///
    /// With motion off the world is still, so the loop should settle completely
    /// rather than wake up to redraw the same frame.
    #[must_use]
    pub const fn ambient_frames(self) -> bool {
        self.ambience()
    }

    /// Scales a duration: unchanged, halved, or skipped entirely.
    #[must_use]
    pub fn duration(self, base: Duration) -> Duration {
        match self {
            Motion::Full => base,
            Motion::Reduced => base / 2,
            Motion::Off => Duration::ZERO,
        }
    }

    /// Whether an effect of this length should play at all.
    #[must_use]
    pub fn shows(self, base: Duration) -> bool {
        !self.duration(base).is_zero()
    }
}

/// The player's preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    /// How much motion they want.
    pub motion: Motion,
}

impl Settings {
    /// The defaults.
    #[must_use]
    pub const fn new() -> Self {
        Settings {
            motion: Motion::Full,
        }
    }

    /// Renders the settings file.
    #[must_use]
    pub fn encode(self) -> String {
        format!("{MAGIC} {VERSION}\nmotion={}\n", self.motion.id())
    }

    /// Parses a settings file, falling back to the defaults for anything else.
    #[must_use]
    pub fn decode(text: &str) -> Settings {
        let mut settings = Settings::new();
        let mut lines = text.lines();
        let Some(header) = lines.next() else {
            return settings;
        };
        let mut parts = header.split_whitespace();
        if parts.next() != Some(MAGIC) || parts.next().and_then(|v| v.parse().ok()) != Some(VERSION)
        {
            return settings;
        }
        for line in lines {
            if let Some(("motion", value)) = line.split_once('=') {
                if let Some(motion) = Motion::from_id(value.trim()) {
                    settings.motion = motion;
                }
            }
        }
        settings
    }

    /// Where the settings live for the current user, if anywhere.
    #[must_use]
    pub fn location() -> Option<PathBuf> {
        base_directory().map(|base| base.join("grimmoire").join("settings.txt"))
    }

    /// Reads the player's settings, or the defaults.
    #[must_use]
    pub fn load() -> Settings {
        Settings::location()
            .and_then(|path| fs::read_to_string(path).ok())
            .map_or_else(Settings::new, |text| Settings::decode(&text))
    }

    /// Writes the settings, replacing any previous file.
    ///
    /// Like the save, this writes a sibling temporary file and renames it over the
    /// target, so an interrupted write cannot leave half a preference behind.
    pub fn save(&self) -> io::Result<()> {
        let Some(path) = Settings::location() else {
            return Ok(());
        };
        save_to(&path, *self)
    }
}

/// Writes `settings` to `path`, creating its directory.
pub fn save_to(path: &Path, settings: Settings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, settings.encode())?;
    fs::rename(&temporary, path)
}

/// The conventional per-user data directory, if one can be found.
fn base_directory() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME").map(|home| Path::new(&home).join(".local").join("share"))
        })
}

/// The settings, shared between the application and the screens that change them.
///
/// The application owns the frame loop and reads the motion setting for every
/// frame; the title screen is the thing that lets the player change it. A shared
/// cell is the smallest honest bridge between the two: one value, one writer, and
/// every change written straight to disk so a preference cannot be lost by
/// quitting in the wrong order.
#[derive(Debug, Clone)]
pub struct Shared(std::rc::Rc<std::cell::Cell<Settings>>);

impl Shared {
    /// Wraps settings for sharing.
    #[must_use]
    pub fn new(settings: Settings) -> Self {
        Shared(std::rc::Rc::new(std::cell::Cell::new(settings)))
    }

    /// The settings as they stand.
    #[must_use]
    pub fn get(&self) -> Settings {
        self.0.get()
    }

    /// Replaces the settings and writes them out. A failure to write is not worth
    /// interrupting play for: the preference holds for this session either way.
    pub fn set(&self, settings: Settings) {
        self.0.set(settings);
        let _ = settings.save();
    }

    /// Advances the motion setting and returns the new one.
    pub fn cycle_motion(&self) -> Settings {
        let next = Settings {
            motion: self.get().motion.next(),
        };
        self.set(next);
        next
    }

    /// How many times the underlying value has been read, for tests.
    #[cfg(test)]
    #[must_use]
    pub fn count(&self) -> usize {
        self.0.get().motion as usize
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Motion, Settings, Shared, save_to};

    #[test]
    fn the_defaults_are_the_full_experience() {
        assert_eq!(Motion::Full, Settings::new().motion);
        assert_eq!(Motion::Full, Settings::default().motion);
    }

    #[test]
    fn settings_round_trip() {
        for motion in Motion::ALL {
            let settings = Settings { motion };
            assert_eq!(settings, Settings::decode(&settings.encode()));
        }
    }

    #[test]
    fn anything_unrecognised_falls_back_to_the_defaults() {
        assert_eq!(Settings::new(), Settings::decode(""));
        assert_eq!(Settings::new(), Settings::decode("not a settings file"));
        assert_eq!(
            Settings::new(),
            Settings::decode("grimmoire-settings 9\nmotion=off\n")
        );
        assert_eq!(
            Settings::new(),
            Settings::decode("grimmoire-settings 1\nmotion=sideways\n")
        );
        assert_eq!(
            Motion::Off,
            Settings::decode("grimmoire-settings 1\nmotion=off\nrubbish\n").motion
        );
    }

    #[test]
    fn motion_cycles_through_every_setting() {
        let mut motion = Motion::Full;
        let mut seen = vec![motion];
        for _ in 0..Motion::ALL.len() - 1 {
            motion = motion.next();
            seen.push(motion);
        }
        assert_eq!(Motion::ALL.to_vec(), seen);
        assert_eq!(Motion::Full, motion.next(), "it comes back round");
    }

    #[test]
    fn motion_scales_and_skips_durations() {
        let base = Duration::from_millis(300);
        assert_eq!(base, Motion::Full.duration(base));
        assert_eq!(base / 2, Motion::Reduced.duration(base));
        assert_eq!(Duration::ZERO, Motion::Off.duration(base));
        assert!(Motion::Full.shows(base));
        assert!(Motion::Reduced.shows(base));
        assert!(!Motion::Off.shows(base));
    }

    #[test]
    fn motion_off_stops_asking_for_frames() {
        assert!(Motion::Full.animates());
        assert!(!Motion::Reduced.animates());
        assert!(!Motion::Off.animates());
        assert!(!Motion::Off.ambient_frames());
        assert!(Motion::Reduced.ambient_frames());
    }

    #[test]
    fn ids_round_trip() {
        for motion in Motion::ALL {
            assert_eq!(Some(motion), Motion::from_id(motion.id()));
        }
        assert_eq!(None, Motion::from_id("brisk"));
    }

    #[test]
    fn shared_settings_are_visible_to_everyone_holding_them() {
        let shared = Shared::new(Settings::new());
        assert_eq!(Motion::Full, shared.get().motion);
        let also = shared.clone();
        shared.cycle_motion();
        assert_eq!(Motion::Reduced, also.get().motion, "it is the same value");
    }

    #[test]
    fn saving_and_loading_goes_through_the_filesystem() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("grimmoire-settings-{unique}"));
        let path = directory.join("settings.txt");
        let settings = Settings {
            motion: Motion::Off,
        };
        save_to(&path, settings).expect("save");
        assert_eq!(
            settings,
            Settings::decode(&std::fs::read_to_string(&path).unwrap())
        );
        assert!(
            !path.with_extension("tmp").exists(),
            "the temporary file should have been renamed away"
        );
        std::fs::remove_dir_all(&directory).expect("cleanup");
    }
}
