//! The help and diagnostics overlay.
//!
//! A reusable secondary screen: any screen can push it with its own list of key
//! bindings. It draws an opaque panel over whatever is beneath and leaves the
//! surrounding frame visible, demonstrating layered composition. It also reports
//! live renderer diagnostics, which keeps them reachable from the game without
//! competing with it.

use crate::input::{Key, KeyCode};
use crate::render::{CanvasView, Frame, Rect, Style};

use super::FrameContext;
use super::screen::{Command, Screen};
use super::theme::Palette;

/// A key binding shown in the overlay: the keys, and what they do.
pub type Binding = (&'static str, &'static str);

/// The column at which descriptions are aligned, relative to the panel interior.
const DESCRIPTION_COLUMN: u16 = 15;

/// A modal overlay listing a screen's controls plus live renderer diagnostics.
#[derive(Debug, Clone, Copy)]
pub struct HelpOverlay {
    controls: &'static [Binding],
}

impl HelpOverlay {
    /// Creates the overlay with the bindings to display.
    #[must_use]
    pub const fn new(controls: &'static [Binding]) -> Self {
        HelpOverlay { controls }
    }
}

impl Screen for HelpOverlay {
    fn name(&self) -> &'static str {
        "help"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only a genuine press dismisses the overlay. `h` is followed immediately
        // by its own release, and treating that as an activation would close the
        // overlay in the same instant it opened; auto-repeat must not either.
        if !key.is_down() || key.is_repeat() {
            return Command::None;
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Function(1) => {
                Command::Pop
            }
            _ => Command::None,
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        let area = frame.bounds();
        let panel_width = area.width.min(74).saturating_sub(2).max(20).min(area.width);
        let panel_height = area
            .height
            .min(24)
            .saturating_sub(2)
            .max(6)
            .min(area.height);
        let panel = Rect::new(
            area.centered_x(panel_width),
            area.centered_y(panel_height),
            panel_width,
            panel_height,
        );
        if panel.is_empty() {
            return;
        }

        let mut view = frame.view(panel);
        view.fill_style(
            view.bounds(),
            Style::new()
                .with_fg(palette.text)
                .with_bg(palette.background),
        );
        view.draw_titled_border(
            view.bounds(),
            "help & diagnostics",
            Style::new().with_fg(palette.accent_dim),
            Style::new().with_fg(palette.accent_alt).bold(),
        );

        let inner = Rect::new(
            1,
            1,
            panel_width.saturating_sub(2),
            panel_height.saturating_sub(2),
        );
        let mut row = inner.y;

        row = heading(&mut view, inner, row, "controls", palette);
        for (keys, description) in self.controls {
            if row >= inner.bottom() {
                break;
            }
            view.put_str(
                inner.x + 1,
                row,
                keys,
                Style::new().with_fg(palette.text).bold(),
            );
            view.put_str(
                inner.x.saturating_add(DESCRIPTION_COLUMN),
                row,
                description,
                palette.muted_style(),
            );
            row = row.saturating_add(1);
        }

        row = row.saturating_add(1);
        row = heading(&mut view, inner, row, "diagnostics", palette);

        let stats = context.stats;
        let diagnostics = [
            ("size", format!("{} x {} cells", stats.width, stats.height)),
            (
                "painted",
                format!("{} / {} cells", stats.cells_written, stats.total_cells()),
            ),
            ("runs", format!("{} contiguous runs", stats.runs)),
            (
                "frame time",
                format!("{:.1} ms", context.frame_time.as_secs_f64() * 1000.0),
            ),
            (
                "frames",
                format!("{:.0} /s (aiming for 60)", stats.frames_per_second),
            ),
            ("colour", context.capabilities.colors.label().to_string()),
            (
                "graphics",
                format!("{} (graphics)", context.graphics.label()),
            ),
            ("renderer", "cell grid, diffed present".to_string()),
        ];
        for (label, value) in &diagnostics {
            if row >= inner.bottom() {
                break;
            }
            view.put_str(inner.x + 1, row, label, palette.muted_style());
            view.put_str(
                inner.x.saturating_add(DESCRIPTION_COLUMN),
                row,
                value,
                Style::new().with_fg(palette.accent_alt),
            );
            row = row.saturating_add(1);
        }
    }
}

/// Writes a section heading and returns the next row.
fn heading(view: &mut CanvasView, inner: Rect, row: u16, text: &str, palette: &Palette) -> u16 {
    if row >= inner.bottom() {
        return row;
    }
    view.put_str(
        inner.x,
        row,
        text,
        Style::new().with_fg(palette.accent).bold(),
    );
    row.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Binding, HelpOverlay};
    use crate::input::{Key, KeyCode};
    use crate::render::{Canvas, Frame};
    use crate::terminal::{Capabilities, PresentStats};
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    const TEST_CONTROLS: [Binding; 2] = [("w a s d", "move"), ("e", "interact")];

    fn overlay() -> HelpOverlay {
        HelpOverlay::new(&TEST_CONTROLS)
    }

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            graphics: crate::terminal::GraphicsSupport::Forced,
            art: crate::ui::art::Art::none(),
            key_releases: true,
            palette: Palette::BASIC,
            stats: PresentStats {
                width: 80,
                height: 24,
                cells_written: 120,
                ..PresentStats::default()
            },
            frame_time: Duration::from_millis(4),
            animating: true,
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
    fn escape_and_question_mark_close_the_overlay() {
        let mut overlay = overlay();
        assert!(matches!(
            overlay.handle_key(Key::new(KeyCode::Esc)),
            Command::Pop
        ));
        assert!(matches!(overlay.handle_key(Key::char('?')), Command::Pop));
        assert!(matches!(overlay.handle_key(Key::char('x')), Command::None));
    }

    #[test]
    fn auto_repeat_does_not_close_the_overlay() {
        let mut overlay = overlay();
        assert!(matches!(
            overlay.handle_key(Key::new(KeyCode::Esc).with_repeat()),
            Command::None
        ));
        assert!(matches!(
            overlay.handle_key(Key::char('?').with_repeat()),
            Command::None
        ));
    }

    #[test]
    fn compose_reports_the_supplied_controls_and_live_diagnostics() {
        let mut canvas = Canvas::new(80, 24);
        draw(&mut canvas, &overlay());
        let text = frame_text(&canvas);

        assert!(text.contains("help & diagnostics"), "missing title");
        assert!(text.contains("controls"), "missing controls heading");
        assert!(text.contains("interact"), "missing supplied control");
        assert!(text.contains("120 / 1920"), "missing cell diagnostics");
        assert!(text.contains("16-colour"), "missing capability label");
    }

    #[test]
    fn compose_is_safe_on_a_tiny_frame() {
        let mut canvas = Canvas::new(12, 5);
        draw(&mut canvas, &overlay());
    }

    #[test]
    fn the_overlay_leaves_the_area_around_the_panel_untouched() {
        use crate::render::{Cell, Style};
        let mut canvas = Canvas::new(80, 24);
        canvas.fill(canvas.bounds(), Cell::new('~', Style::PLAIN));
        draw(&mut canvas, &overlay());

        // The very corner is outside the panel, so the underlying layer shows.
        assert_eq!(Some('~'), canvas.get(0, 0).map(|cell| cell.ch));
    }
}
