//! Screen transition effect.
//!
//! Transitions live above the screen stack: the application composes the current
//! stack, then the transition may overlay a curtain that hides the incoming
//! screen and sweeps away to reveal it. Keeping this out of the screens means
//! screen transitions do not depend on terminal setup or on any particular screen.

use std::time::Duration;

use crate::anim::{Animation, Easing};
use crate::render::{Canvas, Style};

use super::Palette;

/// How long a screen transition lasts.
pub const DURATION: Duration = Duration::from_millis(260);

/// The animation that drives a transition, from fully covered to fully revealed.
#[must_use]
pub fn animation() -> Animation {
    Animation::new(DURATION).eased(Easing::EaseInOutCubic)
}

/// Draws a curtain hiding the top `1 - progress` of `frame`.
///
/// The curtain is opaque, so the screen beneath appears to be revealed as the
/// bright edge sweeps downwards. At `progress >= 1.0` nothing is drawn.
pub fn compose_curtain(frame: &mut Canvas, progress: f32, palette: &Palette) {
    let height = frame.height();
    if height == 0 {
        return;
    }
    let progress = progress.clamp(0.0, 1.0);
    let covered = ((1.0 - progress) * f32::from(height)).ceil();
    let covered = covered.clamp(0.0, f32::from(height)) as u16;
    if covered == 0 {
        return;
    }

    let body = Style::new()
        .with_fg(palette.arcane_dim)
        .with_bg(palette.night);
    let edge = Style::new()
        .with_fg(palette.frost)
        .with_bg(palette.night)
        .bold();
    let width = frame.width();

    for y in 0..covered.min(height) {
        let is_edge = y + 1 == covered;
        let style = if is_edge { edge } else { body };
        for x in 0..width {
            // A sparse, deterministic dither keeps the curtain from reading as a
            // flat void without being noisy.
            let ch = if is_edge {
                '='
            } else if (x + y) % 7 == 0 {
                '.'
            } else {
                ' '
            };
            frame.put_char(x, y, ch, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{animation, compose_curtain};
    use crate::render::{Canvas, Style};
    use crate::ui::Palette;

    fn filled(width: u16, height: u16) -> Canvas {
        let mut canvas = Canvas::new(width, height);
        canvas.fill(canvas.bounds(), crate::render::Cell::new('X', Style::PLAIN));
        canvas
    }

    fn row_text(canvas: &Canvas, y: u16) -> String {
        (0..canvas.width())
            .filter_map(|x| canvas.get(x, y))
            .map(|cell| cell.ch)
            .collect()
    }

    #[test]
    fn a_finished_curtain_draws_nothing() {
        let mut canvas = filled(10, 4);
        compose_curtain(&mut canvas, 1.0, &Palette::BASIC);
        assert_eq!("XXXXXXXXXX", row_text(&canvas, 0));
    }

    #[test]
    fn a_starting_curtain_covers_the_whole_frame() {
        let mut canvas = filled(10, 4);
        compose_curtain(&mut canvas, 0.0, &Palette::BASIC);
        for y in 0..4 {
            assert!(!row_text(&canvas, y).contains('X'), "row {y} leaked");
        }
    }

    #[test]
    fn the_curtain_edge_sweeps_downwards() {
        let mut canvas = filled(10, 6);
        compose_curtain(&mut canvas, 0.5, &Palette::BASIC);
        // Half the height is covered, so the top rows are curtain and the
        // bottom rows are untouched.
        assert!(!row_text(&canvas, 0).contains('X'));
        assert_eq!("XXXXXXXXXX", row_text(&canvas, 5));
    }

    #[test]
    fn the_animation_reaches_completion() {
        let animation = animation();
        assert!(!animation.is_complete(std::time::Duration::ZERO));
        assert!(animation.is_complete(super::DURATION));
    }
}
