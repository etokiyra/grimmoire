//! Discoveries: the things worth learning about the world.
//!
//! A discovery is a **stable, saveable id** with its content attached: a title
//! and a journal entry. The set the player has found lives in
//! [`crate::game::progress::Progress`], and the same ids are used to unlock
//! dialogue variants and to record why a doorway has opened.
//!
//! Keeping the ids in the model and the journal text beside them means the whole
//! progression is auditable in one place: every discovery the game can record is
//! listed in [`Discovery::ALL`], and a save referring to anything else is
//! rejected rather than half-loaded.
//!
//! # Why an enum rather than a string id
//!
//! Areas use a `&'static str` id because they are looked up by name in save data
//! and could be reordered. Discoveries are matched *in code* — a dialogue variant
//! asks "does the player know this?" — so an enum makes every one of those points
//! a compile error when a discovery is added or renamed, and makes an unknown id
//! in a save impossible to confuse with a real one.

/// Something the player can learn about the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Discovery {
    /// The basin in the chapel, and what it is still full of.
    ChapelBasin,
    /// The quiet glade hidden deep in the woods.
    WoodsGlade,
    /// The village well, and what was thrown into it.
    VillageWell,
    /// A bundle of letters that was never sent.
    VillageLetters,
    /// The moon dial at the shrine, and how it is read.
    MoonwellDial,
    /// The moonwell answered: the braziers lit in the order the dial shows.
    MoonwellAnswered,
    /// The bone scribe's tomb in the catacombs.
    CatacombScribe,
    /// The sigil worn into the chapel floor, and what it is a tally of.
    ChapelSigil,
    /// The wall of names in the catacombs: everyone the water was given.
    CatacombNames,
    /// The door under the shrine, sealed from this side with lead.
    ShrineSeal,
    /// A twelfth lantern in the wood, unlit, with a name inside the shutter.
    TwelfthLantern,
}

impl Discovery {
    /// Every discovery the game can record, in a stable order.
    ///
    /// A save naming anything outside this list is treated as unknown and
    /// dropped, so an old or hand-edited file can never invent progress.
    pub const ALL: [Discovery; 11] = [
        Discovery::ChapelBasin,
        Discovery::WoodsGlade,
        Discovery::VillageWell,
        Discovery::VillageLetters,
        Discovery::MoonwellDial,
        Discovery::MoonwellAnswered,
        Discovery::CatacombScribe,
        Discovery::ChapelSigil,
        Discovery::CatacombNames,
        Discovery::ShrineSeal,
        Discovery::TwelfthLantern,
    ];

    /// The stable slug written to save data.
    ///
    /// Deliberately short and never shown to the player: renaming it would
    /// orphan the discovery in an existing save.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Discovery::ChapelBasin => "chapel-basin",
            Discovery::WoodsGlade => "woods-glade",
            Discovery::VillageWell => "village-well",
            Discovery::VillageLetters => "village-letters",
            Discovery::MoonwellDial => "moonwell-dial",
            Discovery::MoonwellAnswered => "moonwell-answered",
            Discovery::CatacombScribe => "catacomb-scribe",
            Discovery::ChapelSigil => "chapel-sigil",
            Discovery::CatacombNames => "catacomb-names",
            Discovery::ShrineSeal => "shrine-seal",
            Discovery::TwelfthLantern => "twelfth-lantern",
        }
    }

    /// The discovery with this slug, if there is one.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Discovery> {
        Discovery::ALL.into_iter().find(|item| item.id() == id)
    }

    /// The heading this discovery is recorded under.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Discovery::ChapelBasin => "The Cracked Basin",
            Discovery::WoodsGlade => "The Quiet Glade",
            Discovery::VillageWell => "The Village Well",
            Discovery::VillageLetters => "Letters Never Sent",
            Discovery::MoonwellDial => "The Moon Dial",
            Discovery::MoonwellAnswered => "The Moon Answered",
            Discovery::CatacombScribe => "The Bone Scribe",
            Discovery::ChapelSigil => "The Worn Sigil",
            Discovery::CatacombNames => "The Wall of Names",
            Discovery::ShrineSeal => "The Sealed Door",
            Discovery::TwelfthLantern => "The Twelfth Lantern",
        }
    }

    /// What the journal says once this has been found.
    ///
    /// This is the reward for exploring: the setting, told a piece at a time.
    #[must_use]
    pub const fn journal(self) -> &'static str {
        match self {
            Discovery::ChapelBasin => {
                "Still water, cold and dark, in a basin no one has filled in years. \
                 Either it is fed from below, or something keeps filling it."
            }
            Discovery::WoodsGlade => {
                "A ring of birches around a soft floor, and no path leading in. \
                 Someone cleared this place, and someone still keeps it clear."
            }
            Discovery::VillageWell => {
                "The well is dry, and the bucket is missing — but the rope is cut \
                 at the top, not the bottom. Whatever they dropped, they did not \
                 want it back."
            }
            Discovery::VillageLetters => {
                "A dead woman's letters, all addressed to the chapel and none of \
                 them opened. The last one says only: 'I have told the moon.'"
            }
            Discovery::MoonwellDial => {
                "A dial cut into the shrine floor: a waning crescent, a full moon, \
                 and a waxing one, in that order. The moon rises dark, fills, and \
                 lets go."
            }
            Discovery::MoonwellAnswered => {
                "Three braziers, lit in the order the dial shows. The water took \
                 the light and gave back a door that had been there all along."
            }
            Discovery::CatacombScribe => {
                "A scribe was buried here with her hands full of wax and her \
                 stylus still sharp. The wall beside her is covered in names — \
                 all of them the same name."
            }
            Discovery::ChapelSigil => {
                "The same sigil is cut into the chapel floor and into the cellar \
                 stone, worn smooth in both. Not a prayer: a tally kept in one \
                 place and copied in another, and the hand that kept it stopped."
            }
            Discovery::CatacombNames => {
                "A wall of names cut small and close, hundreds of them, in one \
                 careful hand and then in a worse one. The last is still sharp. \
                 Nobody scratches a name into a wall for someone who died in bed."
            }
            Discovery::ShrineSeal => {
                "A door with no handle, and lead poured into the seam from this \
                 side. It was not sealed to keep something in. It was sealed by \
                 people standing where I am standing, who did not want to go on."
            }
            Discovery::TwelfthLantern => {
                "A twelfth lantern, unlit, hung higher than the others on a branch \
                 that has grown around it. Inside the shutter, scratched where only \
                 the person lighting it would look: a name, and a small cross."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Discovery;

    #[test]
    fn every_discovery_has_a_unique_stable_id() {
        let mut ids: Vec<&str> = Discovery::ALL.iter().map(|item| item.id()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(count, ids.len(), "two discoveries share a save id");
    }

    #[test]
    fn ids_round_trip() {
        for discovery in Discovery::ALL {
            assert_eq!(Some(discovery), Discovery::from_id(discovery.id()));
        }
        assert_eq!(None, Discovery::from_id("no-such-discovery"));
        assert_eq!(None, Discovery::from_id(""));
    }

    #[test]
    fn every_discovery_has_a_title_and_a_journal_entry() {
        for discovery in Discovery::ALL {
            assert!(!discovery.title().is_empty(), "{discovery:?} has no title");
            assert!(
                discovery.journal().len() > 40,
                "{discovery:?} has no real journal entry"
            );
        }
    }
}
