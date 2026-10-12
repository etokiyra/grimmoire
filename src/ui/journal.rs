//! The journal: what the player has learned, and how much is still out there.
//!
//! A discovery is recorded when the player *finds* something — examines a relic,
//! answers the moonwell — never because they walked into an area. The journal is
//! where those findings accumulate, and it is deliberately the only reward the
//! game gives: no experience, no loot, just the setting, told a piece at a time.
//!
//! It is pushed over the world and pops back to it, so reading costs a keystroke
//! and nothing else; the world keeps breathing underneath.

use crate::game::Discovery;
use crate::input::{Key, KeyCode};
use crate::render::{Frame, Rect, Style, glyph};

use super::FrameContext;
use super::screen::{Command, Screen};
use super::theme::Palette;

/// Where the entry list ends and the entry text begins.
const LIST_WIDTH: u16 = 30;
/// The width the panel is allowed to reach.
const MAX_WIDTH: u16 = 84;
/// The height the panel is allowed to reach.
const MAX_HEIGHT: u16 = 24;

/// A panel listing the discoveries made, with the selected one open.
#[derive(Debug, Clone)]
pub struct JournalScreen {
    entries: Vec<Discovery>,
    selected: usize,
    total: usize,
}

impl JournalScreen {
    /// Creates a journal over the discoveries recorded, in the order found.
    #[must_use]
    pub fn new(entries: Vec<Discovery>, total: usize) -> Self {
        JournalScreen {
            entries,
            selected: 0,
            total,
        }
    }

    /// The selected entry, if there is one.
    #[must_use]
    pub fn selected(&self) -> Option<Discovery> {
        self.entries.get(self.selected).copied()
    }

    fn move_selection(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        let last = self.entries.len() - 1;
        let next = (self.selected as i32 + delta).clamp(0, last as i32);
        self.selected = next as usize;
    }
}

impl Screen for JournalScreen {
    fn name(&self) -> &'static str {
        "journal"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only a genuine press acts. The key that opened the panel is followed by
        // its own release, and a release must not close it again the instant it
        // appears; auto-repeat must not either.
        if !key.is_down() || key.is_repeat() {
            return Command::None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('j') | KeyCode::Char('J') | KeyCode::Char('q') => {
                Command::Pop
            }
            KeyCode::Up | KeyCode::Char('w') => {
                self.move_selection(-1);
                Command::None
            }
            KeyCode::Down | KeyCode::Char('s') => {
                self.move_selection(1);
                Command::None
            }
            _ => Command::None,
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        let area = frame.bounds();
        let width = area.width.saturating_sub(4).clamp(24, MAX_WIDTH);
        let height = area.height.saturating_sub(4).clamp(8, MAX_HEIGHT);
        if width < 24 || height < 6 {
            return;
        }
        let panel = Rect::new(
            area.centered_x(width),
            area.centered_y(height),
            width,
            height,
        );

        {
            let mut view = frame.view(panel);
            view.fill_style(
                view.bounds(),
                Style::new()
                    .with_fg(palette.text)
                    .with_bg(palette.background),
            );
            view.draw_titled_border(
                view.bounds(),
                "journal",
                Style::new().with_fg(palette.accent_dim),
                Style::new().with_fg(palette.accent_alt).bold(),
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

        self.compose_list(frame, inner, palette);
        self.compose_entry(frame, inner, palette);
        self.compose_footer(frame, inner, palette);
    }
}

impl JournalScreen {
    /// The list of headings, down the left of the panel.
    fn compose_list(&self, frame: &mut Frame, inner: Rect, palette: &Palette) {
        let list_width = LIST_WIDTH.min(inner.width / 3).max(12);
        let column = Rect::new(inner.x, inner.y, list_width, inner.height.saturating_sub(2));
        let mut view = frame.view(column);
        let bounds = view.bounds();

        if self.entries.is_empty() {
            view.put_str(bounds.x, bounds.y, "nothing yet", palette.muted_style());
            return;
        }

        for (row, discovery) in self.entries.iter().enumerate() {
            let y = bounds.y.saturating_add(row as u16);
            if y >= bounds.bottom() {
                break;
            }
            if row == self.selected {
                view.fill_style(
                    Rect::new(bounds.x, y, bounds.width, 1),
                    palette.raised_style(),
                );
                view.put_char(bounds.x, y, '›', Style::new().with_fg(palette.focus).bold());
                view.put_str(
                    bounds.x.saturating_add(2),
                    y,
                    discovery.title(),
                    Style::new().with_fg(palette.focus).bold(),
                );
            } else {
                view.put_str(
                    bounds.x.saturating_add(2),
                    y,
                    discovery.title(),
                    palette.secondary_style(),
                );
            }
        }
    }

    /// The selected entry's text, on the right.
    fn compose_entry(&self, frame: &mut Frame, inner: Rect, palette: &Palette) {
        let list_width = LIST_WIDTH.min(inner.width / 3).max(12);
        let text = Rect::new(
            inner.x.saturating_add(list_width + 2),
            inner.y,
            inner.width.saturating_sub(list_width + 2),
            inner.height.saturating_sub(2),
        );
        if text.is_empty() {
            return;
        }
        let mut view = frame.view(text);
        let bounds = view.bounds();
        let Some(discovery) = self.selected() else {
            view.put_str(
                bounds.x,
                bounds.y,
                "Find something worth writing down.",
                palette.muted_style(),
            );
            return;
        };

        view.put_str(
            bounds.x,
            bounds.y,
            discovery.title(),
            Style::new().with_fg(palette.accent).bold(),
        );
        let mut row = bounds.y.saturating_add(2);
        for line in glyph::wrap(discovery.journal(), bounds.width) {
            if row >= bounds.bottom() {
                break;
            }
            view.put_str(bounds.x, row, &line, Style::new().with_fg(palette.text));
            row = row.saturating_add(1);
        }
    }

    /// The count, and how to leave.
    fn compose_footer(&self, frame: &mut Frame, inner: Rect, palette: &Palette) {
        let progress = format!(
            "{}/{} discovered       [w/s] read       [esc] close",
            self.entries.len(),
            self.total
        );
        let mut view = frame.view(inner);
        let bounds = view.bounds();
        view.put_str(
            bounds
                .right()
                .saturating_sub(glyph::display_width(&progress)),
            bounds.bottom().saturating_sub(1),
            &progress,
            palette.muted_style(),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::JournalScreen;
    use crate::game::Discovery;
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    static FOUND: [Discovery; 3] = [
        Discovery::ChapelBasin,
        Discovery::VillageWell,
        Discovery::WoodsGlade,
    ];

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

    fn compose(screen: &JournalScreen, width: u16, height: u16) -> String {
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
    fn an_empty_journal_says_so() {
        let screen = JournalScreen::new(Vec::new(), Discovery::ALL.len());
        let text = compose(&screen, 100, 30);
        assert!(text.contains("journal"), "the panel has no title:\n{text}");
        assert!(text.contains("nothing yet"));
        assert!(
            text.contains(&format!("0/{} discovered", Discovery::ALL.len())),
            "the count is wrong:\n{text}"
        );
    }

    #[test]
    fn the_journal_lists_what_was_found_and_opens_the_first_entry() {
        let screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        let text = compose(&screen, 100, 30);
        assert!(
            text.contains("The Cracked Basin"),
            "missing a title:\n{text}"
        );
        assert!(
            text.contains(&format!("3/{} discovered", Discovery::ALL.len())),
            "{text}"
        );
        assert!(
            text.contains("Still water, cold and dark"),
            "the selected entry is not open:\n{text}"
        );
    }

    #[test]
    fn moving_the_selection_opens_a_different_entry() {
        let mut screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        assert_eq!(Some(Discovery::ChapelBasin), screen.selected());
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Down)),
            Command::None
        ));
        assert_eq!(Some(Discovery::VillageWell), screen.selected());
        let text = compose(&screen, 100, 30);
        assert!(text.contains("rope is cut at the top"), "{text}");

        screen.handle_key(Key::new(KeyCode::Up));
        assert_eq!(Some(Discovery::ChapelBasin), screen.selected());
    }

    #[test]
    fn the_selection_stops_at_both_ends() {
        let mut screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        for _ in 0..10 {
            screen.handle_key(Key::new(KeyCode::Down));
        }
        assert_eq!(Some(Discovery::WoodsGlade), screen.selected());
        for _ in 0..10 {
            screen.handle_key(Key::new(KeyCode::Up));
        }
        assert_eq!(Some(Discovery::ChapelBasin), screen.selected());
    }

    #[test]
    fn escape_closes_the_journal() {
        let mut screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Esc)),
            Command::Pop
        ));
    }

    #[test]
    fn neither_a_release_nor_an_auto_repeat_closes_the_journal() {
        // `j` opens the panel and is immediately followed by its own release. If
        // that release were treated as an activation, the journal would open and
        // close in the same instant, which is what it did the first time it was
        // driven on a real terminal.
        for key in [
            Key::char('j').released(),
            Key::new(KeyCode::Esc).released(),
            Key::new(KeyCode::Esc).with_kind(KeyKind::Repeat),
            Key::char('j').with_kind(KeyKind::Repeat),
        ] {
            let mut screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
            assert!(
                matches!(screen.handle_key(key), Command::None),
                "{key:?} dismissed the journal"
            );
            assert_eq!(Some(Discovery::ChapelBasin), screen.selected());
        }
    }

    #[test]
    fn composing_is_safe_at_any_size() {
        let screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        for width in [0, 1, 20, 40, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40] {
                let _ = compose(&screen, width, height);
            }
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let screen = JournalScreen::new(FOUND.to_vec(), Discovery::ALL.len());
        let mut canvas = Canvas::new(100, 30);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
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
