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

use super::FrameContext;
use super::screen::{Command, Screen};
use crate::game::Dialogue;
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Frame, Rect, Style, glyph};

/// How wide the portrait plate is, in cells.
const PORTRAIT_WIDTH: u16 = 22;
/// How tall the panel is, in cells.
const PANEL_HEIGHT: u16 = 11;

/// A modal panel showing a short exchange, with a portrait when one exists.
#[derive(Debug, Clone, Copy)]
pub struct DialogueScreen {
    dialogue: Dialogue,
    portrait: Option<AssetId>,
}

impl DialogueScreen {
    /// Creates a panel for `dialogue`, showing `portrait` when available.
    #[must_use]
    pub const fn new(dialogue: Dialogue, portrait: Option<AssetId>) -> Self {
        DialogueScreen { dialogue, portrait }
    }

    /// The dialogue being shown.
    #[must_use]
    pub const fn dialogue(&self) -> Dialogue {
        self.dialogue
    }
}

impl Screen for DialogueScreen {
    fn name(&self) -> &'static str {
        "dialogue"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only genuine presses act: a held key must not skip past the panel, and
        // a release must not either.
        if !key.is_down() || key.is_repeat() {
            return Command::None;
        }
        match key.code {
            KeyCode::Enter | KeyCode::Esc | KeyCode::Char(' ') => Command::Pop,
            _ => Command::None,
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
        let panel = Rect::new(
            area.centered_x(width),
            area.bottom().saturating_sub(height).saturating_sub(2),
            width,
            height,
        );

        // An opaque panel so the world shows around it but not through it.
        {
            let mut view = frame.view(panel);
            view.fill_style(
                view.bounds(),
                Style::new()
                    .with_fg(palette.text)
                    .with_bg(palette.surface_raised),
            );
            view.draw_titled_border(
                view.bounds(),
                self.dialogue.speaker,
                palette.border_style(),
                Style::new().with_fg(palette.accent).bold(),
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

        // The portrait occupies a reserved rectangle: no cells are written there,
        // so the image and the text never fight.
        let mut text = inner;
        if let Some(portrait) = self.portrait {
            let plate = Rect::new(
                inner.x,
                inner.y,
                PORTRAIT_WIDTH.min(inner.width.saturating_sub(8)),
                inner.height,
            );
            if plate.width >= 6 {
                frame.place_image(plate, portrait);
                text = Rect::new(
                    plate.right().saturating_add(2),
                    inner.y,
                    inner.right().saturating_sub(plate.right() + 2),
                    inner.height,
                );
            }
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::DialogueScreen;
    use crate::game::Dialogue;
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{AssetId, Canvas, Frame, ImagePlacement};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn dialogue() -> Dialogue {
        Dialogue::new(
            "the Hooded Figure",
            &["You walk where the stones remember."],
        )
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

    fn compose_into(
        screen: &DialogueScreen,
        width: u16,
        height: u16,
    ) -> (Canvas, Vec<ImagePlacement>) {
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
    fn confirm_keys_dismiss_the_panel() {
        for code in [KeyCode::Enter, KeyCode::Esc, KeyCode::Char(' ')] {
            let mut screen = DialogueScreen::new(dialogue(), None);
            assert!(
                matches!(screen.handle_key(Key::new(code)), Command::Pop),
                "{code:?} should close the panel"
            );
        }
    }

    #[test]
    fn the_opening_key_does_not_dismiss() {
        let mut screen = DialogueScreen::new(dialogue(), None);
        assert!(matches!(screen.handle_key(Key::char('e')), Command::None));
    }

    #[test]
    fn movement_keys_are_ignored() {
        let mut screen = DialogueScreen::new(dialogue(), None);
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
    }

    #[test]
    fn repeats_and_releases_do_not_dismiss() {
        let mut screen = DialogueScreen::new(dialogue(), None);
        for kind in [KeyKind::Repeat, KeyKind::Release] {
            assert!(matches!(
                screen.handle_key(Key::new(KeyCode::Enter).with_kind(kind)),
                Command::None
            ));
        }
    }

    #[test]
    fn compose_shows_the_speaker_and_the_lines() {
        let (canvas, images) = compose_into(&DialogueScreen::new(dialogue(), None), 100, 30);
        let text = frame_text(&canvas);
        assert!(text.contains("the Hooded Figure"), "speaker missing");
        assert!(text.contains("stones remember"), "line missing");
        assert!(text.contains("[enter] close"), "hint missing");
        assert!(images.is_empty(), "no portrait was supplied");
    }

    #[test]
    fn a_portrait_is_placed_beside_the_text() {
        let screen = DialogueScreen::new(dialogue(), Some(AssetId::new(2)));
        let (canvas, images) = compose_into(&screen, 100, 30);
        assert_eq!(1, images.len(), "expected a portrait placement");
        // The panel is opaque, so no cell inside it is left showing the world.
        assert!(frame_text(&canvas).contains("stones remember"));
    }

    #[test]
    fn compose_is_safe_on_small_frames() {
        for (width, height) in [(0, 0), (1, 1), (20, 5), (40, 8), (200, 60)] {
            let (_, _) = compose_into(&DialogueScreen::new(dialogue(), None), width, height);
        }
    }
}
