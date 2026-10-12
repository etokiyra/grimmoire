//! What the player has learned, and what that has changed.
//!
//! This is the whole of Grimmoire's progression state: the discoveries recorded,
//! the journal they are recorded in, and the shrine puzzle's progress. It is
//! deliberately small — the point of this phase is a *loop* (explore, discover,
//! learn, unlock, explore further), not a level system — but it is the state that
//! a later inventory, spellbook or quest log would be built beside rather than
//! replacing.
//!
//! Nothing here knows how anything looks. The journal text lives with each
//! [`Discovery`]; this module only tracks *which* of them are known and in what
//! order.

use std::collections::BTreeSet;

use super::discovery::Discovery;

/// One of the three moon phases cut into the shrine's dial and braziers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MoonPhase {
    /// The dark of the moon, drawn as a closing crescent.
    Waning,
    /// The full moon.
    Full,
    /// The waxing crescent.
    Waxing,
}

impl MoonPhase {
    /// Every phase, in the order the dial lists them.
    pub const ALL: [MoonPhase; 3] = [MoonPhase::Waning, MoonPhase::Full, MoonPhase::Waxing];

    /// The stable slug written to save data.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            MoonPhase::Waning => "waning",
            MoonPhase::Full => "full",
            MoonPhase::Waxing => "waxing",
        }
    }

    /// The phase with this slug, if there is one.
    #[must_use]
    pub fn from_id(id: &str) -> Option<MoonPhase> {
        MoonPhase::ALL.into_iter().find(|phase| phase.id() == id)
    }

    /// This phase's index in [`MoonPhase::ALL`].
    ///
    /// Used to give each brazier its own offset in the flame animation, so three
    /// lit braziers are three fires rather than one pattern repeated.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            MoonPhase::Waning => 0,
            MoonPhase::Full => 1,
            MoonPhase::Waxing => 2,
        }
    }

    /// How the phase is named in a message.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            MoonPhase::Waning => "waning",
            MoonPhase::Full => "full",
            MoonPhase::Waxing => "waxing",
        }
    }
}

/// The order the braziers must be lit in.
///
/// This is the shrine's rule, stated in one place. The player learns it from the
/// dial cut into the floor beside the braziers, which lists the same three phases
/// in the same order: the puzzle is reading the room, not guessing.
pub const MOON_ORDER: [MoonPhase; 3] = [MoonPhase::Waning, MoonPhase::Full, MoonPhase::Waxing];

/// What happened when a brazier was lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lighted {
    /// The brazier joined the sequence; more are still dark.
    Lit,
    /// The brazier was out of order and the whole sequence guttered out.
    Guttered,
    /// That was the last one: the moon has been answered.
    Answered,
    /// Nothing changed — the brazier was already burning.
    Already,
}

/// The progression state: what is known, and what it has unlocked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Progress {
    known: BTreeSet<Discovery>,
    journal: Vec<Discovery>,
    braziers: Vec<MoonPhase>,
}

impl Progress {
    /// An empty progression: nothing discovered, nothing lit.
    #[must_use]
    pub const fn new() -> Self {
        Progress {
            known: BTreeSet::new(),
            journal: Vec::new(),
            braziers: Vec::new(),
        }
    }

    /// Whether the player has learned this.
    #[must_use]
    pub fn knows(&self, discovery: Discovery) -> bool {
        self.known.contains(&discovery)
    }

    /// Records a discovery, in journal order.
    ///
    /// Returns whether this was genuinely new, so the interface can celebrate the
    /// first time and stay quiet about a re-examination.
    pub fn record(&mut self, discovery: Discovery) -> bool {
        if !self.known.insert(discovery) {
            return false;
        }
        self.journal.push(discovery);
        true
    }

    /// The discoveries found so far, in the order they were found.
    #[must_use]
    pub fn journal(&self) -> &[Discovery] {
        &self.journal
    }

    /// How many discoveries have been recorded.
    #[must_use]
    pub fn discovered(&self) -> usize {
        self.journal.len()
    }

    /// How many discoveries exist to be found.
    #[must_use]
    pub fn total() -> usize {
        Discovery::ALL.len()
    }

    /// The braziers lit so far, in the order they were lit.
    #[must_use]
    pub fn braziers(&self) -> &[MoonPhase] {
        &self.braziers
    }

    /// Whether a brazier of this phase is currently burning.
    #[must_use]
    pub fn is_lit(&self, phase: MoonPhase) -> bool {
        self.braziers.contains(&phase)
    }

    /// Whether the moonwell has been answered.
    #[must_use]
    pub fn answered(&self) -> bool {
        self.braziers == MOON_ORDER
    }

    /// Lights a brazier, advancing or resetting the sequence.
    ///
    /// The rule is checked against [`MOON_ORDER`], and a brazier lit out of turn
    /// puts *all* of them out rather than only refusing the new one: a partial
    /// success that stays lit would let a player brute-force the order, and the
    /// guttering is the feedback that says the order mattered.
    pub fn light(&mut self, phase: MoonPhase) -> Lighted {
        if self.answered() {
            return Lighted::Already;
        }
        if self.braziers.contains(&phase) {
            return Lighted::Already;
        }
        if MOON_ORDER.get(self.braziers.len()) == Some(&phase) {
            self.braziers.push(phase);
            if self.answered() {
                return Lighted::Answered;
            }
            Lighted::Lit
        } else {
            self.braziers.clear();
            Lighted::Guttered
        }
    }

    /// Rebuilds the state from validated ids.
    ///
    /// Everything passed here must already have been checked against the known
    /// discoveries and phases; this is the constructor save loading uses, and it
    /// exists so that the fields stay private and cannot drift out of step.
    #[must_use]
    pub fn restored(journal: Vec<Discovery>, braziers: Vec<MoonPhase>) -> Progress {
        let mut progress = Progress::new();
        for discovery in journal {
            progress.record(discovery);
        }
        progress.braziers = braziers;
        progress
    }

    /// The journal as save ids.
    #[must_use]
    pub fn journal_ids(&self) -> Vec<String> {
        self.journal
            .iter()
            .map(|item| item.id().to_string())
            .collect()
    }

    /// The lit braziers as save ids.
    #[must_use]
    pub fn brazier_ids(&self) -> Vec<String> {
        self.braziers
            .iter()
            .map(|phase| phase.id().to_string())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Lighted, MOON_ORDER, MoonPhase, Progress};
    use crate::game::discovery::Discovery;

    #[test]
    fn recording_a_discovery_is_idempotent_and_ordered() {
        let mut progress = Progress::new();
        assert!(!progress.knows(Discovery::VillageWell));
        assert!(progress.record(Discovery::VillageWell));
        assert!(!progress.record(Discovery::VillageWell), "already known");
        assert!(progress.record(Discovery::WoodsGlade));
        assert_eq!(
            &[Discovery::VillageWell, Discovery::WoodsGlade],
            progress.journal()
        );
        assert_eq!(2, progress.discovered());
    }

    #[test]
    fn the_braziers_must_be_lit_in_the_order_the_dial_shows() {
        let mut progress = Progress::new();
        assert_eq!(Lighted::Lit, progress.light(MoonPhase::Waning));
        assert_eq!(Lighted::Lit, progress.light(MoonPhase::Full));
        assert_eq!(Lighted::Answered, progress.light(MoonPhase::Waxing));
        assert!(progress.answered());
        assert_eq!(&MOON_ORDER, progress.braziers());
    }

    #[test]
    fn a_brazier_out_of_order_puts_all_of_them_out() {
        let mut progress = Progress::new();
        progress.light(MoonPhase::Waning);
        assert_eq!(Lighted::Guttered, progress.light(MoonPhase::Waxing));
        assert!(progress.braziers().is_empty(), "the whole sequence reset");
        assert!(!progress.answered());

        // And the correct order still works afterwards: no soft lock.
        assert_eq!(Lighted::Lit, progress.light(MoonPhase::Waning));
        assert_eq!(Lighted::Lit, progress.light(MoonPhase::Full));
        assert_eq!(Lighted::Answered, progress.light(MoonPhase::Waxing));
    }

    #[test]
    fn relighting_a_lit_brazier_changes_nothing() {
        let mut progress = Progress::new();
        progress.light(MoonPhase::Waning);
        assert_eq!(Lighted::Already, progress.light(MoonPhase::Waning));
        assert_eq!(1, progress.braziers().len());
    }

    #[test]
    fn an_answered_moonwell_stays_answered() {
        let mut progress = Progress::new();
        for phase in MOON_ORDER {
            progress.light(phase);
        }
        for phase in MoonPhase::ALL {
            assert_eq!(Lighted::Already, progress.light(phase));
        }
        assert_eq!(&MOON_ORDER, progress.braziers());
    }

    #[test]
    fn phases_and_discoveries_round_trip_through_their_ids() {
        for phase in MoonPhase::ALL {
            assert_eq!(Some(phase), MoonPhase::from_id(phase.id()));
        }
        assert_eq!(None, MoonPhase::from_id("gibbous"));

        let mut progress = Progress::new();
        progress.record(Discovery::ChapelBasin);
        progress.record(Discovery::MoonwellDial);
        progress.light(MoonPhase::Waning);
        let rebuilt = Progress::restored(
            progress
                .journal_ids()
                .iter()
                .filter_map(|id| Discovery::from_id(id))
                .collect(),
            progress
                .brazier_ids()
                .iter()
                .filter_map(|id| MoonPhase::from_id(id))
                .collect(),
        );
        assert_eq!(progress, rebuilt);
    }
}
