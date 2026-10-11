//! Dialogue content.
//!
//! Deliberately minimal: a speaker and a few lines. There is no scripting,
//! branching, or quest state yet, and none should be added here until a later
//! phase needs it.

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

#[cfg(test)]
mod tests {
    use super::Dialogue;

    #[test]
    fn dialogue_exposes_its_content() {
        let dialogue = Dialogue::new("a voice", &["First.", "Second."]);
        assert_eq!("a voice", dialogue.speaker);
        assert_eq!(2, dialogue.lines.len());
    }
}
