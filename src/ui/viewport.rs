//! The minimum viewport policy.
//!
//! Grimmoire is designed for a large terminal, not for an arbitrarily small one.
//! Below the minimum the application suspends normal rendering and shows a single
//! legible screen asking for a larger window, preserving all game state. When
//! enough room returns, the interface resumes automatically.
//!
//! The threshold lives here so it is trivially revisable, and every screen is
//! composed for [`MIN_WIDTH`] by [`MIN_HEIGHT`] and up.

use super::theme::Palette;
use crate::render::{Frame, Rect};

/// The narrowest viewport the interface is designed for.
pub const MIN_WIDTH: u16 = 80;
/// The shortest viewport the interface is designed for.
pub const MIN_HEIGHT: u16 = 24;

/// Whether `area` is large enough to render the interface.
///
/// Dimensions are the only signal used. A terminal is never assumed to be
/// maximised: a genuinely large window and a maximised one are treated the same,
/// because fullscreen detection is not reliable.
#[must_use]
pub fn is_usable(area: Rect) -> bool {
    area.width >= MIN_WIDTH && area.height >= MIN_HEIGHT
}

/// Draws the suspend screen: opaque, centred, and safe at any size.
///
/// The frame has already been painted with the theme background; this paints the
/// activity surface over it and writes a short explanation. Every write is
/// clipped, so even a one-cell viewport is handled without panicking.
pub fn compose(frame: &mut Frame, palette: &Palette) {
    let area = frame.bounds();
    if area.is_empty() {
        return;
    }

    let mut view = frame.view(area);
    // Own the viewport even here: never leave the terminal's own background
    // showing on the screen that explains the problem.
    view.fill_style(view.bounds(), palette.surface_style());

    let needed = format!("{MIN_WIDTH} x {MIN_HEIGHT}");
    let current = format!("{} x {}", area.width, area.height);

    const BLOCK: u16 = 6;
    let top = area.centered_y(BLOCK);
    let bounds = view.bounds();

    view.put_str_centered(bounds, top, "Grimmoire", palette.accent_style().bold());
    view.put_str_centered(
        bounds,
        top.saturating_add(1),
        "This terminal is too small to play in.",
        palette.text_style(),
    );
    view.put_str_centered(
        bounds,
        top.saturating_add(2),
        &format!("needs {needed}   have {current}"),
        palette.muted_style(),
    );
    view.put_str_centered(
        bounds,
        top.saturating_add(4),
        "Enlarge the window; play resumes on its own.",
        palette.effect_style(),
    );
    view.put_str_centered(
        bounds,
        top.saturating_add(5),
        "Your progress is untouched.  q quits.",
        palette.muted_style(),
    );
}

#[cfg(test)]
mod tests {
    use super::{MIN_HEIGHT, MIN_WIDTH, compose, is_usable};
    use crate::render::{Canvas, Frame, Rect};
    use crate::ui::Palette;

    fn compose_into(canvas: &mut Canvas, palette: &Palette) {
        let mut images = Vec::new();
        let mut frame = Frame::begin(canvas, &mut images);
        compose(&mut frame, palette);
    }

    #[test]
    fn the_threshold_is_inclusive() {
        assert!(is_usable(Rect::from_size(MIN_WIDTH, MIN_HEIGHT)));
        assert!(!is_usable(Rect::from_size(MIN_WIDTH - 1, MIN_HEIGHT)));
        assert!(!is_usable(Rect::from_size(MIN_WIDTH, MIN_HEIGHT - 1)));
        assert!(is_usable(Rect::from_size(200, 60)));
    }

    #[test]
    fn the_suspend_screen_names_the_requirement() {
        let mut canvas = Canvas::new(100, 30);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        compose_into(&mut canvas, &Palette::EXTENDED);

        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        assert!(text.contains("too small"), "missing explanation");
        assert!(text.contains("80 x 24"), "missing requirement");
        assert!(text.contains("100 x 30"), "missing current size");
    }

    #[test]
    fn the_suspend_screen_stays_opaque() {
        let mut canvas = Canvas::new(90, 26);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        compose_into(&mut canvas, &Palette::EXTENDED);

        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let bg = canvas.get(x, y).map(|cell| cell.style.bg);
                assert_ne!(
                    Some(crate::render::Color::Default),
                    bg,
                    "cell ({x}, {y}) would show the terminal background"
                );
            }
        }
    }

    #[test]
    fn the_suspend_screen_survives_absurd_sizes() {
        for width in [0, 1, 2, 10, 40] {
            for height in [0, 1, 2, 5, 12] {
                let mut canvas = Canvas::new(width, height);
                compose_into(&mut canvas, &Palette::BASIC);
            }
        }
    }
}
