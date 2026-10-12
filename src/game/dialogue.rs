//! Dialogue content.
//!
//! A [`Dialogue`] is a speaker and a few lines, and nothing more: there is no
//! scripting and no branching tree. What the game does need — and what a world
//! with discoveries needs — is for a character to *remember*: to say one thing
//! before the player has learned something and another thing afterwards.
//!
//! That is [`Speech`]: a default exchange plus a list of variants, each unlocked
//! by a discovery. It is deliberately not a dialogue engine. It covers "you know
//! about the well now, so I will talk about it" without introducing a script
//! format, and the renderer never sees any of it — it receives a resolved
//! [`Dialogue`] and the figure that spoke it.

use super::discovery::Discovery;
use super::progress::Progress;

/// A short exchange shown in the dialogue panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dialogue {
    /// Who is speaking.
    pub speaker: &'static str,
    /// The lines shown, in order.
    pub lines: &'static [&'static str],
}

impl Dialogue {
    /// Creates a dialogue.
    #[must_use]
    pub const fn new(speaker: &'static str, lines: &'static [&'static str]) -> Self {
        Dialogue { speaker, lines }
    }
}

/// What a character says, depending on what the player has learned.
///
/// Variants are checked in order and the **first** whose discovery is known wins,
/// so the list reads as "most specific first" and resolution is deterministic
/// regardless of the order discoveries were found in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Speech {
    default: Dialogue,
    variants: &'static [(Discovery, Dialogue)],
}

impl Speech {
    /// Speech that never changes.
    #[must_use]
    pub const fn always(dialogue: Dialogue) -> Self {
        Speech {
            default: dialogue,
            variants: &[],
        }
    }

    /// Speech that changes once discoveries are made.
    #[must_use]
    pub const fn reacting(default: Dialogue, variants: &'static [(Discovery, Dialogue)]) -> Self {
        Speech { default, variants }
    }

    /// The exchange this character would have right now.
    #[must_use]
    pub fn resolve(&self, progress: &Progress) -> Dialogue {
        self.variants
            .iter()
            .find(|(discovery, _)| progress.knows(*discovery))
            .map_or(self.default, |(_, dialogue)| *dialogue)
    }

    /// The exchange shown before anything has been discovered.
    ///
    /// Used by tests and by the journal's "who" listing; resolution always goes
    /// through [`Speech::resolve`].
    #[must_use]
    pub const fn default_dialogue(&self) -> Dialogue {
        self.default
    }
}

#[cfg(test)]
mod tests {
    use super::{Dialogue, Speech};
    use crate::game::discovery::Discovery;
    use crate::game::progress::Progress;

    const BEFORE: Dialogue = Dialogue::new("a voice", &["Keep to the path."]);
    const AFTER: Dialogue = Dialogue::new("a voice", &["You found the glade."]);
    const LATER: Dialogue = Dialogue::new("a voice", &["You answered the moon."]);

    static VARIANTS: [(Discovery, Dialogue); 2] = [
        (Discovery::WoodsGlade, AFTER),
        (Discovery::VillageWell, LATER),
    ];

    #[test]
    fn dialogue_exposes_its_content() {
        let dialogue = Dialogue::new("a voice", &["First.", "Second."]);
        assert_eq!("a voice", dialogue.speaker);
        assert_eq!(2, dialogue.lines.len());
    }

    #[test]
    fn speech_without_variants_never_changes() {
        let speech = Speech::always(BEFORE);
        assert_eq!(BEFORE, speech.resolve(&Progress::new()));
        assert_eq!(BEFORE, speech.default_dialogue());
    }

    #[test]
    fn speech_changes_once_a_discovery_is_known() {
        let speech = Speech::reacting(BEFORE, &VARIANTS);
        assert_eq!(BEFORE, speech.resolve(&Progress::new()));

        let mut progress = Progress::new();
        progress.record(Discovery::VillageWell);
        assert_eq!(LATER, speech.resolve(&progress));

        progress.record(Discovery::WoodsGlade);
        assert_eq!(
            AFTER,
            speech.resolve(&progress),
            "the first matching variant wins, not the newest discovery"
        );
    }

    #[test]
    fn an_unrelated_discovery_does_not_change_speech() {
        let speech = Speech::reacting(BEFORE, &VARIANTS);
        let mut progress = Progress::new();
        progress.record(Discovery::ChapelBasin);
        assert_eq!(BEFORE, speech.resolve(&progress));
    }
}
