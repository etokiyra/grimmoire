//! The conversation overlay.
//!
//! A conversation is a focused overlay: a plate across the lower part of the
//! screen carrying the speaker's portrait, their name, what they are saying, and
//! — when they are asking something — the things the player can say back.
//!
//! # Why a pushed screen
//!
//! Input goes to the top screen and nowhere else. That is what stops the player
//! walking while a conversation is open, and it is why this is a screen rather
//! than a flag inside the exploration screen. A screen cannot reach the world,
//! though, so the conversation *walks its own graph* and hands the world a list of
//! [`Effect`]s when it closes (see [`super::screen::Command::Return`]). The world
//! remains the only thing that stores progress.
//!
//! # Reading
//!
//! Text reveals a character at a time, and any key that would advance instead
//! completes the line first — so the effect is never in the way of reading
//! quickly. With motion turned off, lines appear whole.
//!
//! # Choosing
//!
//! Options are listed at the bottom, most specific first, with a selector that
//! slides between them. Only a genuine key press acts: a release is not a
//! decision, and auto-repeat must not choose twice.

use std::time::Duration;

use super::FrameContext;
use super::ornament;
use super::screen::{Command, Message, Screen};
use super::sprite;
use crate::anim::Easing;
use crate::game::{Choice, Conversation, Effect, Figure, Known, Mood, Node, TalkStart};
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Frame, Rect, Style, glyph};

/// How wide the portrait plate is, in cells.
const PORTRAIT_WIDTH: u16 = 15;
/// How tall the panel is, in cells, before it is clamped to the viewport.
const PANEL_HEIGHT: u16 = 13;
/// How long the plate takes to rise into place.
const ENTRANCE: Duration = Duration::from_millis(140);
/// How long it takes to slide away again.
const EXIT: Duration = Duration::from_millis(160);
/// How long one character of a line takes to appear.
const REVEAL_PER_CHAR: Duration = Duration::from_millis(11);

/// A conversation in progress.
#[derive(Debug)]
pub struct DialogueScreen {
    talk: &'static Conversation,
    node: &'static Node,
    /// What the player knows, updated as their own choices teach them things.
    known: Known,
    /// Everything this conversation has done, in order, for the world to apply.
    effects: Vec<Effect>,
    figure: Figure,
    name: &'static str,
    portrait: Option<AssetId>,
    /// Characters of the current node revealed so far.
    revealed: u16,
    /// How long the current node has been revealing.
    reading: Duration,
    /// The highlighted choice.
    selected: usize,
    /// Time since the plate opened.
    open: Duration,
    /// Time since dismissal began, once it has.
    closing: Option<Duration>,
    /// Set when the conversation has nowhere left to go.
    finished: bool,
}

impl DialogueScreen {
    /// Opens the conversation `start` describes.
    ///
    /// A conversation whose entry cannot be resolved — an empty graph — closes
    /// immediately rather than showing an empty plate.
    #[must_use]
    pub fn new(start: TalkStart, portrait: Option<AssetId>) -> Self {
        let known = start.known.clone();
        let Some(node) = start.talk.entry(|condition| known.holds(condition)) else {
            return DialogueScreen {
                talk: start.talk,
                node: start.talk.first().unwrap_or(&FALLBACK_NODE),
                known,
                effects: Vec::new(),
                figure: start.figure,
                name: start.name,
                portrait,
                revealed: 0,
                reading: Duration::ZERO,
                selected: 0,
                open: Duration::ZERO,
                closing: Some(Duration::ZERO),
                finished: true,
            };
        };
        let mut screen = DialogueScreen {
            talk: start.talk,
            node,
            known,
            effects: Vec::new(),
            figure: start.figure,
            name: start.name,
            portrait,
            revealed: 0,
            reading: Duration::ZERO,
            selected: 0,
            open: Duration::ZERO,
            closing: None,
            finished: false,
        };
        screen.enter(node);
        screen
    }

    /// The node being read.
    #[must_use]
    pub const fn node(&self) -> &'static Node {
        self.node
    }

    /// Everything the conversation has done.
    #[must_use]
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    /// Whether the plate is on its way out.
    #[must_use]
    pub const fn is_closing(&self) -> bool {
        self.closing.is_some()
    }

    /// The total characters in the current node's lines.
    fn total_characters(&self) -> u16 {
        self.node
            .lines
            .iter()
            .map(|line| line.chars().count() as u16)
            .sum()
    }

    /// Whether the current node has finished revealing.
    #[must_use]
    pub fn is_revealed(&self) -> bool {
        self.revealed >= self.total_characters()
    }

    /// The choices the player may actually make right now.
    fn available(&self) -> Vec<&'static Choice> {
        self.node
            .choices
            .iter()
            .filter(|choice| {
                choice
                    .shows
                    .is_none_or(|condition| self.known.holds(condition))
            })
            .collect()
    }

    /// Records an effect, both for the world and for this screen's own view of
    /// what the player knows.
    fn remember(&mut self, effect: Effect) {
        self.known.apply(effect);
        if !self.effects.contains(&effect) {
            self.effects.push(effect);
        }
    }

    /// Moves to a node, doing what reaching it does.
    fn enter(&mut self, node: &'static Node) {
        self.node = node;
        self.revealed = 0;
        self.reading = Duration::ZERO;
        self.selected = 0;
        if let Some(effect) = node.effect {
            self.remember(effect);
        }
    }

    /// Advances: completes the line, takes the chosen option, or moves on.
    fn advance(&mut self) {
        if !self.is_revealed() {
            self.revealed = self.total_characters();
            return;
        }
        let available = self.available();
        if !available.is_empty() {
            let choice = available[self.selected.min(available.len() - 1)];
            if let Some(effect) = choice.effect {
                self.remember(effect);
            }
            match choice.next.and_then(|id| self.talk.node(id)) {
                Some(next) => self.enter(next),
                None => self.finish(),
            }
            return;
        }
        match self.node.next.and_then(|id| self.talk.node(id)) {
            Some(next) => self.enter(next),
            None => self.finish(),
        }
    }

    /// Ends the conversation, keeping everything it did.
    fn finish(&mut self) {
        self.finished = true;
        self.closing = Some(Duration::ZERO);
    }

    /// How far the plate has risen, from 0 (off the bottom) to 1 (settled).
    #[must_use]
    pub fn reveal(&self) -> f32 {
        match self.closing {
            Some(age) => {
                let t = age.as_secs_f32() / EXIT.as_secs_f32();
                1.0 - Easing::EaseInQuad.apply(t.clamp(0.0, 1.0))
            }
            None => {
                let t = self.open.as_secs_f32() / ENTRANCE.as_secs_f32();
                Easing::EaseOutCubic.apply(t.clamp(0.0, 1.0))
            }
        }
    }
}

/// A stand-in node, so a conversation that cannot be resolved still has one.
///
/// Never drawn: a screen with this node closes on its first update.
static FALLBACK_NODE: Node = Node {
    id: crate::game::NodeId::new(""),
    speaker: "",
    title: None,
    lines: &[],
    mood: Mood::Calm,
    choices: &[],
    next: None,
    effect: None,
};

impl Screen for DialogueScreen {
    fn name(&self) -> &'static str {
        "conversation"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only genuine presses act. The key that opened the plate is followed by
        // its own release, and a release must not choose anything; auto-repeat
        // must not walk through a conversation either.
        if !key.is_down() || key.is_repeat() || self.is_closing() {
            return Command::None;
        }
        let choices = self.available().len();
        match key.code {
            KeyCode::Esc => {
                self.finish();
                Command::None
            }
            KeyCode::Up | KeyCode::Char('w') if choices > 1 => {
                self.selected = self.selected.saturating_sub(1);
                Command::None
            }
            KeyCode::Down | KeyCode::Char('s') if choices > 1 => {
                if self.selected + 1 < choices {
                    self.selected += 1;
                }
                Command::None
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('e') | KeyCode::Char('E') => {
                self.advance();
                Command::None
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.open = self.open.saturating_add(delta);
        if let Some(age) = self.closing {
            self.closing = Some(age.saturating_add(delta));
            return;
        }
        if !self.is_revealed() {
            self.reading = self.reading.saturating_add(delta);
            let shown = (self.reading.as_secs_f32() / REVEAL_PER_CHAR.as_secs_f32()) as u16;
            self.revealed = shown.min(self.total_characters());
        }
    }

    fn after_update(&mut self) -> Command {
        match self.closing {
            Some(age) if age >= EXIT => {
                Command::Return(Message::Talk(std::mem::take(&mut self.effects)))
            }
            _ => Command::None,
        }
    }

    fn curtain(&self) -> bool {
        // The plate brings itself in; a full-screen curtain on top would fight it.
        false
    }

    fn is_animating(&self) -> bool {
        if self.closing.is_some() {
            return true;
        }
        self.open < ENTRANCE || !self.is_revealed()
    }

    fn next_frame(&self) -> Option<Duration> {
        if self.is_closing() || self.open < ENTRANCE || !self.is_revealed() {
            Some(Duration::ZERO)
        } else {
            None
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        let area = frame.bounds();
        if area.width < 30 || area.height < 7 {
            return;
        }

        let width = area.width.saturating_sub(6).clamp(30, 92);
        let height = PANEL_HEIGHT.min(area.height.saturating_sub(2));
        let reveal = self.reveal();
        // Slide the whole plate up from below the bottom edge.
        let settled = i32::from(area.bottom().saturating_sub(height).saturating_sub(1));
        let hidden = i32::from(area.height);
        let top = (hidden as f32 + (settled - hidden) as f32 * reveal).round() as u16;
        let panel = Rect::new(area.centered_x(width), top, width, height);

        // An opaque plate, so the world shows around it but not through it.
        {
            let mut view = frame.view(panel);
            view.fill_style(
                view.bounds(),
                Style::new()
                    .with_fg(palette.text)
                    .with_bg(palette.surface_raised),
            );
            let settled = reveal > 0.98;
            let border = if settled {
                Style::new().with_fg(palette.accent).bold()
            } else {
                palette.border_style()
            };
            let title_style = if settled {
                Style::new().with_fg(palette.focus).bold()
            } else {
                Style::new().with_fg(palette.accent_dim)
            };
            let plate_title = match self.node.title {
                Some(title) => format!("{}  ·  {title}", self.name),
                None => self.name.to_string(),
            };
            let bounds = view.bounds();
            ornament::plate(&mut view, bounds, Some(&plate_title), border, title_style);
        }

        let inner = Rect::new(
            panel.x.saturating_add(2),
            panel.y.saturating_add(1),
            panel.width.saturating_sub(4),
            panel.height.saturating_sub(2),
        );
        if inner.height < 3 || inner.width < 20 {
            return;
        }

        let mut text = inner;
        if inner.width >= PORTRAIT_WIDTH + 20 {
            let plate = Rect::new(inner.x, inner.y, PORTRAIT_WIDTH, inner.height.min(10));
            self.compose_portrait(frame, plate, context);
            text = Rect::new(
                plate.right().saturating_add(2),
                inner.y,
                inner.right().saturating_sub(plate.right() + 2),
                inner.height,
            );
        }
        self.compose_text(frame, text, context);
    }
}

impl DialogueScreen {
    /// The speaker's face, in the expression they are holding.
    ///
    /// The portrait is the character's **sprite** — the same art the world draws,
    /// shown on its own plate rather than stretched or redrawn. Their expression
    /// comes from the ink it is lit in, so the supplied art is never altered.
    fn compose_portrait(&self, frame: &mut Frame, plate: Rect, context: &FrameContext) {
        if let Some(asset) = self.portrait {
            frame.place_image(plate, asset);
            return;
        }
        let palette = &context.palette;
        let art = sprite::for_figure(self.figure);
        let inks = art.inks.mooded(self.node.mood);
        let revealed = self.reveal();

        // Its own darker inset: a character in a hood is drawn in inks that are
        // almost the colour of the panel, and a face has to be legible against
        // something.
        {
            let mut inset = frame.view(plate);
            let bounds = inset.bounds();
            inset.fill_style(
                bounds,
                Style::new()
                    .with_fg(palette.text)
                    .with_bg(palette.background),
            );
            // A ring behind the figure, so the plate reads as a presentation
            // rather than as a hole in the panel.
            let ring = Style::new().with_fg(palette.accent_dim);
            for x in 1..bounds.width.saturating_sub(1) {
                inset.put_char(x, 1, ornament::DOT, ring);
                inset.put_char(x, bounds.height.saturating_sub(2), ornament::DOT, ring);
            }
        }

        let mut view = frame.view(plate);
        let bounds = view.bounds();
        let width = art.width;
        let height = art.height;
        if bounds.width < width || bounds.height < height.saturating_add(2) {
            return;
        }
        let left = bounds.centered_x(width);
        let top = bounds.y + 1 + (bounds.height.saturating_sub(height + 2) / 2);
        let rows = sprite::Pose::Idle.rows_for(art);
        for (row, line) in rows.iter().enumerate() {
            for (column, ch) in line.chars().enumerate() {
                if ch == ' ' {
                    continue;
                }
                let ink = inks.of(sprite::part_of(ch));
                let mut style = Style::new().with_fg(ink).bold();
                if revealed < 0.5 {
                    style = style.dim();
                }
                view.put_char(
                    left.saturating_add(column as u16),
                    top.saturating_add(row as u16),
                    ch,
                    style,
                );
            }
        }
    }

    /// The speaker's words, and whatever they are asking.
    fn compose_text(&self, frame: &mut Frame, area: Rect, context: &FrameContext) {
        let palette = &context.palette;
        let mut view = frame.view(area);
        let bounds = view.bounds();
        if bounds.height < 3 {
            return;
        }

        let available = self.available();
        let choice_rows = available.len() as u16;
        // Options always sit at the bottom, so the plate does not jump about as
        // the player moves the selector; the lines get whatever is left.
        let hint_row = bounds.bottom().saturating_sub(1);
        let choices_top = hint_row.saturating_sub(choice_rows);
        let lines_area = Rect::new(
            bounds.x,
            bounds.y.saturating_add(2),
            bounds.width,
            choices_top.saturating_sub(bounds.y).saturating_sub(2),
        );

        // The name is already set into the plate's edge, so this row says who
        // they are rather than repeating what they are called.
        if let Some(title) = self.node.title {
            view.put_str(
                bounds.x,
                bounds.y,
                title,
                Style::new().with_fg(palette.accent_dim),
            );
        }
        ornament::rule(
            &mut view,
            bounds.x,
            bounds.y.saturating_add(1),
            bounds.width,
            Style::new().with_fg(palette.separator),
        );

        // The lines, revealed a character at a time.
        let mut budget = self.revealed;
        let mut row = lines_area.y;
        'lines: for line in self.node.lines {
            for wrapped in glyph::wrap(line, lines_area.width) {
                if row >= lines_area.bottom() {
                    break 'lines;
                }
                let length = wrapped.chars().count() as u16;
                if budget >= length {
                    view.put_str(
                        lines_area.x,
                        row,
                        &wrapped,
                        Style::new().with_fg(palette.text),
                    );
                    budget -= length;
                } else {
                    // Partway through this line: draw what has arrived, and leave a
                    // cursor to show that more is coming.
                    let partial: String = wrapped.chars().take(budget as usize).collect();
                    let drawn = view.put_str(
                        lines_area.x,
                        row,
                        &partial,
                        Style::new().with_fg(palette.text),
                    );
                    // A cursor marks where the next character will land, so a
                    // mid-reveal screen never looks like a finished one.
                    view.put_char(
                        lines_area.x.saturating_add(drawn),
                        row,
                        ornament::DOT,
                        Style::new().with_fg(palette.focus),
                    );
                    break 'lines;
                }
                row = row.saturating_add(1);
            }
        }
        // The options.
        if !available.is_empty() && self.is_revealed() {
            for (slot, choice) in available.iter().enumerate() {
                let y = choices_top.saturating_add(slot as u16);
                if y >= hint_row {
                    break;
                }
                let chosen = slot == self.selected;
                if chosen {
                    view.fill_style(
                        Rect::new(bounds.x, y, bounds.width, 1),
                        Style::new().with_bg(palette.surface),
                    );
                }
                let (mark, style) = if chosen {
                    (
                        ornament::SELECTOR,
                        Style::new().with_fg(palette.focus).bold(),
                    )
                } else {
                    (ornament::DOT, palette.secondary_style())
                };
                view.put_char(bounds.x, y, mark, style);
                view.put_str(bounds.x.saturating_add(2), y, choice.text, style);
            }
        }

        // How to proceed, which depends on what is being asked.
        let hint = if !self.is_revealed() {
            "[enter] read  ·  [esc] leave"
        } else if available.is_empty() {
            "[enter] continue  ·  [esc] leave"
        } else {
            "[w/s] choose  ·  [enter] say it  ·  [esc] leave"
        };
        view.put_str(
            bounds.right().saturating_sub(glyph::display_width(hint)),
            hint_row,
            hint,
            palette.muted_style(),
        );
    }
}

/// The band a mood is drawn with, for the four expressions every portrait has.
impl Mood {
    /// The index into a bust's eye rows.
    #[must_use]
    pub const fn band(self) -> usize {
        match self {
            Mood::Calm => 0,
            Mood::Warm | Mood::Wry => 1,
            Mood::Wary | Mood::Veiled => 2,
            Mood::Sorry | Mood::Shaken => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{DialogueScreen, EXIT};
    use crate::game::{
        Choice, Condition, Conversation, Discovery, Effect, Figure, Flag, Known, Mood, Node,
        NodeId, TalkStart,
    };
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Message, Screen};
    use crate::ui::{FrameContext, Palette};

    const GREETING: NodeId = NodeId::new("greeting");
    const CHAPEL: NodeId = NodeId::new("chapel");
    const SECRET: NodeId = NodeId::new("secret");
    const ACCUSE: NodeId = NodeId::new("accuse");

    static CHOICES: [Choice; 3] = [
        Choice::to("Ask about the chapel.", CHAPEL),
        Choice::to("Accuse them.", ACCUSE).when(Condition::Knows(Discovery::VillageWell)),
        Choice::end("Say nothing."),
    ];

    static NODES: [Node; 4] = [
        Node::asking(
            GREETING,
            "Someone",
            &["Hello there, wanderer.", "Do you want to ask?"],
            &CHOICES,
        )
        .titled("of the hollow village"),
        Node::saying(CHAPEL, "Someone", &["It is a chapel."])
            .in_mood(Mood::Warm)
            .then(GREETING),
        Node::saying(ACCUSE, "Someone", &["You have been to the well."])
            .in_mood(Mood::Wry)
            .doing(Effect::Set(Flag::CassenAccused))
            .then(SECRET),
        Node::saying(SECRET, "Someone", &["That is all."])
            .in_mood(Mood::Sorry)
            .doing(Effect::Learn(Discovery::ChapelBasin)),
    ];

    static TALK: Conversation = Conversation {
        nodes: &NODES,
        entries: &[(Condition::Flag(Flag::LettersPromised), CHAPEL)],
        fallback: GREETING,
    };

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Unsupported,
            art: Art::none(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
            key_releases: true,
            motion: crate::ui::settings::Motion::Full,
        }
    }

    fn start(known: Known) -> TalkStart {
        TalkStart {
            talk: &TALK,
            name: "Someone",
            figure: Figure::Scavenger,
            known,
        }
    }

    fn talk(known: Known) -> DialogueScreen {
        DialogueScreen::new(start(known), None)
    }

    fn settle(screen: &mut DialogueScreen) {
        for _ in 0..40 {
            screen.update(Duration::from_millis(60));
        }
    }

    fn press(screen: &mut DialogueScreen, key: Key) -> Command {
        screen.handle_key(key)
    }

    fn compose(screen: &DialogueScreen, width: u16, height: u16) -> String {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images: Vec<ImagePlacement> = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn the_entry_node_is_chosen_by_what_the_player_knows() {
        assert_eq!(GREETING, talk(Known::new()).node().id);
        let mut known = Known::new();
        known.apply(Effect::Set(Flag::LettersPromised));
        assert_eq!(CHAPEL, talk(known).node().id);
    }

    #[test]
    fn a_conditional_choice_is_hidden_until_it_is_earned() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        let text = compose(&screen, 100, 30);
        assert!(text.contains("Ask about the chapel."), "{text}");
        assert!(
            !text.contains("Accuse them."),
            "a locked option must not be advertised:\n{text}"
        );

        let mut known = Known::new();
        known.remembered(Discovery::VillageWell);
        let mut screen = talk(known);
        settle(&mut screen);
        let text = compose(&screen, 100, 30);
        assert!(text.contains("Accuse them."), "{text}");
    }

    #[test]
    fn a_line_reveals_over_time_and_can_be_read_immediately() {
        let mut screen = talk(Known::new());
        assert!(!screen.is_revealed());

        // Late enough for the plate to have risen, early enough that the first
        // line is still arriving: part of it is drawn and the rest is not.
        screen.update(Duration::from_millis(170));
        let partial = compose(&screen, 100, 30);
        assert!(
            partial.contains("Hello"),
            "nothing was revealed:\n{partial}"
        );
        assert!(
            !partial.contains("Hello there, wanderer."),
            "revealed too fast:\n{partial}"
        );

        // Any key that would advance completes the line instead.
        press(&mut screen, Key::new(KeyCode::Enter));
        assert!(screen.is_revealed(), "any advance completes the line");
        settle(&mut screen);
        let full = compose(&screen, 100, 30);
        assert!(full.contains("Hello there, wanderer."), "{full}");
    }

    #[test]
    fn choosing_an_option_does_what_it_says_and_can_be_reported() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        // The first option is the chapel; press enter to take it.
        press(&mut screen, Key::new(KeyCode::Enter));
        assert_eq!(CHAPEL, screen.node().id);
        assert!(screen.effects().is_empty(), "no effect yet");
    }

    #[test]
    fn a_choice_with_an_effect_records_it_once() {
        let mut known = Known::new();
        known.remembered(Discovery::VillageWell);
        let mut screen = talk(known);
        settle(&mut screen);
        // Move to the accusation: options are [chapel, accuse, nothing].
        press(&mut screen, Key::new(KeyCode::Down));
        press(&mut screen, Key::new(KeyCode::Enter));
        assert_eq!(ACCUSE, screen.node().id);
        assert_eq!(&[Effect::Set(Flag::CassenAccused)], screen.effects());
    }

    #[test]
    fn the_end_of_a_conversation_hands_the_world_everything_it_did() {
        let mut known = Known::new();
        known.remembered(Discovery::VillageWell);
        let mut screen = talk(known);
        settle(&mut screen);
        press(&mut screen, Key::new(KeyCode::Down));
        press(&mut screen, Key::new(KeyCode::Enter)); // accuse
        settle(&mut screen);
        press(&mut screen, Key::new(KeyCode::Enter)); // read the reply
        settle(&mut screen);
        press(&mut screen, Key::new(KeyCode::Enter)); // the last node ends it
        // Play the exit out.
        let mut command = Command::None;
        for _ in 0..20 {
            screen.update(Duration::from_millis(40));
            command = screen.after_update();
            if matches!(command, Command::Return(_)) {
                break;
            }
        }
        match command {
            Command::Return(Message::Talk(effects)) => {
                assert_eq!(
                    vec![
                        Effect::Set(Flag::CassenAccused),
                        Effect::Learn(Discovery::ChapelBasin)
                    ],
                    effects
                );
            }
            Command::None => panic!("expected the effects back"),
            _ => panic!("expected the effects back"),
        }
    }

    #[test]
    fn escape_leaves_and_still_reports_what_happened() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        assert!(matches!(
            press(&mut screen, Key::new(KeyCode::Esc)),
            Command::None
        ));
        assert!(screen.is_closing());
        let mut command = Command::None;
        for _ in 0..20 {
            screen.update(Duration::from_millis(40));
            command = screen.after_update();
            if matches!(command, Command::Return(_)) {
                break;
            }
        }
        assert!(matches!(command, Command::Return(Message::Talk(_))));
    }

    #[test]
    fn a_release_never_chooses_and_auto_repeat_never_walks() {
        // The key that opened the plate is followed by its release; if that were
        // an activation the plate would choose for the player.
        let mut screen = talk(Known::new());
        settle(&mut screen);
        let node = screen.node().id;
        for key in [
            Key::new(KeyCode::Enter).released(),
            Key::new(KeyCode::Esc).released(),
            Key::new(KeyCode::Down).released(),
            Key::new(KeyCode::Enter).with_kind(KeyKind::Repeat),
        ] {
            assert!(
                matches!(press(&mut screen, key), Command::None),
                "{key:?} acted"
            );
        }
        assert_eq!(node, screen.node().id);
        assert!(!screen.is_closing());
    }

    #[test]
    fn the_selector_moves_and_stops_at_both_ends() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        assert_eq!(0, screen.selected);
        press(&mut screen, Key::new(KeyCode::Down));
        assert_eq!(1, screen.selected);
        for _ in 0..5 {
            press(&mut screen, Key::new(KeyCode::Down));
        }
        // Two options are listed without the well having been found: the
        // accusation is hidden, so the last index is one.
        assert_eq!(1, screen.selected, "stops at the last option");
        for _ in 0..5 {
            press(&mut screen, Key::new(KeyCode::Up));
        }
        assert_eq!(0, screen.selected, "stops at the first");
    }

    #[test]
    fn an_empty_conversation_closes_without_drawing_anything() {
        static EMPTY: Conversation = Conversation {
            nodes: &[],
            entries: &[],
            fallback: NodeId::new("missing"),
        };
        let mut screen = DialogueScreen::new(
            TalkStart {
                talk: &EMPTY,
                name: "Nobody",
                figure: Figure::Veiled,
                known: Known::new(),
            },
            None,
        );
        assert!(screen.is_closing(), "it must not hang the game");
        screen.update(EXIT);
        assert!(matches!(
            screen.after_update(),
            Command::Return(Message::Talk(_))
        ));
    }

    #[test]
    fn composition_is_safe_at_any_size() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        for width in [0, 1, 20, 40, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40] {
                let _ = compose(&screen, width, height);
            }
        }
    }

    #[test]
    fn the_plate_names_the_speaker_and_lists_the_options() {
        let mut screen = talk(Known::new());
        settle(&mut screen);
        let text = compose(&screen, 100, 34);
        assert!(
            text.contains("Someone"),
            "the speaker is unlabelled:\n{text}"
        );
        assert!(text.contains("hollow village"), "the epithet is missing");
        assert!(
            text.contains("choose"),
            "the way to choose is undiscoverable"
        );
    }

    #[test]
    fn an_unresolvable_node_id_in_the_graph_is_survivable() {
        // A choice pointing at a node that does not exist ends the conversation
        // rather than panicking: content mistakes must not be fatal in play.
        static BROKEN_CHOICES: [Choice; 1] = [Choice::to("Go nowhere.", NodeId::new("missing"))];
        static BROKEN_NODES: [Node; 1] = [Node::asking(
            GREETING,
            "Nobody",
            &["Well."],
            &BROKEN_CHOICES,
        )];
        static BROKEN: Conversation = Conversation {
            nodes: &BROKEN_NODES,
            entries: &[],
            fallback: GREETING,
        };
        let mut screen = DialogueScreen::new(
            TalkStart {
                talk: &BROKEN,
                name: "Nobody",
                figure: Figure::Veiled,
                known: Known::new(),
            },
            None,
        );
        settle(&mut screen);
        press(&mut screen, Key::new(KeyCode::Enter));
        assert!(screen.is_closing(), "a dead end ends the conversation");
    }

    #[test]
    fn the_plate_is_animating_until_it_settles() {
        let mut screen = talk(Known::new());
        assert!(screen.is_animating());
        settle(&mut screen);
        assert!(!screen.is_animating());
        assert_eq!(None, screen.next_frame());
    }
}
