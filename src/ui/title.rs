//! The initial Grimmoire title screen.
//!
//! This is the project's first real frame composition: a banner, a tagline, a
//! small menu, and a footer laid out onto a [`Canvas`]. It doubles as the
//! renderer demonstration required by the foundation phase and as a test that
//! keyboard input reaches the UI.

use super::theme;
use crate::render::{Canvas, Color, Rect, Style};

/// The banner glyphs for "GRIMMOIRE", generated with figlet's `standard` font.
///
/// Every line is padded to [`BANNER_WIDTH`] so the block stays aligned when it
/// is centred as a whole.
const BANNER: [&str; 5] = [
    "  ____ ____  ___ __  __ __  __  ___ ___ ____  _____ ",
    " / ___|  _ \\|_ _|  \\/  |  \\/  |/ _ \\_ _|  _ \\| ____|",
    "| |  _| |_) || || |\\/| | |\\/| | | | | || |_) |  _|  ",
    "| |_| |  _ < | || |  | | |  | | |_| | ||  _ <| |___ ",
    " \\____|_| \\_\\___|_|  |_|_|  |_|\\___/___|_| \\_\\_____|",
];

/// Width of the banner block in cells (all [`BANNER`] lines are padded to this).
const BANNER_WIDTH: u16 = 52;

/// The tagline shown beneath the banner.
const TAGLINE: &str = "A terminal-native, ASCII-first dark-fantasy wizard RPG";

/// The footer hint summarising the available controls.
const FOOTER: &str = "Up/Down select   Enter choose   Q/Esc quit";

/// Shown when a menu entry that has no behaviour yet is activated.
const NOT_WRITTEN: &str = "This chapter is still being written.";

/// Menu entries, in selection order.
const MENU: [&str; 2] = ["Begin the journey", "Quit"];

/// Index of the entry that leaves the application.
const QUIT_ENTRY: usize = 1;

/// Marker drawn before the highlighted entry.
const SELECTED_MARKER: &str = "> ";
/// Marker drawn before the other entries.
const UNSELECTED_MARKER: &str = "  ";

/// Minimum terminal width before the title screen degrades gracefully.
const MIN_WIDTH: u16 = 58;
/// Minimum terminal height before the title screen degrades gracefully.
const MIN_HEIGHT: u16 = 16;

/// Mutable state of the title screen.
///
/// This is *interface* state, distinct from [`crate::game::GameState`]: it
/// tracks the menu highlight and a transient status message, nothing about the
/// world or the rules.
#[derive(Debug, Default)]
pub struct TitleScreen {
    selected: usize,
    status: Option<&'static str>,
}

impl TitleScreen {
    /// Creates a title screen with the first entry highlighted.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The available menu entries.
    #[must_use]
    pub fn entries(&self) -> &'static [&'static str] {
        &MENU
    }

    /// The index of the currently highlighted entry.
    #[must_use]
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// The status message currently being shown, if any.
    #[must_use]
    pub const fn status(&self) -> Option<&'static str> {
        self.status
    }

    /// Moves the highlight up one entry, wrapping to the last entry.
    pub fn select_previous(&mut self) {
        self.selected = if self.selected == 0 {
            MENU.len() - 1
        } else {
            self.selected - 1
        };
        self.status = None;
    }

    /// Moves the highlight down one entry, wrapping to the first entry.
    pub fn select_next(&mut self) {
        self.selected = (self.selected + 1) % MENU.len();
        self.status = None;
    }

    /// Activates the highlighted entry.
    ///
    /// Returns `true` when the application should quit; otherwise it records a
    /// status message explaining that the selection has no behaviour yet.
    pub fn activate(&mut self) -> bool {
        if self.selected == QUIT_ENTRY {
            true
        } else {
            self.status = Some(NOT_WRITTEN);
            false
        }
    }
}

/// Composes the title screen into `canvas`.
///
/// The whole frame is drawn from scratch, so `canvas` may be reused between
/// frames without being cleared first.
pub fn compose(canvas: &mut Canvas, screen: &TitleScreen) {
    canvas.clear();
    let area = canvas.bounds();

    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        compose_too_small(canvas, area);
        return;
    }

    canvas.draw_border(area, Style::new().with_fg(theme::SHADOW));

    let banner_row = area.y.saturating_add(2);
    let banner_x = area.centered_x(BANNER_WIDTH);
    for (offset, line) in BANNER.iter().enumerate() {
        let row = banner_row.saturating_add(offset as u16);
        canvas.put_str(
            banner_x,
            row,
            line,
            Style::new().with_fg(theme::ARCANE).bold(),
        );
    }

    let tagline_row = banner_row
        .saturating_add(BANNER.len() as u16)
        .saturating_add(1);
    canvas.put_str_centered(
        area,
        tagline_row,
        TAGLINE,
        Style::new().with_fg(theme::FROST).italic(),
    );

    compose_menu(canvas, area, screen);

    if let Some(status) = screen.status() {
        let status_row = menu_top(area)
            .saturating_add(MENU.len() as u16)
            .saturating_add(1);
        canvas.put_str_centered(area, status_row, status, Style::new().with_fg(theme::EMBER));
    }

    canvas.put_str_centered(
        area,
        area.y.saturating_add(area.height).saturating_sub(2),
        FOOTER,
        Style::new().with_fg(theme::SHADOW),
    );
}

/// Draws the selectable menu entries.
fn compose_menu(canvas: &mut Canvas, area: Rect, screen: &TitleScreen) {
    let entry_width = MENU
        .iter()
        .map(|entry| entry.chars().count())
        .max()
        .unwrap_or(0) as u16;
    let menu_x = area.centered_x(entry_width.saturating_add(UNSELECTED_MARKER.len() as u16));
    let top = menu_top(area);

    for (index, entry) in MENU.iter().enumerate() {
        let row = top.saturating_add(index as u16);
        let selected = index == screen.selected();
        let (marker, style) = if selected {
            (
                SELECTED_MARKER,
                Style::new()
                    .with_fg(Color::Black)
                    .with_bg(theme::ARCANE)
                    .bold(),
            )
        } else {
            (UNSELECTED_MARKER, Style::new().with_fg(theme::PARCHMENT))
        };

        canvas.put_str(menu_x, row, marker, style);
        canvas.put_str(
            menu_x.saturating_add(marker.chars().count() as u16),
            row,
            entry,
            style,
        );
    }
}

/// The row on which the menu block starts.
fn menu_top(area: Rect) -> u16 {
    area.y
        .saturating_add((u32::from(area.height) * 3 / 5) as u16)
}

/// Fallback frame for terminals too small to hold the title screen.
fn compose_too_small(canvas: &mut Canvas, area: Rect) {
    let row = area.centered_y(1);
    canvas.put_str_centered(
        area,
        row,
        "Terminal too small for the title screen.",
        Style::new().with_fg(theme::EMBER),
    );
}

#[cfg(test)]
mod tests {
    use super::{BANNER, BANNER_WIDTH, MENU, TitleScreen, compose};
    use crate::render::Canvas;

    fn glyph(canvas: &Canvas, x: u16, y: u16) -> Option<char> {
        canvas.get(x, y).map(|cell| cell.ch)
    }

    #[test]
    fn banner_lines_are_uniformly_padded() {
        for line in BANNER {
            assert_eq!(
                usize::from(BANNER_WIDTH),
                line.chars().count(),
                "banner line {line:?} is not padded to the block width"
            );
        }
    }

    #[test]
    fn selection_wraps_in_both_directions() {
        let mut screen = TitleScreen::new();
        assert_eq!(0, screen.selected());

        screen.select_previous();
        assert_eq!(MENU.len() - 1, screen.selected());

        screen.select_next();
        assert_eq!(0, screen.selected());
    }

    #[test]
    fn activating_quit_requests_exit_without_status() {
        let mut screen = TitleScreen::new();
        screen.select_next();
        assert!(screen.activate());
        assert_eq!(None, screen.status());
    }

    #[test]
    fn activating_unwritten_entry_reports_status() {
        let mut screen = TitleScreen::new();
        assert!(!screen.activate());
        assert!(screen.status().is_some());
    }

    #[test]
    fn any_navigation_clears_a_stale_status() {
        let mut screen = TitleScreen::new();
        screen.activate();
        assert!(screen.status().is_some());
        screen.select_next();
        assert_eq!(None, screen.status());
    }

    #[test]
    fn compose_draws_the_banner_on_the_expected_row() {
        let mut canvas = Canvas::new(80, 24);
        compose(&mut canvas, &TitleScreen::new());
        // Banner is centred: (80 - 52) / 2 = 14; its first ink is a '_' at column 16.
        assert_eq!(Some('_'), glyph(&canvas, 16, 2));
    }

    #[test]
    fn compose_highlights_the_selected_entry() {
        let mut screen = TitleScreen::new();
        let mut canvas = Canvas::new(80, 24);

        compose(&mut canvas, &screen);
        assert_eq!(Some('>'), glyph(&canvas, 30, 14));

        screen.select_next();
        compose(&mut canvas, &screen);
        assert_eq!(Some(' '), glyph(&canvas, 30, 14));
        assert_eq!(Some('>'), glyph(&canvas, 30, 15));
    }

    #[test]
    fn compose_shows_a_status_message_when_present() {
        let mut screen = TitleScreen::new();
        screen.activate();
        let mut canvas = Canvas::new(80, 24);
        compose(&mut canvas, &screen);

        // The status row sits just below the two menu rows (menu top is row 14).
        let row: String = (0..80).filter_map(|x| glyph(&canvas, x, 17)).collect();
        assert!(row.contains("written"), "status row was {row:?}");
    }

    #[test]
    fn compose_degrades_on_tiny_terminals() {
        let mut canvas = Canvas::new(20, 4);
        compose(&mut canvas, &TitleScreen::new());

        let row: String = (0..20).filter_map(|x| glyph(&canvas, x, 1)).collect();
        assert!(row.contains("small"), "fallback row was {row:?}");
    }
}
