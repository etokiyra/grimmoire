//! Terminal-independent input events.
//!
//! The terminal backend translates raw library events into these types so that
//! screens never depend on a specific terminal library, and so that tests can
//! synthesise input directly.

/// A key on the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key.
    Char(char),
    /// The Enter / Return key.
    Enter,
    /// The Escape key.
    Esc,
    /// The Backspace key.
    Backspace,
    /// The Tab key.
    Tab,
    /// The Delete key.
    Delete,
    /// The Insert key.
    Insert,
    /// The Home key.
    Home,
    /// The End key.
    End,
    /// The Page Up key.
    PageUp,
    /// The Page Down key.
    PageDown,
    /// The Up arrow key.
    Up,
    /// The Down arrow key.
    Down,
    /// The Left arrow key.
    Left,
    /// The Right arrow key.
    Right,
    /// A numbered function key.
    Function(u8),
}

/// The modifier keys held while a key was pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct KeyModifiers {
    /// Control was held.
    pub ctrl: bool,
    /// Alt / Meta was held.
    pub alt: bool,
    /// Shift was held.
    pub shift: bool,
}

impl KeyModifiers {
    /// No modifiers held.
    pub const NONE: KeyModifiers = KeyModifiers {
        ctrl: false,
        alt: false,
        shift: false,
    };
}

/// What kind of key event this is.
///
/// Terminals only report presses unless they implement the Kitty keyboard
/// protocol's event-type reporting, which the backend requests when it can. The
/// distinction is what makes "hold to walk, release to stop" possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyKind {
    /// The key was pressed.
    #[default]
    Press,
    /// The key is being held and the terminal reported an auto-repeat.
    Repeat,
    /// The key was released.
    Release,
}

/// A single key event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key {
    /// Which key was pressed.
    pub code: KeyCode,
    /// Which modifiers were held.
    pub modifiers: KeyModifiers,
    /// Whether this is a press, an auto-repeat, or a release.
    pub kind: KeyKind,
}

impl Key {
    /// Creates an unmodified key press.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Key {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyKind::Press,
        }
    }

    /// Creates an unmodified character press.
    #[must_use]
    pub const fn char(ch: char) -> Self {
        Key::new(KeyCode::Char(ch))
    }

    /// Returns a copy with the kind replaced.
    #[must_use]
    pub const fn with_kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    /// Returns a copy marked as an auto-repeat of a held key.
    #[must_use]
    pub const fn with_repeat(self) -> Self {
        self.with_kind(KeyKind::Repeat)
    }

    /// Returns a copy marked as a key release.
    #[must_use]
    pub const fn released(self) -> Self {
        self.with_kind(KeyKind::Release)
    }

    /// Returns a copy with the Control modifier set.
    #[must_use]
    pub const fn with_ctrl(mut self) -> Self {
        self.modifiers.ctrl = true;
        self
    }

    /// Returns a copy with the Alt modifier set.
    #[must_use]
    pub const fn with_alt(mut self) -> Self {
        self.modifiers.alt = true;
        self
    }

    /// Returns a copy with the Shift modifier set.
    #[must_use]
    pub const fn with_shift(mut self) -> Self {
        self.modifiers.shift = true;
        self
    }

    /// Whether this is a press or an auto-repeat - that is, the key is down.
    #[must_use]
    pub const fn is_down(&self) -> bool {
        !matches!(self.kind, KeyKind::Release)
    }

    /// Whether this is an auto-repeat.
    #[must_use]
    pub const fn is_repeat(&self) -> bool {
        matches!(self.kind, KeyKind::Repeat)
    }

    /// Whether the key was released.
    #[must_use]
    pub const fn is_release(&self) -> bool {
        matches!(self.kind, KeyKind::Release)
    }

    /// Whether this activates `ch` without modifiers.
    ///
    /// Releases are not activations, so this is false for them.
    #[must_use]
    pub fn is_char(&self, ch: char) -> bool {
        self.is_down() && self.code == KeyCode::Char(ch) && self.modifiers == KeyModifiers::NONE
    }

    /// Whether this activates `Ctrl`+`ch`. False for releases.
    #[must_use]
    pub fn is_ctrl(&self, ch: char) -> bool {
        self.is_down() && self.modifiers.ctrl && self.code == KeyCode::Char(ch)
    }
}

/// An input event delivered to the application loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// A key was pressed.
    Key(Key),
    /// The terminal was resized to the given dimensions in cells.
    Resize {
        /// New width in columns.
        width: u16,
        /// New height in rows.
        height: u16,
    },
}

impl Event {
    /// Returns the key press, if this event is one.
    #[must_use]
    pub const fn key(self) -> Option<Key> {
        match self {
            Event::Key(key) => Some(key),
            Event::Resize { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Key, KeyCode, KeyModifiers};

    #[test]
    fn key_helpers_distinguish_modifiers() {
        assert!(Key::char('q').is_char('q'));
        assert!(!Key::char('q').with_ctrl().is_char('q'));
        assert!(Key::char('c').with_ctrl().is_ctrl('c'));
        assert!(!Key::new(KeyCode::Esc).is_ctrl('c'));
    }

    #[test]
    fn event_key_accessor() {
        assert_eq!(Some(Key::char('a')), Event::Key(Key::char('a')).key());
        assert_eq!(
            None,
            Event::Resize {
                width: 1,
                height: 2
            }
            .key()
        );
    }

    #[test]
    fn modifiers_are_additive() {
        let key = Key::char('a').with_ctrl().with_shift();
        assert_eq!(
            KeyModifiers {
                ctrl: true,
                alt: false,
                shift: true
            },
            key.modifiers
        );
    }

    #[test]
    fn auto_repeat_and_release_are_distinguished_from_a_press() {
        assert!(!Key::char('a').is_repeat());
        assert!(Key::char('a').is_down());

        let repeated = Key::char('a').with_repeat();
        assert!(repeated.is_repeat());
        assert!(repeated.is_down());
        assert!(
            repeated.is_char('a'),
            "repeat must not change the key identity"
        );

        let released = Key::char('a').released();
        assert!(released.is_release());
        assert!(!released.is_down());
        assert!(!released.is_char('a'), "a release is not an activation");
        assert!(!Key::char('q').released().is_char('q'));
        assert!(!Key::char('c').with_ctrl().released().is_ctrl('c'));
    }
}
