//! Responsive layout for the main screen.
//!
//! Layout is recomputed from the frame bounds every frame, so resizing is handled
//! implicitly rather than through stored dimensions. Three modes are supported:
//! a two-column [`LayoutMode::Wide`] arrangement, a single-column
//! [`LayoutMode::Stacked`] arrangement, and a [`LayoutMode::Minimal`] fallback for
//! terminals too small to draw the full interface.

use super::art::{self, Banner};
use crate::render::Rect;

/// Width below which the interface falls back to [`LayoutMode::Minimal`].
pub const MIN_WIDTH: u16 = 44;
/// Height below which the interface falls back to [`LayoutMode::Minimal`].
pub const MIN_HEIGHT: u16 = 13;
/// Width at or above which the two-column layout is used.
pub const WIDE_MIN_WIDTH: u16 = 84;
/// Height at or above which the two-column layout is used.
pub const WIDE_MIN_HEIGHT: u16 = 22;

/// How the main screen arranges its regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Side-by-side scene and panel, below a full banner.
    Wide,
    /// Scene above panel, single column.
    Stacked,
    /// A single centred notice; the terminal is too small for the interface.
    Minimal,
}

/// The regions of the main screen for a given terminal size.
///
/// A region may be empty (zero width or height) when there is not enough room;
/// callers must check before drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Which arrangement was chosen.
    pub mode: LayoutMode,
    /// Which banner art fits.
    pub banner: Banner,
    /// Banner and tagline block.
    pub title: Rect,
    /// The animated scene region.
    pub scene: Rect,
    /// The information panel region.
    pub panel: Rect,
    /// The bottom status bar.
    pub status: Rect,
}

impl Layout {
    /// Computes the layout for `area`.
    #[must_use]
    pub fn compute(area: Rect) -> Self {
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return Layout {
                mode: LayoutMode::Minimal,
                banner: Banner::None,
                title: Rect::default(),
                scene: Rect::default(),
                panel: Rect::default(),
                status: bottom_bar(area, 2),
            };
        }

        let inner = area.inset(1);
        let status = bottom_bar(inner, 2);
        let content = Rect::new(
            inner.x,
            inner.y,
            inner.width,
            status.y.saturating_sub(inner.y).saturating_sub(1),
        );

        if area.width >= WIDE_MIN_WIDTH && area.height >= WIDE_MIN_HEIGHT {
            Self::wide(content, status)
        } else {
            Self::stacked(content, status)
        }
    }

    /// Two columns: the scene on the left, the panel on the right.
    fn wide(content: Rect, status: Rect) -> Self {
        let banner = Banner::Full;
        let title = banner_block(content, banner);
        let body = below(content, title, 1);

        let scene_width = body.width.saturating_mul(58) / 100;
        let scene = Rect::new(body.x, body.y, scene_width, body.height);
        let panel_x = body.x.saturating_add(scene_width).saturating_add(1);
        let panel = Rect::new(
            panel_x,
            body.y,
            body.right().saturating_sub(panel_x),
            body.height,
        );

        Layout {
            mode: LayoutMode::Wide,
            banner,
            title,
            scene,
            panel,
            status,
        }
    }

    /// One column: the scene above the panel.
    fn stacked(content: Rect, status: Rect) -> Self {
        // Shrink the banner when there is little vertical room, so the scene is
        // not squeezed out by the title.
        let mut banner = art::banner_for_width(content.width);
        if banner == Banner::Full && content.height < 16 {
            banner = Banner::Small;
        }
        if banner == Banner::Small && content.height < 11 {
            banner = Banner::Mini;
        }
        if banner == Banner::Mini && content.height < 8 {
            banner = Banner::None;
        }

        let title = banner_block(content, banner);
        let body = below(content, title, 1);

        // Below a workable height the panel is dropped entirely rather than
        // becoming an unusable sliver, and the scene takes the whole body.
        if body.height < 6 {
            return Layout {
                mode: LayoutMode::Stacked,
                banner,
                title,
                scene: body,
                panel: Rect::default(),
                status,
            };
        }

        let scene_height = (body.height.saturating_mul(3) / 5).clamp(3, body.height);
        let panel_y = body.y.saturating_add(scene_height).saturating_add(1);
        let panel_height = body.bottom().saturating_sub(panel_y);
        if panel_height < 3 {
            return Layout {
                mode: LayoutMode::Stacked,
                banner,
                title,
                scene: body,
                panel: Rect::default(),
                status,
            };
        }

        Layout {
            mode: LayoutMode::Stacked,
            banner,
            title,
            scene: Rect::new(body.x, body.y, body.width, scene_height),
            panel: Rect::new(body.x, panel_y, body.width, panel_height),
            status,
        }
    }
}

/// A block holding the banner plus one row for the tagline.
fn banner_block(content: Rect, banner: Banner) -> Rect {
    let height = banner.height().saturating_add(1);
    let width = banner.width().min(content.width);
    Rect::new(content.centered_x(width), content.y, width, height)
}

/// The region of `content` below `title`, separated by `gap` blank rows.
fn below(content: Rect, title: Rect, gap: u16) -> Rect {
    let y = title.y.saturating_add(title.height).saturating_add(gap);
    Rect::new(
        content.x,
        y,
        content.width,
        content.bottom().saturating_sub(y),
    )
}

/// The bottom `height` rows of `area`.
fn bottom_bar(area: Rect, height: u16) -> Rect {
    let height = area.height.min(height);
    Rect::new(
        area.x,
        area.bottom().saturating_sub(height),
        area.width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::{Layout, LayoutMode, MIN_HEIGHT, MIN_WIDTH};
    use crate::render::Rect;

    fn within(inner: Rect, outer: Rect) -> bool {
        inner.is_empty()
            || (inner.x >= outer.x
                && inner.y >= outer.y
                && inner.right() <= outer.right()
                && inner.bottom() <= outer.bottom())
    }

    fn assert_contained(layout: &Layout, area: Rect) {
        for region in [layout.title, layout.scene, layout.panel, layout.status] {
            assert!(
                within(region, area),
                "region {region:?} not within {area:?}"
            );
        }
    }

    const SIZES: [(u16, u16); 6] = [(100, 30), (84, 22), (80, 24), (60, 16), (44, 13), (30, 10)];

    #[test]
    fn every_region_stays_inside_the_terminal() {
        for (width, height) in SIZES {
            let area = Rect::from_size(width, height);
            let layout = Layout::compute(area);
            assert_contained(&layout, area);
        }
    }

    #[test]
    fn mode_thresholds_are_respected() {
        assert_eq!(
            LayoutMode::Wide,
            Layout::compute(Rect::from_size(100, 30)).mode
        );
        assert_eq!(
            LayoutMode::Stacked,
            Layout::compute(Rect::from_size(80, 24)).mode
        );
        assert_eq!(
            LayoutMode::Minimal,
            Layout::compute(Rect::from_size(MIN_WIDTH - 1, 30)).mode
        );
        assert_eq!(
            LayoutMode::Minimal,
            Layout::compute(Rect::from_size(80, MIN_HEIGHT - 1)).mode
        );
    }

    #[test]
    fn wide_layout_places_scene_beside_panel_without_overlap() {
        let layout = Layout::compute(Rect::from_size(100, 30));
        assert!(!layout.scene.is_empty());
        assert!(!layout.panel.is_empty());
        assert!(layout.scene.right() <= layout.panel.x);
        assert_eq!(layout.scene.y, layout.panel.y);
    }

    #[test]
    fn stacked_layout_places_scene_above_panel() {
        let layout = Layout::compute(Rect::from_size(80, 24));
        assert!(!layout.panel.is_empty());
        assert!(layout.scene.bottom() <= layout.panel.y);
        assert_eq!(layout.scene.x, layout.panel.x);
    }

    #[test]
    fn status_bar_sits_at_the_bottom() {
        let area = Rect::from_size(80, 24);
        let layout = Layout::compute(area);
        assert_eq!(2, layout.status.height);
        assert_eq!(area.bottom() - 1, layout.status.bottom());
    }

    #[test]
    fn minimal_layout_has_only_a_status_bar() {
        let layout = Layout::compute(Rect::from_size(30, 10));
        assert_eq!(LayoutMode::Minimal, layout.mode);
        assert!(layout.title.is_empty());
        assert!(layout.scene.is_empty());
        assert!(layout.panel.is_empty());
        assert!(!layout.status.is_empty());
    }

    #[test]
    fn cramped_stacked_layout_gives_the_scene_the_whole_body() {
        for (width, height) in [(60u16, 14u16), (60, 18), (50, 15)] {
            let layout = Layout::compute(Rect::from_size(width, height));
            assert_eq!(LayoutMode::Stacked, layout.mode, "{width}x{height}");
            assert!(layout.panel.is_empty(), "{width}x{height} panel not empty");
            assert!(
                layout.scene.height >= 3,
                "{width}x{height} scene too small: {:?}",
                layout.scene
            );
        }
    }
}
