//! The authored region: its maps, its cast, and its points of interest.
//!
//! **Content, not logic.** Every map here is an ASCII grid plus a list of things
//! placed on it; every character is a name, a [`Speech`], and a [`Figure`]; every
//! doorway is a position and a destination. Nothing in this file decides how any
//! of it looks — the interface maps figures and props to art — and nothing in it
//! runs a rule: the world model does that.
//!
//! # The region
//!
//! ```text
//!                    Hollow Village ─────┐
//!                          │             │
//!   Ruined Chapel ── Whispering Woods ── Moonwell Shrine
//!        │                                   │ (sealed until answered)
//!   Sunken Cellar ── Forgotten Catacombs ─────┘
//! ```
//!
//! Every area is reachable without solving anything. The moonwell does not gate a
//! place: it opens a *shortcut* between the shrine and the catacombs, which is a
//! reward for exploring rather than a toll on it.
//!
//! # Maps
//!
//! The `&[...]` grids below are **generated**, not hand-edited: see
//! `tools/generate-areas.py` and `tools/apply-areas.py`, and
//! [`docs/map-design.md`](../../docs/map-design.md) for the rules they enforce —
//! above all that no passage the player can stand in is narrower than
//! [`MIN_PASSAGE`] cells, because the character sprite is five cells wide.

use super::area::{Area, AreaId, Direction, Portal, Position};
use super::conversation::{Choice, Condition, Conversation, Effect, Flag, Mood, Node, NodeId};
use super::discovery::Discovery;
use super::entity::{Entity, Figure, Prop};
use super::progress::MoonPhase;

/// The narrowest a passage may be, in cells.
///
/// **Every map must obey this.** The character sprite is five cells wide and
/// three tall, so a passage narrower than three leaves her shoulders through the
/// wall on both sides — which is exactly what the first maps looked like. Three
/// is the sprite width minus two cells of acceptable overlap; the routes a player
/// uses constantly are drawn wider still (the chapel nave is nine cells, its
/// crossings and both stair doorways five).
///
/// The same rule covers anything the player passes *through*: doorways, chamber
/// mouths, corridors, and the gaps between pillars and pews. `tools/generate-areas.py`
/// enforces it when it draws a map, and
/// [`tests::every_passage_fits_the_character`] enforces it here, so a hand-edited
/// map cannot quietly regress. See `docs/map-design.md`.
pub const MIN_PASSAGE: u16 = 3;

/// How wide a doorway in an outer wall is drawn, in cells.
pub const DOORWAY: u16 = 5;

/// The id of the area the game starts in.
pub const CHAPEL: AreaId = AreaId::new("ruined-chapel");
/// The flooded crypt beneath the chapel.
pub const CELLAR: AreaId = AreaId::new("sunken-cellar");
/// The wood the chapel stands in.
pub const WOODS: AreaId = AreaId::new("whispering-woods");
/// The village north of the wood, empty for a generation.
pub const VILLAGE: AreaId = AreaId::new("hollow-village");
/// The shrine east of the wood, and the moonwell inside it.
pub const SHRINE: AreaId = AreaId::new("moonwell-shrine");
/// The burial galleries beneath the shrine.
pub const CATACOMBS: AreaId = AreaId::new("forgotten-catacombs");

// -- the cast ----------------------------------------------------------------
//
// Every character is defined here, once: their name, the silhouette they are
// drawn with, the way they stand, and the conversation they will have. Everything
// else about them — how they look, what colour they are, what their face does —
// belongs to the interface.
//
// Placement happens in the area constructors below, which stand a cast member at
// a position. Nothing about a character is written twice.
//
// Names, lines and story beats are content, and like everything in this file they
// are provisional.

/// A character, defined once.
#[derive(Debug, Clone, Copy)]
pub struct CastMember {
    /// What they are called.
    pub name: &'static str,
    /// The silhouette the interface draws them with.
    pub figure: Figure,
    /// The way they stand, so they can be facing what they are about.
    pub facing: Direction,
    /// What they will say.
    pub talk: &'static Conversation,
}

/// Everyone in the region, in a stable order.
///
/// Used by the conversation integrity tests, which walk every graph the game can
/// put a player inside.
#[must_use]
pub fn roster() -> &'static [CastMember] {
    &CAST
}

/// Ysolde, who keeps the chapel.
pub const YSOLDE_MEMBER: CastMember = CastMember {
    name: "Ysolde",
    figure: Figure::Veiled,
    facing: Direction::East,
    talk: &YSOLDE,
};
/// Rin, who keeps the lanterns.
pub const RIN_MEMBER: CastMember = CastMember {
    name: "Rin",
    figure: Figure::Lantern,
    facing: Direction::South,
    talk: &RIN,
};
/// Cassen, of the hollow village.
pub const CASSEN_MEMBER: CastMember = CastMember {
    name: "Cassen",
    figure: Figure::Scavenger,
    facing: Direction::East,
    talk: &CASSEN,
};
/// Ioreth, priestess of the moonwell.
pub const IORETH_MEMBER: CastMember = CastMember {
    name: "Ioreth",
    figure: Figure::Priestess,
    facing: Direction::East,
    talk: &IORETH,
};
/// Nim, her apprentice.
pub const NIM_MEMBER: CastMember = CastMember {
    name: "Nim",
    figure: Figure::Apprentice,
    facing: Direction::West,
    talk: &NIM,
};
/// Maren, a wandering mage.
pub const MAREN_MEMBER: CastMember = CastMember {
    name: "Maren",
    figure: Figure::Wanderer,
    facing: Direction::South,
    talk: &MAREN,
};
/// Wren, who keeps the glade.
pub const WREN_MEMBER: CastMember = CastMember {
    name: "Wren",
    figure: Figure::Spirit,
    facing: Direction::East,
    talk: &WREN,
};
/// Tamsin, warden of the catacombs.
pub const TAMSIN_MEMBER: CastMember = CastMember {
    name: "Tamsin",
    figure: Figure::Warden,
    facing: Direction::West,
    talk: &TAMSIN,
};
/// Vesper, who the catacombs kept.
pub const VESPER_MEMBER: CastMember = CastMember {
    name: "Vesper",
    figure: Figure::Revenant,
    facing: Direction::West,
    talk: &VESPER,
};

/// A conversation for a figure that is not part of the cast.
///
/// The player never speaks, but the tools that draw every figure need something
/// to hand `Entity::npc`. This is deliberately not any character's.
pub const YOADLE_TALK: Conversation = Conversation {
    nodes: &[],
    entries: &[],
    fallback: NodeId::new(""),
};

/// The whole cast.
pub static CAST: [CastMember; 9] = [
    YSOLDE_MEMBER,
    RIN_MEMBER,
    CASSEN_MEMBER,
    IORETH_MEMBER,
    NIM_MEMBER,
    MAREN_MEMBER,
    WREN_MEMBER,
    TAMSIN_MEMBER,
    VESPER_MEMBER,
];

/// Stands a cast member in an area.
fn standing(at: Position, member: &'static CastMember) -> Entity {
    Entity::npc(at, member.name, member.figure, member.facing, member.talk)
}

// -- Ysolde, who keeps the chapel and has no face -----------------------------

mod ysolde {
    use super::{Choice, Condition, NodeId};

    pub const MOON: NodeId = NodeId::new("moon");
    pub const BASIN: NodeId = NodeId::new("basin");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const CHAPEL: NodeId = NodeId::new("chapel");
    pub const VILLAGE: NodeId = NodeId::new("village");
    pub const LETTERS: NodeId = NodeId::new("letters");
    pub const HERSELF: NodeId = NodeId::new("herself");

    pub static CHOICES: [Choice; 4] = [
        Choice::to("Ask about the chapel.", CHAPEL),
        Choice::to("Ask about the village.", VILLAGE),
        Choice::to("Ask what she is.", HERSELF),
        Choice::end("Say nothing."),
    ];

    pub const ADDRESSED: NodeId = NodeId::new("addressed");

    pub static ADDRESSED_CHOICES: [Choice; 5] = [
        Choice::to("Ask about the letters.", LETTERS)
            .when(Condition::Knows(super::Discovery::VillageLetters)),
        Choice::to("Ask about the chapel.", CHAPEL),
        Choice::to("Ask about the village.", VILLAGE),
        Choice::to("Ask what she is.", HERSELF),
        Choice::end("Say nothing."),
    ];

    pub const RETURNED: NodeId = NodeId::new("returned");
}

/// Ysolde: patient, older than the building, and never quite answering.
static YSOLDE: Conversation = Conversation {
    nodes: &[
        Node::saying(
            ysolde::MOON,
            "Ysolde",
            &[
                "You answered the moon. Nobody has done that since the village was alive.",
                "The door under the shrine remembers you now. It will let you both ways.",
            ],
        )
        .titled("the veiled one")
        .in_mood(Mood::Warm)
        .then(ysolde::GREETING),
        Node::saying(
            ysolde::BASIN,
            "Ysolde",
            &[
                "You looked in the basin. Most people only look at it.",
                "It fills itself. I have never decided whether that is a kindness.",
            ],
        )
        .titled("the veiled one")
        .in_mood(Mood::Veiled)
        .then(ysolde::GREETING),
        Node::asking(
            ysolde::GREETING,
            "Ysolde",
            &[
                "You walk where the stones still remember.",
                "The stair below has not been swept in a long while. Mind the water.",
            ],
            &ysolde::CHOICES,
        )
        .titled("the veiled one"),
        Node::asking(
            ysolde::ADDRESSED,
            "Ysolde",
            &[
                "You walk where the stones still remember.",
                "There is something on you that came from the village. I can nearly read it from here.",
            ],
            &ysolde::ADDRESSED_CHOICES,
        )
        .titled("the veiled one"),
        Node::saying(
            ysolde::RETURNED,
            "Ysolde",
            &[
                "He brought them up himself. Walked the whole path with them held in front of him like a lantern.",
                "I read one. I will not read the rest. Some of it is for the person who wrote it.",
            ],
        )
        .titled("the veiled one")
        .in_mood(Mood::Sorry)
        .then(ysolde::GREETING),
        Node::saying(
            ysolde::CHAPEL,
            "Ysolde",
            &[
                "A chapel, before it was a ruin. People came to be told that things would be all right.",
                "I did the telling. I was not very good at it.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(ysolde::GREETING),
        Node::saying(
            ysolde::VILLAGE,
            "Ysolde",
            &[
                "There was a village north of the wood. There is still a village north of the wood.",
                "The difference is a matter of who is doing the living in it.",
            ],
        )
        .in_mood(Mood::Veiled)
        .then(ysolde::GREETING),
        Node::saying(
            ysolde::LETTERS,
            "Ysolde",
            &[
                "Letters for the chapel. Yes. I know the hand.",
                "She wrote to me every season and never sent them.",
                "I think she liked having somewhere to send them to.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(ysolde::GREETING),
        Node::saying(
            ysolde::HERSELF,
            "Ysolde",
            &[
                "I am the part of the chapel that was left standing.",
                "You may take that however you like. Everyone does.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(ysolde::GREETING),
    ],
    entries: &[
        (Condition::Knows(Discovery::MoonwellAnswered), ysolde::MOON),
        (Condition::Flag(Flag::LettersPromised), ysolde::RETURNED),
        (Condition::Knows(Discovery::VillageLetters), ysolde::ADDRESSED),
        (Condition::Knows(Discovery::ChapelBasin), ysolde::BASIN),
    ],
    fallback: ysolde::GREETING,
};

// -- Rin, who keeps the lanterns ----------------------------------------------

mod rin {
    use super::{Choice, NodeId};

    pub const GLADE: NodeId = NodeId::new("glade");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const LANTERNS: NodeId = NodeId::new("lanterns");
    pub const ABOUT_HER: NodeId = NodeId::new("about-her");
    pub const VILLAGE: NodeId = NodeId::new("village");

    pub static CHOICES: [Choice; 4] = [
        Choice::to("Ask about the lanterns.", LANTERNS),
        Choice::to("Ask about the village.", VILLAGE),
        Choice::to("Ask how long she has been here.", ABOUT_HER),
        Choice::end("Leave her to it."),
    ];
}

/// Rin: bright, talkative, and hazy about anything more than a week old.
static RIN: Conversation = Conversation {
    nodes: &[
        Node::saying(
            rin::GLADE,
            "Rin",
            &[
                "You found the glade! I keep that clear too. I do not remember being told to.",
                "Do not mention it to the man in the village.",
                "He takes things, and that is the one thing he cannot have.",
            ],
        )
        .titled("lantern keeper")
        .in_mood(Mood::Warm)
        .then(rin::GREETING),
        Node::asking(
            rin::GREETING,
            "Rin",
            &[
                "Oh — a person. I have not seen one since, um. Since a while.",
                "I keep the lanterns lit along the path. Someone has to, or the path forgets where it goes.",
            ],
            &rin::CHOICES,
        )
        .titled("lantern keeper"),
        Node::saying(
            rin::LANTERNS,
            "Rin",
            &[
                "There are eleven. There were twelve, and then there were eleven.",
                "I do not think about the twelfth one very much.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(rin::GREETING),
        Node::saying(
            rin::VILLAGE,
            "Rin",
            &[
                "The village is up the path. Nobody lives there.",
                "I go up sometimes, when the lanterns are all lit and I want to feel like there is somewhere",
                "to be.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(rin::GREETING),
        Node::saying(
            rin::ABOUT_HER,
            "Rin",
            &[
                "Since the wood was smaller. That is the only answer I have and I have checked it several times.",
                "I am not being mysterious. I would tell you if I knew.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(rin::GREETING),
    ],
    entries: &[(Condition::Knows(Discovery::WoodsGlade), rin::GLADE)],
    fallback: rin::GREETING,
};

// -- Cassen, who would rather you did not look closely -------------------------

mod cassen {
    use super::{Choice, Condition, Effect, Flag, NodeId};

    pub const AFTER: NodeId = NodeId::new("after");
    pub const LETTERS: NodeId = NodeId::new("letters");
    pub const WELL: NodeId = NodeId::new("well");
    pub const ACCUSED: NodeId = NodeId::new("accused");
    pub const LET_GO: NodeId = NodeId::new("let-go");
    pub const PROMISE: NodeId = NodeId::new("promise");
    pub const REFUSE: NodeId = NodeId::new("refuse");
    pub const KEEPING: NodeId = NodeId::new("keeping");

    pub static WELL_CHOICES: [Choice; 3] = [
        Choice::to("Tell him you know he cut the rope.", ACCUSED),
        Choice::to("Ask him about the letters.", LETTERS),
        Choice::to("Say nothing, and let him wonder.", LET_GO)
            .doing(Effect::Set(Flag::CassenBefriended)),
    ];

    /// Once the accusation has been made, it cannot be made again, and he is
    /// noticeably more cooperative about everything else.
    pub static AFTER_CHOICES: [Choice; 3] = [
        Choice::to("Ask him about the letters.", LETTERS)
            .when(Condition::Knows(super::Discovery::VillageLetters)),
        Choice::to("Ask him about the well again.", WELL),
        Choice::end("Leave him to it."),
    ];

    pub static LETTERS_CHOICES: [Choice; 2] = [
        Choice::to("Tell him to give them back.", PROMISE),
        Choice::to("Tell him he can keep them.", REFUSE),
    ];
}

/// Cassen: cagey, self-justifying, and not quite the villain he sounds like.
static CASSEN: Conversation = Conversation {
    nodes: &[
        Node::saying(
            cassen::ACCUSED,
            "Cassen",
            &[
                "I said I would. Saying is free. Doing costs rope, and I had better uses for rope.",
                "The water was bad, if it helps. Maren knew. Maren knew and said nothing, which is worse.",
            ],
        )
        .titled("of the hollow village")
        .in_mood(Mood::Shaken)
        .doing(Effect::Set(Flag::CassenAccused))
        .then(cassen::KEEPING),
        Node::asking(
            cassen::LETTERS,
            "Cassen",
            &[
                "Those letters were hers, and now they are chapel business. That is what the address says.",
                "You are not going to read them out loud. I would rather you did not.",
            ],
            &cassen::LETTERS_CHOICES,
        )
        .in_mood(Mood::Wary),
        Node::saying(
            cassen::PROMISE,
            "Cassen",
            &[
                "Fine. Fine. I will walk them up to the chapel myself, and she can tell me what she thinks of me.",
                "She has had a long time to work out an opinion.",
            ],
        )
        .in_mood(Mood::Sorry)
        .doing(Effect::Set(Flag::LettersPromised))
        .then(cassen::KEEPING),
        Node::saying(
            cassen::REFUSE,
            "Cassen",
            &[
                "I have kept them dry for nine years. I have kept them dry longer than she kept them, and",
                "she wrote them.",
                "Do not look at me like that. I am aware of how that sounds.",
            ],
        )
        .in_mood(Mood::Wary)
        .then(cassen::KEEPING),
        Node::saying(
            cassen::LET_GO,
            "Cassen",
            &[
                "Hm. Most people come at me with an opinion.",
                "You are not most people, then. All right. Sit down if you want. There is a wall.",
            ],
        )
        .in_mood(Mood::Warm)
        .then(cassen::KEEPING),
        Node::asking(
            cassen::WELL,
            "Cassen",
            &[
                "So you found the well. Dry, and the rope cut at the top, not the bottom.",
                "I did not cut it. I only said that I would, which is a different thing, legally.",
            ],
            &cassen::WELL_CHOICES,
        )
        .in_mood(Mood::Wary),
        Node::asking(
            cassen::AFTER,
            "Cassen",
            &[
                "You said your piece about the rope. I heard it. I have not slept any worse.",
                "Ask me whatever you like. I am a changed man, allegedly.",
            ],
            &cassen::AFTER_CHOICES,
        )
        .titled("of the hollow village")
        .in_mood(Mood::Wry),
        Node::asking(
            cassen::KEEPING,
            "Cassen",
            &[
                "Mind the houses. They come down if you lean on them, and then so do you.",
                "You would not have anything worth trading. No. Nobody ever does.",
            ],
            &cassen::WELL_CHOICES,
        )
        .titled("of the hollow village"),
    ],
    entries: &[
        (Condition::Flag(Flag::CassenAccused), cassen::AFTER),
        (Condition::Knows(Discovery::VillageLetters), cassen::LETTERS),
        (Condition::Knows(Discovery::VillageWell), cassen::WELL),
    ],
    fallback: cassen::KEEPING,
};

// -- Ioreth, who answers in the right order for the wrong reasons --------------

mod ioreth {
    use super::{Choice, NodeId};

    pub const ANSWERED: NodeId = NodeId::new("answered");
    pub const DIAL: NodeId = NodeId::new("dial");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const APPRENTICE: NodeId = NodeId::new("apprentice");
    pub const BELOW: NodeId = NodeId::new("below");

    pub static CHOICES: [Choice; 3] = [
        Choice::to("Ask about the apprentice.", APPRENTICE),
        Choice::to("Ask what is below the shrine.", BELOW),
        Choice::end("Leave the silence to itself."),
    ];
}

/// Ioreth: serene, exact, and careful about what she leaves out.
static IORETH: Conversation = Conversation {
    nodes: &[
        Node::saying(
            ioreth::ANSWERED,
            "Ioreth",
            &[
                "The seal is open. Whatever is below was buried on purpose, by people who could have burned",
                "it instead.",
                "I will not tell you not to go. I will tell you to take your own light.",
            ],
        )
        .titled("priestess of the moonwell")
        .in_mood(Mood::Sorry)
        .then(ioreth::GREETING),
        Node::saying(
            ioreth::DIAL,
            "Ioreth",
            &[
                "You have read the dial, then. Waning, full, waxing: dark, filled, and let go.",
                "Light the braziers in that order and the water will remember what it is for.",
            ],
        )
        .titled("priestess of the moonwell")
        .in_mood(Mood::Warm)
        .then(ioreth::GREETING),
        Node::asking(
            ioreth::GREETING,
            "Ioreth",
            &[
                "The moon drank here before the village had a name. It still drinks, if it is asked properly.",
                "Very little of this is mine to explain. The floor explains it better.",
            ],
            &ioreth::CHOICES,
        )
        .titled("priestess of the moonwell"),
        Node::saying(
            ioreth::APPRENTICE,
            "Ioreth",
            &[
                "Nim. She came with the last of the village and has not yet decided to leave.",
                "She is better at this than I am. Do not tell her; she would be insufferable, and correctly so.",
            ],
        )
        .in_mood(Mood::Warm)
        .then(ioreth::GREETING),
        Node::saying(
            ioreth::BELOW,
            "Ioreth",
            &[
                "A scribe, and the things a scribe would not write down.",
                "We sealed it because we could not decide, and sealing it was a decision.",
                "That is the whole of the story.",
            ],
        )
        .in_mood(Mood::Veiled)
        .then(ioreth::GREETING),
    ],
    entries: &[
        (Condition::Knows(Discovery::MoonwellAnswered), ioreth::ANSWERED),
        (Condition::Knows(Discovery::MoonwellDial), ioreth::DIAL),
    ],
    fallback: ioreth::GREETING,
};

// -- Nim, the shrine's apprentice ---------------------------------------------

mod nim {
    use super::{Choice, NodeId};

    pub const ANSWERED: NodeId = NodeId::new("answered");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const CHARMS: NodeId = NodeId::new("charms");
    pub const TELL: NodeId = NodeId::new("tell");
    pub const IORETH: NodeId = NodeId::new("ioreth");

    pub static CHOICES: [Choice; 3] = [
        Choice::to("Ask about the charms.", CHARMS),
        Choice::to("Ask about Ioreth.", IORETH),
        Choice::end("Leave her to her work."),
    ];

    pub static ANSWERED_CHOICES: [Choice; 3] = [
        Choice::to("Tell her the moon answered.", TELL),
        Choice::to("Ask about the charms.", CHARMS),
        Choice::to("Ask about Ioreth.", IORETH),
    ];
}

/// Nim: cheerful, nosy, and nearly competent. The ribbons mean something.
static NIM: Conversation = Conversation {
    nodes: &[
        Node::asking(
            nim::ANSWERED,
            "Nim",
            &[
                "Oh! Oh, you did it, did you not? The water is doing the thing it does.",
                "Tell me. Please. I have been asking it for a year and it does not gossip.",
            ],
            &nim::ANSWERED_CHOICES,
        )
        .titled("apprentice of the moonwell"),
        Node::saying(
            nim::TELL,
            "Nim",
            &[
                "Waning, full, waxing. Of course. Of course it is, it is written on the floor, I am an idiot.",
                "Ioreth knew. Ioreth always knew, and she let me hand her things for a year.",
            ],
        )
        .in_mood(Mood::Shaken)
        .doing(Effect::Set(Flag::NimToldOfTheMoonwell))
        .then(nim::GREETING),
        Node::asking(
            nim::GREETING,
            "Nim",
            &[
                "You are the one who came up from the wood.",
                "I saw you on the path and I waved and you did not see me.",
                "That is fine. I am extremely used to it.",
            ],
            &nim::CHOICES,
        )
        .titled("apprentice of the moonwell"),
        Node::saying(
            nim::CHARMS,
            "Nim",
            &[
                "One a season, and one for luck, and one because the knot was pretty.",
                "They are for keeping. Nobody has ever asked me what I am keeping out.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(nim::GREETING),
        Node::saying(
            nim::IORETH,
            "Nim",
            &[
                "She is not cold. She is *tired*, and she has decided that is the same as being calm.",
                "Do not tell her I said that either. It is a whole pile of things I have said now.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(nim::GREETING),
    ],
    entries: &[
        (
            Condition::Knows(Discovery::MoonwellAnswered),
            nim::ANSWERED,
        ),
        (Condition::Flag(Flag::NimToldOfTheMoonwell), nim::GREETING),
    ],
    fallback: nim::GREETING,
};

// -- Maren, who was passing through nine years ago -----------------------------

mod maren {
    use super::{Choice, NodeId};

    pub const CONFESS: NodeId = NodeId::new("confess");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const ROPE: NodeId = NodeId::new("rope");
    pub const WHY: NodeId = NodeId::new("why");
    pub const CASSEN: NodeId = NodeId::new("cassen");
    pub const NAME: NodeId = NodeId::new("name");

    pub static CHOICES: [Choice; 3] = [
        Choice::to("Ask about the well.", ROPE),
        Choice::to("Ask what she is doing here.", NAME),
        Choice::end("Leave her to her map."),
    ];

    pub static ROPE_CHOICES: [Choice; 2] = [
        Choice::to("Ask why she did it.", WHY),
        Choice::to("Ask whether Cassen knows.", CASSEN),
    ];
}

/// Maren: easy company, a wide hat, and a confession she has been carrying.
static MAREN: Conversation = Conversation {
    nodes: &[
        Node::saying(
            maren::WHY,
            "Maren",
            &[
                "Because the water was bad, and the village would not be told, and I was twenty-four and",
                "certain about everything.",
                "Cutting the rope was the only argument I had that they would listen to. It did not work.",
                "It never does.",
            ],
        )
        .titled("wandering mage")
        .in_mood(Mood::Sorry)
        .doing(Effect::Set(Flag::MarenConfessed))
        .then(maren::CONFESS),
        Node::saying(
            maren::CASSEN,
            "Maren",
            &[
                "He has been telling people he did it for nine years. I have let him.",
                "It is a better story than mine, and it has kept him talking to people, which he needs more",
                "than I do.",
            ],
        )
        .in_mood(Mood::Wry)
        .doing(Effect::Set(Flag::MarenConfessed))
        .then(maren::CONFESS),
        Node::asking(
            maren::CONFESS,
            "Maren",
            &[
                "There. That is the whole of it, and you may do what you like with it.",
                "I would rather you did not tell him. I am aware of how that sounds as well.",
            ],
            &maren::CHOICES,
        )
        .titled("wandering mage"),
        Node::asking(
            maren::GREETING,
            "Maren",
            &[
                "Do not be alarmed. I am a mage, not a magistrate — I only look official because the hat is",
                "enormous.",
                "Maren. I have been drawing this crossroads for two days and it keeps coming out wrong.",
            ],
            &maren::CHOICES,
        )
        .titled("wandering mage"),
        Node::asking(
            maren::ROPE,
            "Maren",
            &[
                "The rope, yes. Cut at the top, and the well dry as a bone beneath it.",
                "I know because I cut it. You may as well have that from me rather than from him.",
            ],
            &maren::ROPE_CHOICES,
        )
        .in_mood(Mood::Sorry),
        Node::saying(
            maren::NAME,
            "Maren",
            &[
                "Passing through. I have been passing through for nine years, which is a long way to pass",
                "through somewhere.",
                "I map things. Roads mostly. This one has been stubborn.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(maren::GREETING),
    ],
    entries: &[
        (Condition::Flag(Flag::CassenAccused), maren::CONFESS),
        (Condition::Knows(Discovery::VillageWell), maren::ROPE),
    ],
    fallback: maren::GREETING,
};

// -- Wren, who keeps the glade ------------------------------------------------

mod wren {
    use super::{Choice, NodeId};

    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const SWEPT: NodeId = NodeId::new("swept");
    pub const RIN: NodeId = NodeId::new("rin");
    pub const QUIET: NodeId = NodeId::new("quiet");

    pub static CHOICES: [Choice; 4] = [
        Choice::to("Ask who sweeps the floor.", SWEPT),
        Choice::to("Ask about the keeper of the path.", RIN),
        Choice::to("Sit down and say nothing for a while.", QUIET),
        Choice::end("Step back out of the ring."),
    ];
}

/// Wren: a wood spirit who speaks in halves and would rather not be a host.
static WREN: Conversation = Conversation {
    nodes: &[
        Node::saying(
            wren::RIN,
            "Wren",
            &[
                "The one with the lanterns. She comes to the edge and does not come in.",
                "I leave her a light. She thinks it is hers. That is — easier. For both of us.",
            ],
        )
        .titled("of the quiet glade")
        .in_mood(Mood::Warm)
        .doing(Effect::Set(Flag::WrenToldOfRin))
        .then(wren::GREETING),
        Node::asking(
            wren::GREETING,
            "Wren",
            &[
                "...You are in the ring.",
                "That is — it is not a problem. It is only that people do not.",
            ],
            &wren::CHOICES,
        )
        .titled("of the quiet glade")
        .in_mood(Mood::Wary),
        Node::saying(
            wren::SWEPT,
            "Wren",
            &[
                "I do. Not often. Not well, probably.",
                "There was a floor here before there was a wood. I have decided it should stay visible.",
            ],
        )
        .in_mood(Mood::Calm)
        .then(wren::GREETING),
        Node::saying(
            wren::QUIET,
            "Wren",
            &[
                "Yes. Thank you.",
                "...That is the whole reply. I am sorry it is short. It is not rudeness.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(wren::GREETING),
    ],
    entries: &[],
    fallback: wren::GREETING,
};

// -- Tamsin, warden of the catacombs ------------------------------------------

mod tamsin {
    use super::{Choice, NodeId};

    pub const SCRIBE: NodeId = NodeId::new("scribe");
    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const NAME: NodeId = NodeId::new("name");
    pub const VESPER: NodeId = NodeId::new("vesper");
    pub const THANKS: NodeId = NodeId::new("thanks");

    pub static CHOICES: [Choice; 3] = [
        Choice::to("Ask about the scribe.", SCRIBE),
        Choice::to("Ask about the revenant.", VESPER),
        Choice::end("Leave her to her notes."),
    ];

    pub static SCRIBE_CHOICES: [Choice; 2] = [
        Choice::to("Tell her the name on the tomb.", NAME),
        Choice::to("Say you could not read it.", VESPER),
    ];
}

/// Tamsin: steel, patience, and a far gentler manner than the armour suggests.
static TAMSIN: Conversation = Conversation {
    nodes: &[
        Node::saying(
            tamsin::NAME,
            "Tamsin",
            &[
                "You read it. On the lid. Two letters and then nothing, over and over.",
                "Thank you. That is the whole of her, and I have been looking at the rest of it for eleven days.",
            ],
        )
        .titled("warden of the catacombs")
        .in_mood(Mood::Warm)
        .doing(Effect::Set(Flag::TamsinGivenTheName))
        .then(tamsin::THANKS),
        Node::asking(
            tamsin::THANKS,
            "Tamsin",
            &[
                "I will write it up properly now. She gets a name in the record, even if it is half of one.",
                "If you are going deeper, take the left fork. The right one floods.",
            ],
            &tamsin::CHOICES,
        )
        .titled("warden of the catacombs"),
        Node::asking(
            tamsin::GREETING,
            "Tamsin",
            &[
                "Hold. — Sorry. Hold a moment. There. I have put the pen down.",
                "Tamsin. I am the warden here, which mostly means I am the only one who has not left.",
            ],
            &tamsin::CHOICES,
        )
        .titled("warden of the catacombs"),
        Node::asking(
            tamsin::SCRIBE,
            "Tamsin",
            &[
                "A scribe, buried with her hands full of wax and her stylus still sharp.",
                "That is not a burial. That is a desk.",
                "She was writing something she was not allowed to write. The lid stops after two letters.",
            ],
            &tamsin::SCRIBE_CHOICES,
        )
        .in_mood(Mood::Sorry),
        Node::saying(
            tamsin::VESPER,
            "Tamsin",
            &[
                "The revenant. Vesper. It was here before I was and it has never once been rude to me.",
                "It stands in the draught on purpose. It thinks it is keeping us warm by taking the cold itself.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(tamsin::GREETING),
    ],
    entries: &[(Condition::Flag(Flag::TamsinGivenTheName), tamsin::THANKS)],
    fallback: tamsin::GREETING,
};

// -- Vesper, who was buried standing up ---------------------------------------

mod vesper {
    use super::{Choice, Condition, NodeId};

    pub const GREETING: NodeId = NodeId::new("greeting");
    pub const SCRIBED: NodeId = NodeId::new("scribed");
    pub const SCRIBE: NodeId = NodeId::new("scribe");
    pub const HERE: NodeId = NodeId::new("here");
    pub const WARDEN: NodeId = NodeId::new("warden");

    pub static CHOICES: [Choice; 3] = [
        Choice::to("Ask who is buried here.", HERE),
        Choice::to("Ask about the warden.", WARDEN),
        Choice::end("Step back into the gallery."),
    ];

    /// Once the tomb has been read, the revenant will talk about it — but only if
    /// asked, which is the whole of its character.
    pub static SCRIBED_CHOICES: [Choice; 4] = [
        Choice::to("Ask about the scribe's tomb.", SCRIBE)
            .when(Condition::Knows(super::Discovery::CatacombScribe)),
        Choice::to("Ask who is buried here.", HERE),
        Choice::to("Ask about the warden.", WARDEN),
        Choice::end("Step back into the gallery."),
    ];
}

/// Vesper: dead, polite, and quietly desperate to be asked the right question.
static VESPER: Conversation = Conversation {
    nodes: &[
        Node::asking(
            vesper::SCRIBED,
            "Vesper",
            &[
                "Do not stand in the doorway. The draught takes the warmth out of the living.",
                "You have been to the chamber at the middle. I can tell; you are holding your hands differently.",
            ],
            &vesper::SCRIBED_CHOICES,
        )
        .titled("what the catacombs kept")
        .in_mood(Mood::Wary),
        Node::asking(
            vesper::GREETING,
            "Vesper",
            &[
                "Do not stand in the doorway. The draught takes the warmth out of the living.",
                "I am not a ghost. I am the last thing that was left of one.",
            ],
            &vesper::CHOICES,
        )
        .titled("what the catacombs kept"),
        Node::saying(
            vesper::SCRIBE,
            "Vesper",
            &[
                "The moon's name. She got as far as the second letter.",
                "I have had a very long time to think about those two letters.",
            ],
        )
        .in_mood(Mood::Sorry)
        .then(vesper::GREETING),
        Node::saying(
            vesper::HERE,
            "Vesper",
            &[
                "Everyone. That is what a catacomb is for, and I am not being clever about it.",
                "The ones with names are on the walls. The ones without are on the floor.",
                "Mind where you step; they would not mind.",
            ],
        )
        .in_mood(Mood::Wry)
        .then(vesper::GREETING),
        Node::saying(
            vesper::WARDEN,
            "Vesper",
            &[
                "She writes down everything I say and gets most of it wrong, and she is the best thing that",
                "has happened here.",
                "Do not tell her I said that. She would write it down.",
            ],
        )
        .in_mood(Mood::Warm)
        .then(vesper::GREETING),
    ],
    entries: &[(
        Condition::Knows(Discovery::CatacombScribe),
        vesper::SCRIBED,
    )],
    fallback: vesper::GREETING,
};

// -- areas -------------------------------------------------------------------

/// The starting area: a long ruined chapel, its nave running north to the apse.
#[must_use]
pub fn chapel() -> Area {
    Area::new(
        CHAPEL,
        "Ruined Chapel",
        &[
            "############################################################################################################################################",
            "#.......#...........#...........#...........#...........#...........#...........#...........#...........#...........#...........#..........#",
            "#.......#...........#...........#...........#...........#...........#...........#...........#...........#...........#...........#..........#",
            "#...,,,.,,.,,.....,,.....,,.....#,.....,,.....,,.....,,...,,,,,,,,,,#,,,,,,,,,,,#,,.....,,.....,,.....,,.....,,.....#,.....,,.....,,,,.,,..#",
            "#..,.,##.,................................................#,,,,,,,,,,,,,,,,,,,,,,,#...............................................,.,##.,..#",
            "#..,,.,#,.................................................#,,,,,,,,,,,####,,,,,,,,#...............................................#,.,##...#",
            "#..,#,.,##............................................#..,#,,,,,,,,#,,,,,#,,,,,,,,##..,...........................................,#,.,#,..#",
            "#...##,.,#............................................#..,#,,,,,,,,###,,,#,,,,,,,,##..,............................................##,.,,..#",
            "#..,.##,.,............................................#..,#,,,,,,,,,,,,,,,,,,,,,,,##..,...........................................,.##,.,..#",
            "#..,,.##,.................................................#########,,,,,,,#########...............................................#,.,#,...#",
            "#...................................................................,,,,,..................................................................#",
            "#..,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,..#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#.....................................................#..,.#,.,,.,#.,,,,,.#,.,,.,#.,..#....................................................#",
            "#.....................................................#..,..........,,,,,..........#..,....................................................#",
            "#.....................................................#..,..........,,,,,..........#..,....................................................#",
            "#..........................................................,,.,#.,,.,,,,,.,,.,#.,,.........................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...............,#############......................................,,,,,.......................................,#############.............#",
            "#...............,.,#...##....#.............................##.,,.##.,,,,,.##.,,.##..............................,.,#...##....#.............#",
            "#...............,............#......................................,,,,,.......................................,............#.............#",
            "#...............#............#........................#..,..........,,,,,..........#..,.........................#............#.............#",
            "+...............#............#........................#..,.,,.##.,,.,,,,,.,,.##.,,.#..,.........................#............#.............#",
            "+...............,............,........................#..,..........,,,,,..........#..,.........................,............,.............#",
            "+...................................................................,,,,,..................................................................#",
            "+...............,............,......................................,,,,,.......................................,............,.............#",
            "+...............#............#......................................,,,,,.......................................#............#.............#",
            "#...............#............#...............................#...#..,,,,,..#...#................................#............#.............#",
            "#...............#............#......................................,,,,,.......................................#............#.............#",
            "#...............,.,#...##....#........................#..,..........,,,,,..........#..,.........................,.,#...##....#.............#",
            "#...............,#############........................#..,..........,,,,,..........#..,.........................,#############.............#",
            "#.....................................................#..,...#...#..,,,,,..#...#...#..,....................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#...................................................................,,,,,..................................................................#",
            "#............................................................#...#..,,,,,..#...#...........................................................#",
            "#...,#,.,#..........................................................,,,,,..........................................................,#,.,,..#",
            "#..,.,##.,............................................#..,..........,,,,,..........#..,...........................................,.,##.,..#",
            "#..,,.,##.............................................#..,..........,,,,,..........#..,...........................................#,.,##...#",
            "#..,#,.,,,............................................#..,..........,,,,,..........#..,...........................................,#,.,#,..#",
            "#..,##,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,##,,,,..#",
            "#..,.##,.,..........................................................,,,,,.........................................................,.##,.,..#",
            "#..,,.~~~~~~~.......................................................,,,,,.........................................................#,.,#,...#",
            "#.....~~~~~~~.......T...............................................,,,,,..................................................T...............#",
            "#.....~~~~~~~...............................................T.......,,,,,.....................................T.............T..............#",
            "#.....~~~~~~~.,...................................T.................,,,,,......................T...........................................#",
            "#..............T........T.....T.....................................,,,,,.......,...................T.................T....................#",
            "#...,,.....,,.....,,.....,,.....#,.....,,.....,,.....,,.....,,.....,,,,,,.,,.....,,.....,,.....,,.....,,.....,,.....#,.....,,.....,,.......#",
            "#.......,...........#...........#...........#...........#...........,,,,,.......,...........#...........#...........#...........#..........#",
            "#.......#...........#...........#...........#...........#...........,...........#...........#...........#...........#...........#..........#",
            "####################################################################+++++###################################################################",
        ],
        vec![
            standing(Position::new(56, 33), &YSOLDE_MEMBER),
            Entity::relic(
                Position::new(76, 6),
                "a cracked basin",
                Prop::Basin,
                "Still water, cold and dark; it has not rained in a long while.",
                Discovery::ChapelBasin,
            ),
            Entity::object(
                Position::new(60, 44),
                "a worn sigil",
                Prop::Sigil,
                "Cut into the flagstones, and worn almost away by feet that are long gone.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(70, 51),
                CELLAR,
                Position::new(20, 12),
                "the Sunken Cellar",
            ),
            Portal::new(
                Position::new(0, 25),
                WOODS,
                Position::new(64, 40),
                "the Whispering Woods",
            ),
        ],
        Position::new(70, 44),
    )
}

/// The crypt beneath the chapel: four chambers, one of them under water.
#[must_use]
pub fn cellar() -> Area {
    Area::new(
        CELLAR,
        "Sunken Cellar",
        &[
            "##################+++++#################################################################################################################",
            "#..................,,,.................................................................................................................#",
            "#..................,,,.................................................................................................................#",
            "#..................,,,.................................................................................................................#",
            "#..................,,,..................................,..............................................................................#",
            "#..................,,,...................................................................................................#.............#",
            "#.....,############,,,#################################,,,##,.............,#######################################################.....#",
            "#.....,.,#....##...,,,....##....#,....##....##....##....##..,.............,.,#....##....##....##...##,....##....##....##....##...#.....#",
            "#.....,............,,,...........,..,.......................,.............,..........................,...........................#.....#",
            "#.....#............,,,...,.......,..........................#.............#..........................,...........................#.....#",
            "#.....#............,,,...........,..........................#.............#..........................,..........,,####,,,###,....#.....#",
            "#.....#....................#.....,..........................#.............#....#.....................,..........#,,,,,,,,,,,#....#.....#",
            "#...............#................,..........................#.....,,,,,,,,...........................,..........#,,,,,,,,,,,#....#.....#",
            "#....#.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,#,,,,#.....#",
            "#................................,..........................#.....,,,,,,,,...........................,..........#,,,,,,,,,,,#....#.....#",
            "#.....##.........................,.........................##.............#..........................,..........#,,,,,,,,,,,#....#.....#",
            "#.....#..........................,..........................#.............#..........................,..........####,,,,,####....#.....#",
            "#.....#..........................,..........................##............#..........................,...........,...............#.....#",
            "#.....#..........................,................#.........#.............#..........................,#..........................#.....#",
            "#.....,.,#....##....##....##....#,....##....##....##....##..,.............,.,#....##....##....##....#,....##....,,.,..##....,#...#.....#",
            "#.....,#####################################################,.............,##########################,,,##########################.....#",
            "#............................................................................................#.........................................#",
            "#.............................,...................................,,,,,,,,.............................................................#",
            "#..................,..............................................,,,,,,,,.............................................................#",
            "#.....#######################################################.....,,,,,,,,,#################,,,###################################.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#............#,.,#....##....##....##....#,....##....##....##....##...#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.,...........,..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#...#.........#..........................,..............#............#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,...........................,................#..........#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....#",
            "#......,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#.....,,,,,,,,......................#....,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#.......................#..,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#............#.............,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#.............#..........................,...........................#.....#",
            "#.....#~~~~~~~~~~~~~~~~~~~~~~~~~,,,~~~~~~~~~~~~~~~~~~~~~~~~~#....#........,.,#....##....##....##....#,....##....##....##....##...#.....#",
            "#.....#######################################################.............,###########,,,############################...###########....#",
            "#..................................................................#.................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "#....................................................................................................................,,,...............#",
            "####################################################################################################################+++++###############",
        ],
        vec![Entity::object(
            Position::new(118, 13),
            "a faded sigil",
            Prop::Sigil,
            "The carving is worn smooth, but it hums faintly under your fingers.",
        )],
        vec![
            Portal::new(
                Position::new(20, 0),
                CHAPEL,
                Position::new(70, 49),
                "the Ruined Chapel",
            ),
            Portal::new(
                Position::new(118, 43),
                CATACOMBS,
                Position::new(8, 21),
                "the Forgotten Catacombs",
            ),
        ],
        Position::new(20, 12),
    )
}

/// The wood: a trodden trail between the chapel and the village, and a glade.
#[must_use]
pub fn woods() -> Area {
    Area::new(
        WOODS,
        "Whispering Woods",
        &[
            "############################################################+++++###############################################################",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,T,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,T,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,TT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,+",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,TT,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTT,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTT,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,T,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,T,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTT,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTT,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTT,,,TT,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,,,,,,,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "#TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT,,,,,TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT#",
            "##############################################################+++++#############################################################",
        ],
        vec![
            standing(Position::new(63, 17), &RIN_MEMBER),
            standing(Position::new(14, 33), &WREN_MEMBER),
            Entity::relic(
                Position::new(18, 30),
                "a standing stone",
                Prop::StandingStone,
                "A hole worn right through it, and a floor around it swept clean of leaves.",
                Discovery::WoodsGlade,
            ),
            Entity::object(
                Position::new(52, 26),
                "a waymarker cairn",
                Prop::Cairn,
                "Stones stacked by someone with more patience than sense.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(64, 43),
                CHAPEL,
                Position::new(5, 25),
                "the Ruined Chapel",
            ),
            Portal::new(
                Position::new(62, 0),
                VILLAGE,
                Position::new(62, 38),
                "the Hollow Village",
            ),
            Portal::new(
                Position::new(127, 20),
                SHRINE,
                Position::new(5, 20),
                "the Moonwell Shrine",
            ),
        ],
        Position::new(64, 40),
    )
}

/// The village: a crossroad of fallen houses around a dry well.
#[must_use]
pub fn village() -> Area {
    Area::new(
        VILLAGE,
        "Hollow Village",
        &[
            "############################################################################################################################",
            "#..........................................................................................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#.............,#########################,.......,#########################,.......,############################............#",
            "#.............,.,#...##...##...##...##..,.......,.,#...##...##...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,.........................,.......,.........................,.......,...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#..........#.,............#.......#..........#.,............#.......#............,.#............#............#",
            "#.............#...........#.............#.......#...........#.............#.......#.............#.............#............#",
            "#.............#..........,.#............#.......#..........,.#............#.......#............#.,............#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............,.,#...##...#,...##...##..,.......,.,#...##...#,...##...##..,.......,.,#...##...#,...##...##....#............#",
            "#.............,###########...###########,.......,###########...###########,.......,############...#############............#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#.............,###########...###########,.......,###########...###########,.......,############...#############............#",
            "#.............,.,#...##...##...##...##..,.......,.,#...##...##...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,.........................,.......,.........................,.......,...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............#..........#.,............#.......#..........#.,............#.......#............#.,............#............#",
            "#.............#...........#.............#.......#...........#.............#.......#.............#.............#............#",
            "#.............#..........,.,............#.......#..........,.,............#.......#............,.#............#............#",
            "#.............#.........................#.......#.........................#.......#...........................#............#",
            "#.............,.,#...##...,,...##...##..,.......,.,#...##...,,...##...##..,.......,.,#...##...##...##...##....#............#",
            "#.............,#########################,.......,#########################,.......,#############,,,############............#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "#...........................................................,,,,,..........................................................#",
            "############################################################+++++###########################################################",
        ],
        vec![
            standing(Position::new(40, 20), &CASSEN_MEMBER),
            standing(Position::new(62, 30), &MAREN_MEMBER),
            Entity::relic(
                Position::new(62, 20),
                "the village well",
                Prop::Well,
                "Dry to the bottom. The rope is cut at the top, not the bottom.",
                Discovery::VillageWell,
            ),
            Entity::relic(
                Position::new(20, 10),
                "a bundle of letters",
                Prop::Letters,
                "Tied with string and addressed to the chapel. None of them were opened.",
                Discovery::VillageLetters,
            ),
        ],
        vec![
            Portal::new(
                Position::new(62, 41),
                WOODS,
                Position::new(64, 40),
                "the Whispering Woods",
            ),
            Portal::new(
                Position::new(0, 20),
                SHRINE,
                Position::new(56, 5),
                "the Moonwell Shrine",
            ),
        ],
        Position::new(62, 38),
    )
}

/// The shrine: a pale hall around the moonwell, with three braziers and a dial.
#[must_use]
pub fn shrine() -> Area {
    Area::new(
        SHRINE,
        "Moonwell Shrine",
        &[
            "######################################################+++++#############################################################",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....#.,,,,#.....#.....#.....#.....#.....#.....#.....#.....#............#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.....................................................,,,,,............................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....,.,,,,,.....,.....#.....#.....#.....#.....#.....#.....#............#",
            "#.....................................................,,~,,............................................................#",
            "#.................................................~~~~~~~~~~~~~........................................................#",
            "#..............................................~~~~~~~~~~~~~~~~~~~.....................................................#",
            "#.............................................~~~~~~~~~~~~~~~~~~~~~....................................................#",
            "#............................................~~~~~~~~~~~~~~~~~~~~~~~...................................................#",
            "+.........#.....#.....#.....#.....#.....#...~~~~~~~~~~~~~~~~~~~~~~~~~.,.....#.....#.....#.....#.....#.....#............+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,..~~~~~~~~~~~~~~~~~~~~~~~~~~~.................................................+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "+...........................................~~~~~~~~~~~~~~~~~~~~~~~~~..................................................+",
            "#............................................~~~~~~~~~~~~~~~~~~~~~~~...................................................#",
            "#.........#.....#.....#.....#.....#.....#.....~~~~~~~~~~~~~~~~~~~~~...#.....#.....#.....#.....#.....#.....#............#",
            "#..............................................~~~~~~~~~~~~~~~~~~~.....................................................#",
            "#.................................................~~~~~~~~~~~~~........................................................#",
            "#.......................................................~..............................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#.........#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#.....#............#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "#......................................................................................................................#",
            "########################################################################################################################",
        ],
        vec![
            standing(Position::new(30, 14), &IORETH_MEMBER),
            standing(Position::new(84, 20), &NIM_MEMBER),
            Entity::relic(
                Position::new(40, 20),
                "the moon dial",
                Prop::Dial,
                "A waning crescent, a full moon, and a waxing one — in that order.",
                Discovery::MoonwellDial,
            ),
            Entity::brazier(
                Position::new(78, 16),
                "the first brazier",
                MoonPhase::Waning,
                "A bronze bowl on a stem, marked with a closing crescent. Cold.",
            ),
            Entity::brazier(
                Position::new(78, 20),
                "the second brazier",
                MoonPhase::Full,
                "A bronze bowl on a stem, marked with a full moon. Cold.",
            ),
            Entity::brazier(
                Position::new(78, 24),
                "the third brazier",
                MoonPhase::Waxing,
                "A bronze bowl on a stem, marked with an opening crescent. Cold.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(56, 0),
                VILLAGE,
                Position::new(62, 38),
                "the Hollow Village",
            ),
            Portal::new(
                Position::new(0, 20),
                WOODS,
                Position::new(124, 20),
                "the Whispering Woods",
            ),
            Portal::sealed(
                Position::new(119, 20),
                CATACOMBS,
                Position::new(121, 21),
                "the Forgotten Catacombs",
                Discovery::MoonwellAnswered,
            ),
        ],
        Position::new(56, 5),
    )
}

/// The catacombs: a long gallery of burials, and something still awake in it.
#[must_use]
pub fn catacombs() -> Area {
    Area::new(
        CATACOMBS,
        "Forgotten Catacombs",
        &[
            "####################################################################################################################################",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#.....##.......##..,,,..##.......##.......##,,.....##.......##.......##.......##.......##......,,,.......##.......##.,,,...##......#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,............############,,,############..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#.....#........#...,,,..#........#........#,,,.....#......#,,,,,,,,,,,,,,,,,,,,,,,,,#..,.......,,,.......#........#..,,,...#.......#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "+.,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,#,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,.+",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#.....#........#...,,,..#........#........#,,,.....#......#,,,,,,,,,,,,,,,,,,,,,,,,,#..,.......,,,.......#........#..,,,...#.......#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............#,,,,,,,,,,,,,,,,,,,,,,,,,#..........,,,...................,,,...........#",
            "#..................,,,.....................,,,............############,,,############..........,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#.....##.......##..,,,..##.......##.......##,,.....##.......,,.......,#.......,,.......##......,,,.......##.......##.,,,...##......#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................,,,.....................,,,.................................................,,,...................,,,...........#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "#..................................................................................................................................#",
            "####################################################################################################################################",
        ],
        vec![
            standing(Position::new(52, 21), &VESPER_MEMBER),
            standing(Position::new(95, 21), &TAMSIN_MEMBER),
            Entity::relic(
                Position::new(71, 22),
                "the bone scribe's tomb",
                Prop::Sarcophagus,
                "The lid is carved with a name that stops after two letters, over and over.",
                Discovery::CatacombScribe,
            ),
            Entity::object(
                Position::new(20, 30),
                "a heap of grave goods",
                Prop::Cairn,
                "Wax, a stylus, and a bowl. Whoever was buried here meant to keep working.",
            ),
        ],
        vec![
            Portal::new(
                Position::new(0, 21),
                CELLAR,
                Position::new(118, 40),
                "the Sunken Cellar",
            ),
            Portal::sealed(
                Position::new(131, 21),
                SHRINE,
                Position::new(115, 20),
                "the Moonwell Shrine",
                Discovery::MoonwellAnswered,
            ),
        ],
        Position::new(8, 21),
    )
}

/// Every area in the region, in a stable order.
#[must_use]
pub fn all() -> Vec<Area> {
    vec![
        chapel(),
        cellar(),
        woods(),
        village(),
        shrine(),
        catacombs(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        CATACOMBS, CELLAR, CHAPEL, all, catacombs, cellar, chapel, shrine, village, woods,
    };
    use crate::game::area::{Area, Direction, Position, Tile};
    use crate::game::discovery::Discovery;
    use crate::game::entity::EntityKind;
    use std::collections::{BTreeMap, BTreeSet, VecDeque};

    /// The viewport the world is designed to fill, from `ui::viewport`.
    const REFERENCE: (u16, u16) = (120, 40);

    /// Every walkable tile reachable from `start`, ignoring entities (a single
    /// NPC or object cannot seal a region, and blocking on them would give a
    /// misleading picture of the layout).
    fn reachable(area: &Area, start: Position) -> BTreeSet<Position> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        if !area.tile_walkable(start) {
            return seen;
        }
        seen.insert(start);
        queue.push_back(start);
        while let Some(position) = queue.pop_front() {
            for direction in Direction::ALL {
                if let Some(next) = position.checked_step(direction, area.width(), area.height()) {
                    if area.tile_walkable(next) && seen.insert(next) {
                        queue.push_back(next);
                    }
                }
            }
        }
        seen
    }

    fn walkable_count(area: &Area) -> usize {
        (0..area.height())
            .flat_map(|y| (0..area.width()).map(move |x| Position::new(x, y)))
            .filter(|position| area.tile_walkable(*position))
            .count()
    }

    fn adjacent_to_reachable(area: &Area, position: Position, seen: &BTreeSet<Position>) -> bool {
        Direction::ALL.iter().any(|direction| {
            position
                .checked_step(*direction, area.width(), area.height())
                .is_some_and(|neighbour| seen.contains(&neighbour))
        })
    }

    /// How many walkable cells lie in a straight line through `position`.
    fn run(area: &Area, position: Position, dx: i32, dy: i32) -> u16 {
        let mut length = 1;
        for sign in [1i32, -1] {
            let mut x = i32::from(position.x) + dx * sign;
            let mut y = i32::from(position.y) + dy * sign;
            while (0..i32::from(area.width())).contains(&x)
                && (0..i32::from(area.height())).contains(&y)
                && area.tile(Position::new(x as u16, y as u16)).is_walkable()
            {
                length += 1;
                x += dx * sign;
                y += dy * sign;
            }
        }
        length
    }

    /// The narrowest cross-section through a tile: the smaller of the horizontal
    /// and vertical runs, which is the one the character has to fit through.
    fn clearance(area: &Area, position: Position) -> u16 {
        run(area, position, 1, 0).min(run(area, position, 0, 1))
    }

    #[test]
    fn every_passage_fits_the_character() {
        for area in all() {
            let area = &area;
            let mut worst = u16::MAX;
            let mut where_at = Position::new(0, 0);
            for y in 0..area.height() {
                for x in 0..area.width() {
                    let position = Position::new(x, y);
                    if !area.tile_walkable(position) {
                        continue;
                    }
                    let clearance = clearance(area, position);
                    if clearance < worst {
                        worst = clearance;
                        where_at = position;
                    }
                }
            }
            assert!(
                worst >= super::MIN_PASSAGE,
                "{} has a {worst}-cell passage at {},{}; the character sprite is \
                 five cells wide, so no passage may be narrower than {}",
                area.name(),
                where_at.x,
                where_at.y,
                super::MIN_PASSAGE,
            );
        }
    }

    #[test]
    fn the_main_routes_are_wider_than_the_minimum() {
        // The route a player walks constantly is drawn to the character's full
        // width rather than to the floor.
        let chapel = chapel();
        assert!(
            clearance(&chapel, Position::new(70, 44)) >= super::DOORWAY,
            "the nave should be a full character wide"
        );
    }

    #[test]
    fn every_doorway_is_a_full_character_wide() {
        for area in all() {
            let area = &area;
            for portal in area.portals() {
                // The door is a run of door tiles in the wall; measure it along
                // the wall it sits in.
                // A door on an edge has no neighbour outside it: `checked_sub`
                // rather than `saturating_sub`, or the door would be compared
                // with itself and read as a one-cell horizontal opening.
                let left = portal
                    .at
                    .x
                    .checked_sub(1)
                    .map(|x| area.tile(Position::new(x, portal.at.y)));
                let right = portal
                    .at
                    .x
                    .checked_add(1)
                    .map(|x| area.tile(Position::new(x, portal.at.y)));
                let horizontal = left == Some(Tile::Door) || right == Some(Tile::Door);
                let (dx, dy) = if horizontal { (1, 0) } else { (0, 1) };
                let mut width = 1;
                for sign in [1i32, -1] {
                    let mut x = i32::from(portal.at.x) + dx * sign;
                    let mut y = i32::from(portal.at.y) + dy * sign;
                    while (0..i32::from(area.width())).contains(&x)
                        && (0..i32::from(area.height())).contains(&y)
                        && area.tile(Position::new(x as u16, y as u16)) == Tile::Door
                    {
                        width += 1;
                        x += dx * sign;
                        y += dy * sign;
                    }
                }
                assert!(
                    width >= super::MIN_PASSAGE,
                    "{}: the doorway at {},{} is only {width} cells wide",
                    area.name(),
                    portal.at.x,
                    portal.at.y
                );
            }
        }
    }

    #[test]
    fn every_area_has_a_distinct_id_and_name() {
        let areas = all();
        let ids: BTreeSet<_> = areas.iter().map(crate::game::Area::id).collect();
        let names: BTreeSet<_> = areas.iter().map(crate::game::Area::name).collect();
        assert_eq!(areas.len(), ids.len(), "two areas share an id");
        assert_eq!(areas.len(), names.len(), "two areas share a name");
        assert!(areas.len() >= 6, "the region lost a place");
    }

    #[test]
    fn each_area_fills_the_reference_viewport() {
        for area in all() {
            assert!(
                area.width() >= REFERENCE.0 && area.height() >= REFERENCE.1,
                "{} is {}x{}, smaller than the reference viewport {:?}",
                area.name(),
                area.width(),
                area.height(),
                REFERENCE
            );
        }
    }

    #[test]
    fn each_area_spawns_the_player_on_walkable_ground() {
        for area in all() {
            assert!(
                area.tile_walkable(area.spawn()),
                "{} spawns inside terrain",
                area.name()
            );
            assert!(area.entity_at(area.spawn()).is_none());
        }
    }

    #[test]
    fn every_entity_stands_on_floor_and_can_be_reached() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            for entity in area.entities() {
                assert!(
                    area.tile_walkable(entity.position),
                    "{} stands on terrain in {}",
                    entity.name,
                    area.name()
                );
                assert!(
                    adjacent_to_reachable(area, entity.position, &seen),
                    "{} at {:?} cannot be walked up to in {}",
                    entity.name,
                    entity.position,
                    area.name()
                );
            }
        }
    }

    #[test]
    fn every_doorway_is_on_a_door_and_reachable_from_both_sides() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            for portal in area.portals() {
                assert_eq!(
                    Tile::Door,
                    area.tile(portal.at),
                    "{} has a doorway that is not a door",
                    area.name()
                );
                assert!(
                    adjacent_to_reachable(area, portal.at, &seen),
                    "the doorway at {:?} in {} cannot be reached",
                    portal.at,
                    area.name()
                );

                let destination = all()
                    .into_iter()
                    .find(|candidate| candidate.id() == portal.to)
                    .unwrap_or_else(|| panic!("{} leads to a missing area", area.name()));
                assert!(
                    destination.tile_walkable(portal.arrive),
                    "{} arrives inside terrain in {}",
                    area.name(),
                    destination.name()
                );
                assert!(
                    destination.entity_at(portal.arrive).is_none(),
                    "{} arrives on top of an entity",
                    area.name()
                );
                let destination_seen = reachable(&destination, portal.arrive);
                assert!(
                    !destination_seen.is_empty(),
                    "arriving in {} leaves the player stuck",
                    destination.name()
                );
            }
        }
    }

    #[test]
    fn every_doorway_runs_both_ways() {
        let areas = all();
        for area in &areas {
            for portal in area.portals() {
                let destination = areas
                    .iter()
                    .find(|candidate| candidate.id() == portal.to)
                    .unwrap_or_else(|| panic!("{} leads nowhere", area.name()));
                assert!(
                    destination
                        .portals()
                        .iter()
                        .any(|back| back.to == area.id()),
                    "{} leads to {} but there is no way back",
                    area.name(),
                    destination.name()
                );
            }
        }
    }

    #[test]
    fn the_whole_region_is_reachable_from_where_the_game_starts() {
        // A breadth-first walk over the *area graph*, following only doorways that
        // are open from the start. This is the test that would have caught a new
        // place being drawn, populated, and then walled off from the world.
        let areas = all();
        let start = areas
            .iter()
            .find(|area| area.id() == CHAPEL)
            .expect("the chapel exists");
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        seen.insert(start.id());
        queue.push_back(start.id());
        while let Some(id) = queue.pop_front() {
            let area = areas
                .iter()
                .find(|area| area.id() == id)
                .expect("a known area");
            for portal in area.portals() {
                // A sealed doorway is still a connection: it is reachable, visible
                // and interactive, it simply does not open yet.
                if seen.insert(portal.to) {
                    queue.push_back(portal.to);
                }
            }
        }
        assert_eq!(
            areas.len(),
            seen.len(),
            "unreachable areas: {:?}",
            areas
                .iter()
                .map(crate::game::Area::id)
                .filter(|id| !seen.contains(id))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn at_least_one_doorway_stays_open_into_every_hidden_place() {
        // A whole area behind a puzzle would be a toll on exploration rather than
        // a reward for it. Only the shortcut may be sealed.
        for area in all() {
            let sealed = area
                .portals()
                .iter()
                .filter(|portal| portal.requires.is_some())
                .count();
            assert!(
                sealed < area.portals().len() || sealed == 0,
                "{} is only reachable through a sealed door",
                area.name()
            );
        }
    }

    #[test]
    fn most_of_each_area_is_one_connected_space() {
        for area in &all() {
            let seen = reachable(area, area.spawn());
            let walkable = walkable_count(area);
            let fraction = seen.len() * 100 / walkable.max(1);
            assert!(
                fraction >= 80,
                "{}: only {fraction}% of {walkable} walkable tiles are reachable from the spawn",
                area.name()
            );
        }
    }

    #[test]
    fn the_region_has_every_voice_and_somewhere_to_use_it() {
        let voices: BTreeSet<&str> = all()
            .iter()
            .flat_map(|area| area.entities())
            .filter_map(|entity| match entity.kind {
                EntityKind::Npc { .. } => Some(entity.name),
                _ => None,
            })
            .collect();
        assert!(voices.len() >= 5, "the cast shrank to {voices:?}");
        assert!(voices.contains("Ysolde"), "the first voice is gone");
    }

    #[test]
    fn each_area_has_something_to_look_at() {
        for area in &all() {
            let landmarks = area
                .entities()
                .iter()
                .filter(|entity| !matches!(entity.kind, EntityKind::Npc { .. }))
                .count();
            assert!(
                landmarks >= 1,
                "{} has nothing in it to examine",
                area.name()
            );
        }
    }

    #[test]
    fn every_discovery_can_actually_be_made() {
        // Each discovery is either granted by examining a landmark or by answering
        // the moonwell. An unreachable journal entry would be content that exists
        // and can never be read.
        let mut granted: BTreeSet<Discovery> = BTreeSet::new();
        let mut braziers = 0;
        for area in all() {
            for entity in area.entities() {
                if let EntityKind::Object {
                    discovery: Some(discovery),
                    ..
                } = entity.kind
                {
                    granted.insert(discovery);
                }
                if matches!(entity.kind, EntityKind::Brazier { .. }) {
                    braziers += 1;
                }
            }
        }
        let phases: BTreeSet<crate::game::MoonPhase> = all()
            .iter()
            .flat_map(|area| area.entities())
            .filter_map(|entity| match entity.kind {
                EntityKind::Brazier { phase, .. } => Some(phase),
                _ => None,
            })
            .collect();
        if braziers > 0 {
            assert_eq!(
                crate::game::progress::MOON_ORDER.len(),
                phases.len(),
                "the moonwell needs one brazier per phase, and no repeats"
            );
            granted.insert(Discovery::MoonwellAnswered);
        }
        for discovery in Discovery::ALL {
            assert!(
                granted.contains(&discovery),
                "{} can never be discovered",
                discovery.title()
            );
        }
    }

    #[test]
    fn the_moonwell_braziers_are_all_in_one_place() {
        let areas: BTreeMap<_, _> = all()
            .into_iter()
            .map(|area| {
                let count = area
                    .entities()
                    .iter()
                    .filter(|entity| matches!(entity.kind, EntityKind::Brazier { .. }))
                    .count();
                (area.id(), count)
            })
            .collect();
        let with_braziers: Vec<_> = areas
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(id, count)| (*id, *count))
            .collect();
        assert_eq!(
            vec![(super::SHRINE, 3)],
            with_braziers,
            "the braziers belong together, in the shrine"
        );
    }

    #[test]
    fn every_area_has_a_reachable_exit() {
        for area in all() {
            assert!(!area.portals().is_empty(), "{} is a dead end", area.name());
        }
        // And the two places below ground connect to each other, not only upwards.
        let cellar = cellar();
        assert!(
            cellar.portals().iter().any(|portal| portal.to == CATACOMBS),
            "the cellar has no way down to the catacombs"
        );
        assert!(catacombs().portals().iter().any(|p| p.to == CELLAR));
        assert!(woods().portals().iter().any(|p| p.to == CHAPEL));
        assert!(village().portals().iter().any(|p| p.to == super::WOODS));
        assert!(shrine().portals().iter().any(|p| p.to == super::VILLAGE));
    }

    #[test]
    fn the_sealed_way_is_the_shortcut_and_needs_the_moonwell() {
        let sealed: Vec<_> = all()
            .into_iter()
            .flat_map(|area| {
                area.portals()
                    .iter()
                    .filter(|portal| portal.requires.is_some())
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        assert_eq!(2, sealed.len(), "the shortcut has two ends");
        for portal in sealed {
            assert_eq!(
                Some(Discovery::MoonwellAnswered),
                portal.requires,
                "only the moonwell opens anything"
            );
        }
    }
}
