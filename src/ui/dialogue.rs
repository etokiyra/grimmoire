//! The dialogue overlay.
//!
//! A conversation is a focused overlay: a panel across the lower part of the
//! screen with a portrait plate and the speaker's lines. The world stays visible
//! around it, and more importantly input routing does the work - while the panel
//! is open the player cannot move and movement keys cannot re-trigger an
//! interaction.
//!
//! Dismissal deliberately does **not** use the key that opens a conversation
//! (`e`). A terminal that reports auto-repeats as ordinary presses would
//! otherwise close the panel the instant it opened.
//!
//! The portrait is ASCII by default, like everything else in the game. When the
//! development flag enables pixel interface art, an image is drawn in the same
//! rectangle instead; the panel looks right either way.
//!
//! # Opening and closing
//!
//! The panel slides up from the bottom edge and slides back down to leave, which
//! is why it animates itself rather than asking the stack for a full-screen
//! curtain: the world stays visible and still while a conversation comes and
//! goes. Dismissal returns no command until the exit has played, so the panel
//! finishes leaving instead of vanishing.

use std::time::Duration;

use super::FrameContext;
use super::screen::{Command, Screen};
use super::theme::Palette;
use crate::anim::Easing;
use crate::game::Dialogue;
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Frame, Rect, Style, glyph};

/// How wide the portrait plate is, in cells.
const PORTRAIT_WIDTH: u16 = 22;
/// How tall the panel is, in cells.
const PANEL_HEIGHT: u16 = 11;
/// How long the panel takes to rise into place.
const ENTRANCE: Duration = Duration::from_millis(220);
/// How long it takes to slide away again.
const EXIT: Duration = Duration::from_millis(150);

/// The speaker's bust, drawn as ASCII when there is no image.
///
/// Deliberately chibi: a big cowl and two large eyes take most of the plate,
/// which is the same proportions as the figures out in the world.
const PORTRAIT_ASCII: [&str; 7] = [
    "   _____   ",
    "  /     \\  ",
    " |  o o  | ",
    "  \\_____/  ",
    " /#######\\ ",
    "|#########|",
    " \\#######/ ",
];

/// A modal panel showing a short exchange, with a portrait when one exists.
#[derive(Debug, Clone, Copy)]
pub struct DialogueScreen {
    dialogue: Dialogue,
    portrait: Option<AssetId>,
    /// Time since the panel opened.
    open: Duration,
    /// Time since dismissal began, once it has.
    closing: Option<Duration>,
}

impl DialogueScreen {
    /// Creates a panel for `dialogue`, showing `portrait` when available.
    #[must_use]
    pub const fn new(dialogue: Dialogue, portrait: Option<AssetId>) -> Self {
        DialogueScreen {
            dialogue,
            portrait,
            open: Duration::ZERO,
            closing: None,
        }
    }

    /// The dialogue being shown.
    #[must_use]
    pub const fn dialogue(&self) -> Dialogue {
        self.dialogue
    }

    /// Whether the panel is on its way out.
    #[must_use]
    pub const fn is_closing(&self) -> bool {
        self.closing.is_some()
    }

    /// How far the panel has risen, from 0 (just off the bottom) to 1 (settled).
    ///
    /// This is the whole animation: one eased number the geometry and the border
    /// both read from, so the panel can never be half-drawn.
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

impl Screen for DialogueScreen {
    fn name(&self) -> &'static str {
        "dialogue"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only genuine presses act: a held key must not skip past the panel, and
        // a release must not either.
        if !key.is_down() || key.is_repeat() || self.is_closing() {
            return Command::None;
        }
        match key.code {
            // Dismissal starts the exit; the pop comes from `after_update` once it
            // has played out, so the panel leaves rather than vanishing.
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => {
                self.closing = Some(Duration::ZERO);
                Command::None
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.open = self.open.saturating_add(delta);
        if let Some(age) = self.closing {
            self.closing = Some(age.saturating_add(delta));
        }
    }

    fn after_update(&mut self) -> Command {
        match self.closing {
            Some(age) if age >= EXIT => Command::Pop,
            _ => Command::None,
        }
    }

    fn curtain(&self) -> bool {
        // The panel brings itself in; a full-screen curtain on top would fight it.
        false
    }

    fn is_animating(&self) -> bool {
        match self.closing {
            Some(age) => age < EXIT,
            None => self.open < ENTRANCE,
        }
    }

    fn next_frame(&self) -> Option<Duration> {
        if self.is_animating() {
            Some(Duration::ZERO)
        } else {
            None
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        let area = frame.bounds();
        if area.width < 40 || area.height < 8 {
            return;
        }

        let width = area.width.saturating_sub(8).clamp(32, 96);
        let height = PANEL_HEIGHT.min(area.height.saturating_sub(2));
        let reveal = self.reveal();
        // Slide the whole panel up from below the bottom edge.
        let settled = i32::from(area.bottom().saturating_sub(height).saturating_sub(2));
        let hidden = i32::from(area.height);
        let top = (hidden as f32 + (settled - hidden) as f32 * reveal).round() as u16;
        let panel = Rect::new(area.centered_x(width), top, width, height);

        // An opaque panel so the world shows around it but not through it.
        {
            let mut view = frame.view(panel);
            view.fill_style(
                view.bounds(),
                Style::new()
                    .with_fg(palette.text)
                    .with_bg(palette.surface_raised),
            );
            // The frame brightens as the panel settles, so a conversation seems
            // to come into focus rather than simply appear.
            let edge = if reveal > 0.98 {
                Style::new().with_fg(palette.accent).bold()
            } else {
                palette.border_style()
            };
            view.draw_titled_border(
                view.bounds(),
                self.dialogue.speaker,
                palette.border_style(),
                edge,
            );
        }

        let inner = Rect::new(
            panel.x.saturating_add(2),
            panel.y.saturating_add(1),
            panel.width.saturating_sub(4),
            panel.height.saturating_sub(2),
        );
        if inner.is_empty() {
            return;
        }

        // The portrait occupies a reserved rectangle: the image and the text
        // never fight over the same cells.
        let mut text = inner;
        let plate = Rect::new(
            inner.x,
            inner.y,
            PORTRAIT_WIDTH.min(inner.width.saturating_sub(8)),
            inner.height,
        );
        if plate.width >= 6 {
            if let Some(portrait) = self.portrait {
                frame.place_image(plate, portrait);
            } else {
                draw_ascii_portrait(frame, plate, palette);
            }
            text = Rect::new(
                plate.right().saturating_add(2),
                inner.y,
                inner.right().saturating_sub(plate.right() + 2),
                inner.height,
            );
        }

        // A view re-bases its origin, so the coordinates below are local to the
        // text column rather than to the frame.
        let mut view = frame.view(text);
        let column = view.bounds();
        let mut row = column.y;
        for line in self.dialogue.lines {
            for wrapped in glyph::wrap(line, column.width) {
                if row >= column.bottom() {
                    break;
                }
                view.put_str(column.x, row, &wrapped, Style::new().with_fg(palette.text));
                row = row.saturating_add(1);
            }
        }

        // The dismissal hint sits on the panel's last interior row.
        let hint = "[enter] close";
        view.put_str(
            column.right().saturating_sub(glyph::display_width(hint)),
            column.bottom().saturating_sub(1),
            hint,
            palette.muted_style(),
        );
    }
}

/// Draws the speaker's bust in the portrait plate, centred.
fn draw_ascii_portrait(frame: &mut Frame, plate: Rect, palette: &Palette) {
    let width = PORTRAIT_ASCII
        .iter()
        .map(|row| row.chars().count())
        .max()
        .unwrap_or(0) as u16;
    if plate.width < width || plate.height < PORTRAIT_ASCII.len() as u16 {
        return;
    }
    let mut view = frame.view(plate);
    let left = view.bounds().centered_x(width);
    let top = view.bounds().centered_y(PORTRAIT_ASCII.len() as u16);
    for (row, line) in PORTRAIT_ASCII.iter().enumerate() {
        view.put_str(
            left,
            top.saturating_add(row as u16),
            line,
            Style::new().with_fg(palette.character).bold(),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{DialogueScreen, ENTRANCE, EXIT};
    use crate::game::Dialogue;
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{AssetId, Canvas, Frame, ImagePlacement};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn dialogue() -> Dialogue {
        Dialogue::new("Ysolde", &["You walk where the stones remember."])
    }

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Forced,
            art: Art::none(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
            key_releases: true,
        }
    }

    /// A panel that has finished arriving.
    fn settled() -> DialogueScreen {
        let mut screen = DialogueScreen::new(dialogue(), None);
        screen.update(ENTRANCE);
        screen
    }

    fn compose(screen: &DialogueScreen, width: u16, height: u16) -> (Canvas, Vec<ImagePlacement>) {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        (canvas, images)
    }

    fn frame_text(canvas: &Canvas) -> String {
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
    fn confirm_keys_start_the_dismissal_rather_than_ending_it() {
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char(' ')] {
            let mut screen = settled();
            assert!(
                matches!(screen.handle_key(Key::new(code)), Command::None),
                "{code:?} should begin the exit, not skip it"
            );
            assert!(screen.is_closing());
        }
    }

    #[test]
    fn the_panel_pops_itself_once_the_exit_has_played() {
        let mut screen = settled();
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter)),
            Command::None
        ));
        assert!(screen.is_animating(), "the exit needs frames");
        for _ in 0..40 {
            screen.update(Duration::from_millis(10));
        }
        assert!(!screen.is_animating());
        assert!(matches!(screen.after_update(), Command::Pop));
    }

    #[test]
    fn a_second_dismissal_while_closing_does_nothing() {
        let mut screen = settled();
        screen.handle_key(Key::new(KeyCode::Enter));
        screen.update(EXIT / 2);
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter)),
            Command::None
        ));
        assert!(screen.is_closing());
    }

    #[test]
    fn the_panel_rises_into_place() {
        let mut screen = DialogueScreen::new(dialogue(), None);
        assert!(screen.reveal() < 0.01, "it starts off the bottom");
        assert!(screen.is_animating());

        screen.update(ENTRANCE / 2);
        let half = screen.reveal();
        assert!(half > 0.3 && half < 0.99, "halfway is neither end: {half}");

        screen.update(ENTRANCE);
        assert!(screen.reveal() > 0.999, "and it settles exactly in place");
        assert!(!screen.is_animating());
        assert_eq!(None, screen.next_frame(), "a settled panel needs no frames");
    }

    #[test]
    fn a_panel_that_has_not_risen_yet_leaves_the_screen_alone() {
        // At rest the panel sits below the bottom edge, so nothing of it is drawn.
        let screen = DialogueScreen::new(dialogue(), None);
        let (canvas, _) = compose(&screen, 100, 30);
        assert!(
            !frame_text(&canvas).contains("Ysolde"),
            "the panel should still be off-screen"
        );
    }

    #[test]
    fn the_opening_key_does_not_dismiss() {
        let mut screen = settled();
        assert!(matches!(screen.handle_key(Key::char('e')), Command::None));
        assert!(!screen.is_closing());
    }

    #[test]
    fn movement_keys_are_ignored() {
        let mut screen = settled();
        for key in [
            Key::char('w'),
            Key::char('a'),
            Key::char('s'),
            Key::char('d'),
            Key::new(KeyCode::Right),
        ] {
            assert!(
                matches!(screen.handle_key(key), Command::None),
                "{key:?} must not close the panel"
            );
        }
        assert!(!screen.is_closing());
    }

    #[test]
    fn repeats_and_releases_do_not_dismiss() {
        let mut screen = settled();
        for kind in [KeyKind::Repeat, KeyKind::Release] {
            assert!(matches!(
                screen.handle_key(Key::new(KeyCode::Enter).with_kind(kind)),
                Command::None
            ));
        }
        assert!(!screen.is_closing());
    }

    #[test]
    fn the_panel_wants_no_full_screen_curtain() {
        assert!(!settled().curtain(), "the panel animates itself");
    }

    #[test]
    fn compose_shows_the_speaker_the_lines_and_an_ascii_portrait() {
        let (canvas, images) = compose(&settled(), 100, 30);
        let text = frame_text(&canvas);
        assert!(text.contains("Ysolde"), "speaker missing");
        assert!(text.contains("stones remember"), "line missing");
        assert!(text.contains("[enter] close"), "hint missing");
        assert!(
            images.is_empty(),
            "the default portrait is ASCII, not an image"
        );
        assert!(
            text.contains("o o"),
            "the ASCII portrait is missing:\n{text}"
        );
    }

    #[test]
    fn a_portrait_is_placed_beside_the_text() {
        let screen = DialogueScreen {
            portrait: Some(AssetId::new(2)),
            ..settled()
        };
        let (canvas, images) = compose(&screen, 100, 30);
        assert_eq!(1, images.len(), "expected a portrait placement");
        assert!(frame_text(&canvas).contains("stones remember"));
    }

    #[test]
    fn compose_is_safe_on_small_frames() {
        for (width, height) in [(0, 0), (1, 1), (20, 5), (40, 8), (200, 60)] {
            let _ = compose(&settled(), width, height);
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let (canvas, _) = compose(&settled(), 100, 30);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                assert_ne!(
                    Some(crate::render::Color::Default),
                    canvas.get(x, y).map(|cell| cell.style.bg),
                    "cell ({x}, {y}) would show the terminal background"
                );
            }
        }
    }
}
