//! Conversations: branching dialogue as data.
//!
//! A conversation is a small graph of [`Node`]s with [`Choice`]s between them.
//! It is **content, not code**: a node names its speaker, its lines, its
//! expression, what it does, and where it goes next. Nothing here knows how any
//! of it is drawn, and nothing here mutates game state — a conversation *asks*
//! for effects and the world applies them.
//!
//! # Why a graph and not a script
//!
//! The previous phase gave each character a default exchange plus variants
//! unlocked by discoveries. That is the same idea as an entry [`Condition`], so
//! it was generalised rather than replaced: characters still react to what the
//! player knows, and now they can also ask a question and act on the answer.
//!
//! # Conditions, effects and stability
//!
//! [`Condition`]s gate entries and choices; [`Effect`]s record what a
//! conversation did. Both refer to things by their stable save id, so a
//! conversation may be rewritten freely — reordering nodes, changing lines,
//! merging branches — without disturbing anyone's save. A node's id is only
//! meaningful *within its own conversation*, and is not persisted.

use super::discovery::Discovery;

/// A stable id for a node, unique within its conversation.
///
/// Not persisted: a conversation always starts from its entry, so node ids may be
/// renamed as long as every reference moves with them. The graph-integrity test
/// checks exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(&'static str);

impl NodeId {
    /// Creates a node id.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        NodeId(id)
    }

    /// The slug.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Something the player has done, remembered across saves.
///
/// Deliberately a small, closed set: a flag is a *story fact*, not a variable.
/// Adding a hundred of them would be a quest system by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Flag {
    /// Cassen was told that his lie about the rope does not hold.
    CassenAccused,
    /// Cassen was left alone, and repaid it with the truth about the letters.
    CassenBefriended,
    /// The letters were promised back to the chapel.
    LettersPromised,
    /// Maren admitted what she took from the village.
    MarenConfessed,
    /// Nim was told what the moonwell did.
    NimToldOfTheMoonwell,
    /// Wren was told about the glade's keeper.
    WrenToldOfRin,
    /// Tamsin was given the scribe's name.
    TamsinGivenTheName,
    /// Ysolde was told that the moon was answered.
    YsoldeToldOfTheMoonwell,
}

impl Flag {
    /// Every flag the game can set.
    pub const ALL: [Flag; 8] = [
        Flag::CassenAccused,
        Flag::CassenBefriended,
        Flag::LettersPromised,
        Flag::MarenConfessed,
        Flag::NimToldOfTheMoonwell,
        Flag::WrenToldOfRin,
        Flag::TamsinGivenTheName,
        Flag::YsoldeToldOfTheMoonwell,
    ];

    /// The stable slug written to save data.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Flag::CassenAccused => "cassen-accused",
            Flag::CassenBefriended => "cassen-befriended",
            Flag::LettersPromised => "letters-promised",
            Flag::MarenConfessed => "maren-confessed",
            Flag::NimToldOfTheMoonwell => "nim-told",
            Flag::WrenToldOfRin => "wren-told",
            Flag::TamsinGivenTheName => "tamsin-given-name",
            Flag::YsoldeToldOfTheMoonwell => "ysolde-told",
        }
    }

    /// The flag with this slug, if there is one.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Flag> {
        Flag::ALL.into_iter().find(|flag| flag.id() == id)
    }
}

/// Whether something in a conversation is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    /// The player has found this.
    Knows(Discovery),
    /// The player has *not* found this.
    NotKnows(Discovery),
    /// This happened earlier.
    Flag(Flag),
    /// This has not happened.
    NotFlag(Flag),
}

/// Something a node or a choice does when it is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// Record a discovery, exactly as examining a relic would.
    Learn(Discovery),
    /// Remember that something happened.
    Set(Flag),
}

/// One option in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice {
    /// What the option says.
    pub text: &'static str,
    /// Whether it appears at all. A hidden option is not a locked one: it is not
    /// shown, so it cannot advertise a secret.
    pub shows: Option<Condition>,
    /// The node this leads to, or `None` to end the conversation here.
    pub next: Option<NodeId>,
    /// What taking this option does.
    pub effect: Option<Effect>,
}

impl Choice {
    /// A plain option leading to `next`.
    #[must_use]
    pub const fn to(text: &'static str, next: NodeId) -> Self {
        Choice {
            text,
            shows: None,
            next: Some(next),
            effect: None,
        }
    }

    /// An option that ends the conversation.
    #[must_use]
    pub const fn end(text: &'static str) -> Self {
        Choice {
            text,
            shows: None,
            next: None,
            effect: None,
        }
    }

    /// Shows this option only when `condition` holds.
    #[must_use]
    pub const fn when(mut self, condition: Condition) -> Self {
        self.shows = Some(condition);
        self
    }

    /// Does something when taken.
    #[must_use]
    pub const fn doing(mut self, effect: Effect) -> Self {
        self.effect = Some(effect);
        self
    }
}

/// How a speaker is holding themselves, for the portrait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mood {
    /// Composed, saying what they mean.
    #[default]
    Calm,
    /// Pleased about something.
    Warm,
    /// Guarded, or suspicious.
    Wary,
    /// Something is wrong and they know it.
    Sorry,
    /// Amused, and not hiding it.
    Wry,
    /// Saying less than they know.
    Veiled,
    /// Surprised, or caught out.
    Shaken,
}

/// One beat of a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Node {
    /// Used to be pointed at; see [`NodeId`].
    pub id: NodeId,
    /// Who is talking.
    pub speaker: &'static str,
    /// A short epithet under the name, when it adds something.
    pub title: Option<&'static str>,
    /// What they say, in order. Kept short: a long speech belongs in two nodes.
    pub lines: &'static [&'static str],
    /// Their expression.
    pub mood: Mood,
    /// What the player may say back. Empty means the node simply continues.
    pub choices: &'static [Choice],
    /// Where `enter` goes when there is nothing to choose.
    pub next: Option<NodeId>,
    /// What reaching this node does.
    pub effect: Option<Effect>,
}

impl Node {
    /// A node with no choices that moves straight on.
    #[must_use]
    pub const fn saying(id: NodeId, speaker: &'static str, lines: &'static [&'static str]) -> Self {
        Node {
            id,
            speaker,
            title: None,
            lines,
            mood: Mood::Calm,
            choices: &[],
            next: None,
            effect: None,
        }
    }

    /// A node that offers choices.
    #[must_use]
    pub const fn asking(
        id: NodeId,
        speaker: &'static str,
        lines: &'static [&'static str],
        choices: &'static [Choice],
    ) -> Self {
        Node {
            id,
            speaker,
            title: None,
            lines,
            mood: Mood::Calm,
            choices,
            next: None,
            effect: None,
        }
    }

    /// Sets the speaker's expression.
    #[must_use]
    pub const fn in_mood(mut self, mood: Mood) -> Self {
        self.mood = mood;
        self
    }

    /// Sets the epithet shown under the speaker's name.
    #[must_use]
    pub const fn titled(mut self, title: &'static str) -> Self {
        self.title = Some(title);
        self
    }

    /// Does something when reached.
    #[must_use]
    pub const fn doing(mut self, effect: Effect) -> Self {
        self.effect = Some(effect);
        self
    }

    /// Continues to `next` when the player presses `enter`.
    #[must_use]
    pub const fn then(mut self, next: NodeId) -> Self {
        self.next = Some(next);
        self
    }
}

/// A snapshot of what the player knows, for evaluating conditions.
///
/// The conversation screen is handed one of these so it can walk the graph — and
/// update it as the player's own choices teach them things — without holding a
/// reference to the world. The world is still the only thing that *stores*
/// progress: the screen reports its effects back when it closes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Known {
    discoveries: std::collections::BTreeSet<Discovery>,
    flags: std::collections::BTreeSet<Flag>,
}

impl Known {
    /// Nothing known yet.
    #[must_use]
    pub fn new() -> Self {
        Known::default()
    }

    /// Whether a condition holds.
    #[must_use]
    pub fn holds(&self, condition: Condition) -> bool {
        match condition {
            Condition::Knows(discovery) => self.discoveries.contains(&discovery),
            Condition::NotKnows(discovery) => !self.discoveries.contains(&discovery),
            Condition::Flag(flag) => self.flags.contains(&flag),
            Condition::NotFlag(flag) => !self.flags.contains(&flag),
        }
    }

    /// Applies an effect to this snapshot, exactly as the world will.
    pub fn apply(&mut self, effect: Effect) {
        match effect {
            Effect::Learn(discovery) => {
                self.discoveries.insert(discovery);
            }
            Effect::Set(flag) => {
                self.flags.insert(flag);
            }
        }
    }

    /// Records knowledge held without an effect, for building a snapshot.
    pub fn remembered(&mut self, discovery: Discovery) {
        self.discoveries.insert(discovery);
    }

    /// Records a story fact held without an effect, for building a snapshot.
    pub fn flagged(&mut self, flag: Flag) {
        self.flags.insert(flag);
    }

    /// The discoveries in this snapshot.
    #[must_use]
    pub fn discoveries(&self) -> &std::collections::BTreeSet<Discovery> {
        &self.discoveries
    }

    /// The flags in this snapshot.
    #[must_use]
    pub fn flags(&self) -> &std::collections::BTreeSet<Flag> {
        &self.flags
    }
}

/// A branching conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conversation {
    /// Where the conversation goes after the entry node.
    pub nodes: &'static [Node],
    /// Entry nodes, checked in order: the first whose condition holds wins.
    ///
    /// This is the generalisation of the old discovery-reactive speech: a
    /// character who has something new to say says it first.
    pub entries: &'static [(Condition, NodeId)],
    /// Where to start when nothing else matched.
    pub fallback: NodeId,
}

impl Conversation {
    /// The node with this id, if the conversation has one.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&'static Node> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// The node this conversation starts at for a player who knows `holds`.
    #[must_use]
    pub fn entry(&self, holds: impl Fn(Condition) -> bool) -> Option<&'static Node> {
        let chosen = self
            .entries
            .iter()
            .find(|(condition, _)| holds(*condition))
            .map_or(self.fallback, |(_, id)| *id);
        self.node(chosen)
    }

    /// The first node, ignoring conditions.
    #[must_use]
    pub fn first(&self) -> Option<&'static Node> {
        self.node(self.fallback)
    }

    /// Every id this conversation can reach, for the integrity test.
    #[must_use]
    pub fn reachable(&self) -> Vec<NodeId> {
        let mut seen = Vec::new();
        let mut queue: Vec<NodeId> = self
            .entries
            .iter()
            .map(|(_, id)| *id)
            .chain(std::iter::once(self.fallback))
            .collect();
        while let Some(id) = queue.pop() {
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            let Some(node) = self.node(id) else {
                continue;
            };
            queue.extend(node.choices.iter().filter_map(|choice| choice.next));
            queue.extend(node.next);
        }
        seen.sort_unstable();
        seen
    }
}

#[cfg(test)]
mod tests {
    use super::{Choice, Condition, Conversation, Effect, Flag, Mood, Node, NodeId};
    use crate::game::discovery::Discovery;

    const A: NodeId = NodeId::new("a");
    const B: NodeId = NodeId::new("b");

    static CHOICES: [Choice; 2] = [Choice::to("Say something.", B), Choice::end("Say nothing.")];

    static NODES: [Node; 2] = [
        Node::saying(A, "Someone", &["First."]).then(B),
        Node::saying(B, "Someone", &["Second."]),
    ];

    static TALK: Conversation = Conversation {
        nodes: &NODES,
        entries: &[(Condition::Knows(Discovery::VillageWell), B)],
        fallback: A,
    };

    #[test]
    fn an_entry_condition_selects_where_the_conversation_starts() {
        assert_eq!(Some(A), TALK.entry(|_| false).map(|node| node.id));
        assert_eq!(
            Some(B),
            TALK.entry(|condition| condition == Condition::Knows(Discovery::VillageWell))
                .map(|node| node.id)
        );
    }

    #[test]
    fn nodes_are_found_by_id_and_missing_ones_are_none() {
        assert_eq!(Some(A), TALK.node(A).map(|node| node.id));
        assert_eq!(None, TALK.node(NodeId::new("nope")));
    }

    #[test]
    fn reachability_walks_every_edge() {
        assert_eq!(vec![A, B], TALK.reachable());
    }

    #[test]
    fn choices_carry_conditions_and_effects() {
        let choice = Choice::to("Ask about the well.", B)
            .when(Condition::Knows(Discovery::VillageWell))
            .doing(Effect::Set(Flag::CassenAccused));
        assert_eq!(Some(Condition::Knows(Discovery::VillageWell)), choice.shows);
        assert_eq!(Some(Effect::Set(Flag::CassenAccused)), choice.effect);
        assert_eq!(Some(B), choice.next);
    }

    #[test]
    fn a_node_can_be_titled_and_mooded() {
        let node = Node::asking(A, "Someone", &["Well?"], &CHOICES)
            .titled("the lantern keeper")
            .in_mood(Mood::Wry);
        assert_eq!(Some("the lantern keeper"), node.title);
        assert_eq!(Mood::Wry, node.mood);
        assert_eq!(2, node.choices.len());
    }

    #[test]
    fn every_conversation_in_the_game_is_a_sound_graph() {
        // The test lives here rather than beside the content so that it holds for
        // every conversation anyone ever adds: no dangling node, no unreachable
        // node, no empty speech.
        for character in crate::game::areas::roster() {
            let talk = character.talk;
            for (condition, id) in talk.entries {
                assert!(
                    talk.node(*id).is_some(),
                    "{}: entry {condition:?} points at a missing node {}",
                    character.name,
                    id.as_str()
                );
            }
            assert!(
                talk.node(talk.fallback).is_some(),
                "{}: the fallback node is missing",
                character.name
            );
            for id in talk.reachable() {
                let node = talk.node(id).unwrap_or_else(|| {
                    panic!("{}: unreachable id {}", character.name, id.as_str())
                });
                assert!(
                    !node.lines.is_empty(),
                    "{}: node {} says nothing",
                    character.name,
                    id.as_str()
                );
                assert!(
                    node.lines.len() <= 4,
                    "{}: node {} has too many lines to read at once",
                    character.name,
                    id.as_str()
                );
                for line in node.lines {
                    assert!(
                        line.chars().count() <= 96,
                        "{}: a line in {} is too long to wrap: {line:?}",
                        character.name,
                        id.as_str()
                    );
                }
                for choice in node.choices {
                    assert!(
                        !choice.text.is_empty(),
                        "{}: an unlabelled choice in {}",
                        character.name,
                        id.as_str()
                    );
                    if let Some(next) = choice.next {
                        assert!(
                            talk.node(next).is_some(),
                            "{}: a choice in {} points at a missing node {}",
                            character.name,
                            id.as_str(),
                            next.as_str()
                        );
                    }
                }
                if let Some(next) = node.next {
                    assert!(
                        talk.node(next).is_some(),
                        "{}: {} continues to a missing node {}",
                        character.name,
                        id.as_str(),
                        next.as_str()
                    );
                }
            }
            // Every node must be reachable, or it is dialogue nobody can read.
            for node in talk.nodes {
                assert!(
                    talk.reachable().contains(&node.id),
                    "{}: node {} cannot be reached",
                    character.name,
                    node.id.as_str()
                );
            }
        }
    }
}
