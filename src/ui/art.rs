//! ASCII art assets and small helpers for drawing them.
//!
//! Art is stored as fixed-width rows so it can be centred as a block. Use
//! [`draw_stencil`] to draw art over an existing scene: it leaves space glyphs
//! transparent so the layer underneath shows through.

use crate::render::{CanvasView, Image, Rgb, Style};

/// The full "GRIMMOIRE" banner (generated with figlet's `standard` font).
pub const BANNER_FULL: [&str; 5] = [
    "  ____ ____  ___ __  __ __  __  ___ ___ ____  _____ ",
    " / ___|  _ \\|_ _|  \\/  |  \\/  |/ _ \\_ _|  _ \\| ____|",
    "| |  _| |_) || || |\\/| | |\\/| | | | | || |_) |  _|  ",
    "| |_| |  _ < | || |  | | |  | | |_| | ||  _ <| |___ ",
    " \\____|_| \\_\\___|_|  |_|_|  |_|\\___/___|_| \\_\\_____|",
];

/// Width of [`BANNER_FULL`] in columns.
pub const BANNER_FULL_WIDTH: u16 = 52;

/// The compact "GRIMMOIRE" banner (figlet's `small` font).
pub const BANNER_SMALL: [&str; 4] = [
    "  ___ ___ ___ __  __ __  __  ___ ___ ___ ___ ",
    " / __| _ \\_ _|  \\/  |  \\/  |/ _ \\_ _| _ \\ __|",
    "| (_ |   /| || |\\/| | |\\/| | (_) | ||   / _| ",
    " \\___|_|_\\___|_|  |_|_|  |_|\\___/___|_|_\\___|",
];

/// Width of [`BANNER_SMALL`] in columns.
pub const BANNER_SMALL_WIDTH: u16 = 45;

/// The miniature "GRIMMOIRE" banner (figlet's `mini` font).
pub const BANNER_MINI: [&str; 3] = [
    " __ _ ___         _ ___ _  _ ",
    "/__|_) | |\\/||\\/|/ \\ | |_)|_ ",
    "\\_|| \\_|_|  ||  |\\_/_|_| \\|_ ",
];

/// Width of [`BANNER_MINI`] in columns.
pub const BANNER_MINI_WIDTH: u16 = 29;

/// A wizard's tower silhouette used as a scene backdrop.
pub const TOWER: [&str; 8] = [
    "     /\\     ",
    "    /  \\    ",
    "   /    \\   ",
    "  /______\\  ",
    "  |      |  ",
    "  | [##] |  ",
    "  |      |  ",
    " _|______|_ ",
];

/// Width of [`TOWER`] in columns.
pub const TOWER_WIDTH: u16 = 12;

/// Which banner (if any) fits the current layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Banner {
    /// The full five-row banner.
    Full,
    /// The compact four-row banner.
    Small,
    /// The miniature three-row banner.
    Mini,
    /// No banner; too little room.
    None,
}

impl Banner {
    /// The art rows for this banner.
    #[must_use]
    pub const fn lines(self) -> &'static [&'static str] {
        match self {
            Banner::Full => &BANNER_FULL,
            Banner::Small => &BANNER_SMALL,
            Banner::Mini => &BANNER_MINI,
            Banner::None => &[],
        }
    }

    /// The width of this banner in columns.
    #[must_use]
    pub const fn width(self) -> u16 {
        match self {
            Banner::Full => BANNER_FULL_WIDTH,
            Banner::Small => BANNER_SMALL_WIDTH,
            Banner::Mini => BANNER_MINI_WIDTH,
            Banner::None => 0,
        }
    }

    /// The height of this banner in rows.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.lines().len() as u16
    }
}

/// Chooses the widest banner that fits in `width` columns.
#[must_use]
pub fn banner_for_width(width: u16) -> Banner {
    if width >= BANNER_FULL_WIDTH {
        Banner::Full
    } else if width >= BANNER_SMALL_WIDTH {
        Banner::Small
    } else if width >= BANNER_MINI_WIDTH {
        Banner::Mini
    } else {
        Banner::None
    }
}

/// Draws a banner centred in `view`, revealing it left-to-right up to `reveal`.
///
/// `reveal` runs from `0.0` (nothing) to `1.0` (fully drawn). Glyphs within a
/// small band of the reveal edge are drawn with `edge`, which produces a moving
/// highlight as the title appears.
pub fn draw_banner(
    view: &mut CanvasView,
    banner: Banner,
    row: u16,
    reveal: f32,
    style: Style,
    edge: Style,
) {
    let lines = banner.lines();
    if lines.is_empty() || banner.width() == 0 {
        return;
    }
    let reveal = reveal.clamp(0.0, 1.0);
    let start_x = view.width().saturating_sub(banner.width()) / 2;
    let width = f32::from(banner.width());

    for (row_offset, line) in lines.iter().enumerate() {
        let y = row.saturating_add(row_offset as u16);
        for (column, ch) in line.chars().enumerate() {
            let threshold = (column as f32 + 0.5) / width;
            if threshold > reveal {
                break;
            }
            let near_edge = (threshold - reveal).abs() < 0.08;
            let glyph_style = if near_edge { edge } else { style };
            view.put_char(start_x.saturating_add(column as u16), y, ch, glyph_style);
        }
    }
}

/// Draws art rows as a stencil, leaving space glyphs transparent.
///
/// This is how layered scenes composite: the backdrop shows through the gaps in
/// the silhouette instead of being erased.
pub fn draw_stencil(view: &mut CanvasView, x: u16, y: u16, lines: &[&str], style: Style) {
    for (row_offset, line) in lines.iter().enumerate() {
        let row = y.saturating_add(row_offset as u16);
        let mut column = x;
        for ch in line.chars() {
            if ch != ' ' {
                view.put_char(column, row, ch, style);
            }
            column = column.saturating_add(1);
        }
    }
}

/// A procedural portrait plate: a hooded figure lit from behind.
///
/// This is placeholder art for the graphics proof of concept. It is generated
/// rather than loaded, so the project needs no image decoder and the result is
/// deterministic and testable. If image-backed art becomes central, this is
/// where a real decoder and real art would replace it.
#[must_use]
pub fn portrait(width: u32, height: u32) -> Image {
    let glow = Rgb::new(255, 206, 122);
    let deep = Rgb::new(14, 10, 22);
    let violet = Rgb::new(48, 32, 78);
    let mut image = Image::radial(width, height, violet, deep);

    if width == 0 || height == 0 {
        return image;
    }

    let w = width as f32;
    let h = height as f32;
    let cx = (w - 1.0) / 2.0;

    // A soft halo behind the figure.
    let halo_cy = h * 0.42;
    let halo_r = h * 0.46;
    for y in 0..height {
        for x in 0..width {
            let dy = y as f32 - halo_cy;
            let dx = (x as f32 - cx) * 0.85;
            let distance = (dx * dx + dy * dy).sqrt() / halo_r;
            if distance < 1.0 {
                let falloff = (1.0 - distance).powi(2);
                let base = image.pixel(x, y);
                image.set_pixel(x, y, base.lerp(glow, falloff * 0.6));
            }
        }
    }

    // The figure: a hood over shoulders, drawn as a stencil so the halo reads
    // around it.
    let hood_cy = h * 0.40;
    let hood_rx = w * 0.17;
    let hood_ry = h * 0.21;
    let shoulder_top = h * 0.58;
    for y in 0..height {
        for x in 0..width {
            let dx = (x as f32 - cx) / hood_rx;
            let dy = (y as f32 - hood_cy) / hood_ry;
            let in_hood = dx * dx + dy * dy < 1.0;

            let in_shoulders = if (y as f32) > shoulder_top {
                let spread = (y as f32 - shoulder_top) / (h - shoulder_top).max(1.0);
                (x as f32 - cx).abs() < w * (0.10 + 0.22 * spread)
            } else {
                false
            };

            if in_hood || in_shoulders {
                image.set_pixel(x, y, Rgb::new(9, 7, 14));
            }
        }
    }

    // A warm rim on the leading edge of the hood.
    let rim_radius = w * 0.19;
    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - cx;
            let dy = y as f32 - hood_cy;
            let distance = (dx * dx + dy * dy).sqrt();
            if (distance - rim_radius).abs() < 1.2 && dy < 0.0 {
                image.set_pixel(x, y, Rgb::new(190, 150, 96));
            }
        }
    }

    image
}

#[cfg(test)]
mod tests {
    use super::{
        BANNER_FULL, BANNER_FULL_WIDTH, BANNER_MINI, BANNER_MINI_WIDTH, BANNER_SMALL,
        BANNER_SMALL_WIDTH, Banner, TOWER, TOWER_WIDTH, banner_for_width, draw_stencil, portrait,
    };
    use crate::render::{Canvas, Rect, Style};

    #[test]
    fn banner_rows_are_uniformly_padded() {
        for (lines, width) in [
            (&BANNER_FULL[..], BANNER_FULL_WIDTH),
            (&BANNER_SMALL[..], BANNER_SMALL_WIDTH),
            (&BANNER_MINI[..], BANNER_MINI_WIDTH),
        ] {
            for line in lines {
                assert_eq!(usize::from(width), line.chars().count(), "line {line:?}");
            }
        }
    }

    #[test]
    fn tower_rows_are_uniformly_padded() {
        for line in TOWER {
            assert_eq!(
                usize::from(TOWER_WIDTH),
                line.chars().count(),
                "line {line:?}"
            );
        }
    }

    #[test]
    fn banner_selection_picks_the_widest_that_fits() {
        assert_eq!(Banner::Full, banner_for_width(80));
        assert_eq!(Banner::Small, banner_for_width(50));
        assert_eq!(Banner::Mini, banner_for_width(30));
        assert_eq!(Banner::None, banner_for_width(10));
    }

    #[test]
    fn banner_heights_match_their_rows() {
        assert_eq!(5, Banner::Full.height());
        assert_eq!(4, Banner::Small.height());
        assert_eq!(3, Banner::Mini.height());
        assert_eq!(0, Banner::None.height());
    }

    #[test]
    fn stencil_leaves_spaces_transparent() {
        let mut canvas = Canvas::new(6, 2);
        canvas.fill(canvas.bounds(), crate::render::Cell::new('~', Style::PLAIN));
        {
            let mut view = canvas.view(Rect::from_size(6, 2));
            draw_stencil(&mut view, 0, 0, &["a b"], Style::new());
        }
        assert_eq!(Some('a'), canvas.get(0, 0).map(|cell| cell.ch));
        assert_eq!(Some('~'), canvas.get(1, 0).map(|cell| cell.ch));
        assert_eq!(Some('b'), canvas.get(2, 0).map(|cell| cell.ch));
    }

    #[test]
    fn the_portrait_is_a_lit_silhouette() {
        let image = portrait(64, 64);
        assert_eq!(64, image.width());
        assert_eq!(64, image.height());

        // The halo beside the figure is far brighter than a corner.
        let halo = image.pixel(12, 26);
        let corner = image.pixel(0, 0);
        let brightness = |c: crate::render::Rgb| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
        assert!(
            brightness(halo) > brightness(corner) + 60,
            "expected a glow around the figure"
        );

        // The figure itself is near-black.
        let figure = image.pixel(32, 30);
        assert!(brightness(figure) < 80, "the figure should be a silhouette");
    }

    #[test]
    fn the_portrait_is_deterministic_and_size_safe() {
        assert_eq!(portrait(32, 32), portrait(32, 32));
        let empty = portrait(0, 0);
        assert_eq!(0, empty.width());
    }
}
