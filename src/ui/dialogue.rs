//! The dialogue panel.
//!
//! A dialogue is pushed on top of the exploration screen rather than drawn by
//! it, so the world stays visible behind the panel and, more importantly, input
//! routing does the work: while the panel is active the player cannot move, and
//! movement keys cannot re-trigger an interaction.
//!
//! Dismissal deliberately does **not** use the key that opens a conversation
//! (`e`). A terminal that reports auto-repeats as ordinary presses would
//! otherwise close the panel the instant it opened.

use crate::game::Dialogue;
use crate::input::{Key, KeyCode};
use crate::render::{Frame, Rect, Style, glyph};

use super::FrameContext;
use super::screen::{Command, Screen};

/// A modal panel showing a short exchange.
#[derive(Debug, Clone, Copy)]
pub struct DialogueScreen {
    dialogue: Dialogue,
}

impl DialogueScreen {
    /// Creates a panel for `dialogue`.
    #[must_use]
    pub const fn new(dialogue: Dialogue) -> Self {
        DialogueScreen { dialogue }
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
        // Repeats are ignored so a held key cannot skip past the panel.
        if key.is_repeat() {
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
        if area.is_empty() {
            return;
        }

        let text_width = area.width.saturating_sub(6).clamp(16, 68);
        let mut lines: Vec<String> = Vec::new();
        for line in self.dialogue.lines {
            lines.extend(glyph::wrap(line, text_width));
        }

        let longest = lines
            .iter()
            .map(|line| glyph::display_width(line))
            .max()
            .unwrap_or(0);
        let speaker_width = glyph::display_width(self.dialogue.speaker).saturating_add(4);
        let panel_width = longest
            .max(speaker_width)
            .saturating_add(4)
            .min(area.width)
            .max(8);
        let panel_height = (lines.len() as u16)
            .saturating_add(4)
            .min(area.height)
            .max(4);

        // Sit near the bottom of the screen, like a dialogue box, but leave the
        // exploration screen's message and status rows visible beneath it.
        let x = area.centered_x(panel_width);
        let y = area
            .bottom()
            .saturating_sub(panel_height)
            .saturating_sub(3)
            .max(area.y);
        let panel = Rect::new(x, y, panel_width, panel_height);

        let mut view = frame.view(panel);
        view.fill_style(
            view.bounds(),
            Style::new()
                .with_fg(palette.text)
                .with_bg(palette.background),
        );
        view.draw_titled_border(
            view.bounds(),
            self.dialogue.speaker,
            Style::new().with_fg(palette.accent_dim),
            Style::new().with_fg(palette.accent_alt).bold(),
        );

        let inner = Rect::new(
            2,
            1,
            panel_width.saturating_sub(4),
            panel_height.saturating_sub(2),
        );
        let mut row = inner.y;
        for line in &lines {
            if row >= inner.bottom() {
                break;
            }
            view.put_str(inner.x, row, line, Style::new().with_fg(palette.text));
            row = row.saturating_add(1);
        }

        if panel_height >= 4 {
            let hint = "[enter] close";
            view.put_str(
                panel_width
                    .saturating_sub(2)
                    .saturating_sub(glyph::display_width(hint)),
                panel_height.saturating_sub(2),
                hint,
                palette.muted_style(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::DialogueScreen;
    use crate::game::Dialogue;
    use crate::input::{Key, KeyCode};
    use crate::render::{Canvas, Frame};
    use crate::terminal::{Capabilities, PresentStats};
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
            image_backend: crate::terminal::ImageBackend::None,
            palette: Palette::BASIC,
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
        }
    }

    /// Composes a screen into a canvas, supplying an empty image list.
    fn draw<S: Screen + ?Sized>(canvas: &mut Canvas, screen: &S) {
        let mut images = Vec::new();
        let mut frame = Frame::begin(canvas, &mut images);
        screen.compose(&mut frame, &context());
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
            let mut screen = DialogueScreen::new(dialogue());
            assert!(
                matches!(screen.handle_key(Key::new(code)), Command::Pop),
                "{code:?} should close the panel"
            );
        }
    }

    #[test]
    fn the_opening_key_does_not_dismiss() {
        let mut screen = DialogueScreen::new(dialogue());
        assert!(matches!(screen.handle_key(Key::char('e')), Command::None));
    }

    #[test]
    fn movement_keys_are_ignored() {
        let mut screen = DialogueScreen::new(dialogue());
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
    fn auto_repeat_does_not_dismiss_the_panel() {
        let mut screen = DialogueScreen::new(dialogue());
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter).with_repeat()),
            Command::None
        ));
    }

    #[test]
    fn compose_shows_the_speaker_and_the_lines() {
        let mut canvas = Canvas::new(80, 24);
        draw(&mut canvas, &DialogueScreen::new(dialogue()));
        let text = frame_text(&canvas);
        assert!(text.contains("the Hooded Figure"), "speaker missing");
        assert!(text.contains("stones remember"), "line missing");
        assert!(text.contains("[enter] close"), "hint missing");
    }

    #[test]
    fn compose_wraps_a_long_line() {
        let mut canvas = Canvas::new(50, 20);
        let screen = DialogueScreen::new(Dialogue::new(
            "a voice",
            &[
                "The stair below has not been swept in a long while, and the water has crept a good deal further than anyone would like to admit.",
            ],
        ));
        draw(&mut canvas, &screen);
        let text = frame_text(&canvas);
        assert!(text.contains("stair below"));
        assert!(text.contains("water has crept"));
    }

    #[test]
    fn compose_is_safe_on_a_tiny_frame() {
        let mut canvas = Canvas::new(8, 4);
        draw(&mut canvas, &DialogueScreen::new(dialogue()));
    }
}
