//! The presentation study: a rendering prototype for evaluating the visual
//! direction.
//!
//! This is not the game. It is a dedicated screen for judging how far the cell
//! renderer can be pushed, and for exercising the graphics backend end to end.
//! It demonstrates, in one place:
//!
//! * a fully opaque, application-owned background;
//! * the theme's roles — background, surface, raised surface, border, text,
//!   muted text, accents, and the effect colour;
//! * precisely composed regions: a scene, a portrait plate, a detail column, a
//!   header, and a status bar;
//! * layered content that does not overwrite itself;
//! * a richer ASCII environment with deliberate hierarchy;
//! * a reveal transition and a restrained ambient effect;
//! * an image-backed portrait when the terminal supports one, with an ASCII
//!   fallback that is drawn whenever it does not.

use std::sync::Arc;
use std::time::Duration;

use super::FrameContext;
use super::art;
use super::help::{Binding, HelpOverlay};
use super::screen::{Command, Screen};
use super::showcase::Showcase;
use super::theme::Palette;
use crate::anim::{Animation, Animator, Easing};
use crate::input::{Key, KeyCode};
use crate::render::{CanvasView, Frame, Image, Rect, Style, glyph};

/// How long the opening reveal takes.
const REVEAL: Duration = Duration::from_millis(520);
/// The base resolution of the generated portrait, before scaling to the panel.
const PORTRAIT_WIDTH: u32 = 240;
/// See [`PORTRAIT_WIDTH`].
const PORTRAIT_HEIGHT: u32 = 144;

/// Bindings listed by the help overlay while the study is open.
const CONTROLS: [Binding; 7] = [
    ("g", "toggle the image-backed portrait"),
    ("space", "pause or resume the scene"),
    ("r", "replay the reveal"),
    ("h / ? / F1", "controls and diagnostics"),
    ("F2", "renderer showcase"),
    ("Esc", "leave the study"),
    ("q / Ctrl+C", "quit Grimmoire"),
];

/// Animations owned by the study.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StageKey {
    /// The opening reveal.
    Reveal,
}

/// The presentation study screen.
#[derive(Debug)]
pub struct StageScreen {
    animator: Animator<StageKey>,
    scene_time: Duration,
    motion: bool,
    portrait: Arc<Image>,
    show_image: bool,
    frames: u64,
}

impl StageScreen {
    /// Creates the study with the reveal already playing.
    #[must_use]
    pub fn new() -> Self {
        let mut animator = Animator::new();
        animator.start(StageKey::Reveal, reveal());
        StageScreen {
            animator,
            scene_time: Duration::ZERO,
            motion: true,
            portrait: Arc::new(art::portrait(PORTRAIT_WIDTH, PORTRAIT_HEIGHT)),
            show_image: true,
            frames: 0,
        }
    }

    /// Whether the scene is currently moving.
    #[must_use]
    pub const fn is_moving(&self) -> bool {
        self.motion
    }

    /// Whether image-backed content is enabled.
    #[must_use]
    pub const fn show_image(&self) -> bool {
        self.show_image
    }

    /// Draws the header bar.
    fn compose_header(
        &self,
        frame: &mut Frame,
        rect: Rect,
        palette: &Palette,
        context: &FrameContext,
    ) {
        if rect.is_empty() {
            return;
        }
        let mut view = frame.view(rect);
        view.fill_style(view.bounds(), palette.raised_style());

        view.put_str(1, 0, "GRIMMOIRE", palette.accent_style().bold());
        view.put_str(11, 0, "presentation study", palette.muted_style());

        let right = format!(
            "{}x{}   {}   {:.0} fps",
            context.stats.width,
            context.stats.height,
            context.capabilities.colors.label(),
            fps(context.frame_time)
        );
        view.put_str(
            view.width()
                .saturating_sub(glyph::display_width(&right))
                .saturating_sub(1),
            0,
            &right,
            palette.muted_style(),
        );
    }

    /// Draws the environment scene panel.
    fn compose_scene(&self, frame: &mut Frame, rect: Rect, palette: &Palette) {
        if rect.width < 4 || rect.height < 3 {
            return;
        }
        let mut panel = frame.view(rect);
        panel.draw_titled_border(
            panel.bounds(),
            "the vale at dusk",
            palette.border_style(),
            palette.accent_style().bold(),
        );
        let inner = Rect::new(
            1,
            1,
            rect.width.saturating_sub(2),
            rect.height.saturating_sub(2),
        );
        let mut view = panel.sub(inner);
        compose_vale(&mut view, palette, self.scene_time.as_secs_f32());
    }

    /// Draws the portrait plate: an image where possible, ASCII art otherwise.
    fn compose_portrait(
        &self,
        frame: &mut Frame,
        rect: Rect,
        palette: &Palette,
        context: &FrameContext,
    ) {
        if rect.width < 4 || rect.height < 3 {
            return;
        }
        {
            let mut panel = frame.view(rect);
            panel.draw_titled_border(
                panel.bounds(),
                "portrait",
                palette.border_style(),
                palette.accent_style().bold(),
            );
        }

        let inner = Rect::new(
            rect.x.saturating_add(1),
            rect.y.saturating_add(1),
            rect.width.saturating_sub(2),
            rect.height.saturating_sub(2),
        );
        if inner.is_empty() {
            return;
        }

        if self.show_image && context.image_backend.is_available() {
            // Intent only: whether these pixels are painted is the terminal's
            // business, and the presenter reserves the rectangle either way.
            frame.place_image(inner, Arc::clone(&self.portrait));
            return;
        }

        let mut view = frame.view(inner);
        compose_hooded_figure(&mut view, palette, self.show_image);
    }

    /// Draws the detail column: the theme, and the live render path.
    fn compose_detail(
        &self,
        frame: &mut Frame,
        rect: Rect,
        palette: &Palette,
        context: &FrameContext,
    ) {
        if rect.width < 6 || rect.height < 4 {
            return;
        }
        let mut panel = frame.view(rect);
        panel.draw_titled_border(
            panel.bounds(),
            "detail",
            palette.border_style(),
            palette.accent_style().bold(),
        );
        let inset = Rect::new(
            2,
            1,
            rect.width.saturating_sub(4),
            rect.height.saturating_sub(2),
        );
        if inset.is_empty() {
            return;
        }
        let mut view = panel.sub(inset);
        // `sub` re-bases the origin, so all content coordinates are relative to
        // the inset view, not to the panel.
        let area = view.bounds();
        let mut row = area.y;

        row = heading(&mut view, area, row, "theme", palette);
        let roles: [(&str, Style); 6] = [
            ("text", palette.text_style()),
            ("muted", palette.muted_style()),
            ("accent", palette.accent_style()),
            ("alt", Style::new().with_fg(palette.accent_alt)),
            ("effect", palette.effect_style()),
            ("nature", Style::new().with_fg(palette.nature)),
        ];
        for (name, style) in roles {
            if row >= area.bottom() {
                break;
            }
            // A full block is one column wide, so it is a genuine colour swatch.
            view.put_char(area.x, row, '█', style);
            view.put_str(area.x.saturating_add(2), row, name, palette.muted_style());
            row = row.saturating_add(1);
        }

        row = row.saturating_add(1);
        row = heading(&mut view, area, row, "render path", palette);
        let stats = context.stats;
        let backend = context.image_backend.label();
        let metrics = match context.capabilities.cell_metrics {
            Some(metrics) => format!("{}x{} px", metrics.width, metrics.height),
            None => "unreported".to_string(),
        };
        let lines = [
            ("surface", "opaque, owned".to_string()),
            ("colour", context.capabilities.colors.label().to_string()),
            (
                "graphics",
                context.capabilities.graphics.label().to_string(),
            ),
            ("backend", backend.to_string()),
            ("cell", metrics),
            (
                "cells",
                format!("{} / {}", stats.cells_written, stats.total_cells()),
            ),
            ("runs", format!("{}", stats.runs)),
            ("images", format!("{}", stats.images_drawn)),
            ("frame", format!("{}", self.frames)),
        ];
        for (label, value) in lines {
            if row >= area.bottom() {
                break;
            }
            view.put_str(area.x, row, label, palette.muted_style());
            view.put_str(
                area.x.saturating_add(9),
                row,
                &value,
                Style::new().with_fg(palette.text),
            );
            row = row.saturating_add(1);
        }
    }

    /// Draws the status bar.
    fn compose_status(
        &self,
        frame: &mut Frame,
        rect: Rect,
        palette: &Palette,
        context: &FrameContext,
    ) {
        if rect.is_empty() {
            return;
        }
        let mut view = frame.view(rect);
        view.fill_style(view.bounds(), palette.raised_style());

        let mode = if self.motion { "in motion" } else { "paused" };
        let portrait = if self.show_image && context.image_backend.is_available() {
            "portrait: image"
        } else {
            "portrait: ascii"
        };
        let left = format!("{mode}   {portrait}");
        view.put_str(1, 0, &left, palette.muted_style());

        if rect.height > 1 {
            view.put_str_centered(
                Rect::new(0, 1, view.width(), 1),
                1,
                "g portrait    space pause    r replay    F2 showcase    Esc leave    q quit",
                palette.muted_style(),
            );
        }
    }

    /// Masks the not-yet-revealed part of the frame.
    fn compose_reveal(&self, frame: &mut Frame, palette: &Palette) {
        let progress = self
            .animator
            .value_or(StageKey::Reveal, 1.0)
            .clamp(0.0, 1.0);
        if progress >= 1.0 {
            return;
        }
        let area = frame.bounds();
        if area.is_empty() {
            return;
        }
        let revealed = (progress * f32::from(area.width)).ceil() as u16;
        let mut view = frame.view(area);
        let mask = palette.surface_style();
        let edge = Style::new()
            .with_fg(palette.accent_alt)
            .with_bg(palette.surface_raised)
            .bold();
        for y in 0..area.height {
            for x in revealed..area.width {
                if x == revealed {
                    view.put_char(x, y, '|', edge);
                } else {
                    view.put_char(x, y, ' ', mask);
                }
            }
        }
    }
}

impl Default for StageScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl Screen for StageScreen {
    fn name(&self) -> &'static str {
        "stage"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        match key.code {
            KeyCode::Esc => Command::Pop,
            KeyCode::Char('g') | KeyCode::Char('G') => {
                if key.is_repeat() {
                    return Command::None;
                }
                self.show_image = !self.show_image;
                Command::None
            }
            KeyCode::Char(' ') => {
                if key.is_repeat() {
                    return Command::None;
                }
                self.motion = !self.motion;
                Command::None
            }
            KeyCode::Char('r') => {
                if key.is_repeat() {
                    return Command::None;
                }
                self.animator.start(StageKey::Reveal, reveal());
                Command::None
            }
            KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Function(1) => {
                if key.is_repeat() {
                    return Command::None;
                }
                Command::Push(Box::new(HelpOverlay::new(&CONTROLS)))
            }
            KeyCode::Function(2) => {
                if key.is_repeat() {
                    return Command::None;
                }
                Command::Push(Box::new(Showcase::new()))
            }
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.animator.tick(delta);
        self.frames = self.frames.saturating_add(1);
        if self.motion {
            self.scene_time = self.scene_time.saturating_add(delta);
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let area = frame.bounds();
        if area.width < 40 || area.height < 12 {
            // The application's viewport policy normally catches this first; the
            // study still refuses to draw a mangled layout on its own.
            let palette = &context.palette;
            let mut view = frame.view(area);
            view.fill_style(view.bounds(), palette.surface_style());
            let row = area.centered_y(1);
            view.put_str_centered(
                view.bounds(),
                row,
                "enlarge the window to study this",
                palette.muted_style(),
            );
            return;
        }

        let palette = &context.palette;
        let layout = StageLayout::compute(area);
        self.compose_header(frame, layout.header, palette, context);
        self.compose_scene(frame, layout.scene, palette);
        self.compose_portrait(frame, layout.portrait, palette, context);
        self.compose_detail(frame, layout.detail, palette, context);
        self.compose_status(frame, layout.status, palette, context);
        self.compose_reveal(frame, palette);
    }

    fn is_animating(&self) -> bool {
        self.animator.active_count() > 0 || self.motion
    }
}

/// The regions of the study.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StageLayout {
    header: Rect,
    scene: Rect,
    portrait: Rect,
    detail: Rect,
    status: Rect,
}

impl StageLayout {
    /// Computes the regions for `area`.
    fn compute(area: Rect) -> Self {
        let header = Rect::new(area.x, area.y, area.width, 1);
        let status_height = area.height.min(2);
        let status = Rect::new(
            area.x,
            area.bottom().saturating_sub(status_height),
            area.width,
            status_height,
        );
        let body = Rect::new(
            area.x,
            header.bottom(),
            area.width,
            status.y.saturating_sub(header.bottom()),
        );

        // A third of the width goes to the right-hand column, with a floor so the
        // panels stay legible and a ceiling so the scene keeps the room. A
        // one-column gutter keeps the two panels from sharing a border.
        let right_width = (body.width / 3)
            .clamp(24, 34)
            .min(body.width.saturating_sub(25));
        let scene_width = body.width.saturating_sub(right_width).saturating_sub(1);
        let scene = Rect::new(body.x, body.y, scene_width, body.height);
        let right = Rect::new(
            body.x.saturating_add(scene_width).saturating_add(1),
            body.y,
            right_width,
            body.height,
        );

        let portrait_height = (right.height * 3 / 5).clamp(5, right.height);
        let portrait = Rect::new(right.x, right.y, right.width, portrait_height);
        let detail = Rect::new(
            right.x,
            right.y.saturating_add(portrait_height),
            right.width,
            right.height.saturating_sub(portrait_height),
        );

        StageLayout {
            header,
            scene,
            portrait,
            detail,
            status,
        }
    }
}

/// The reveal animation.
fn reveal() -> Animation {
    Animation::new(REVEAL).eased(Easing::EaseInOutCubic)
}

/// Formats a frame time as a rate.
fn fps(frame_time: Duration) -> f32 {
    if frame_time.is_zero() {
        0.0
    } else {
        1.0 / frame_time.as_secs_f32()
    }
}

/// Writes a section heading and returns the next row.
fn heading(view: &mut CanvasView, inner: Rect, row: u16, text: &str, palette: &Palette) -> u16 {
    if row >= inner.bottom() {
        return row;
    }
    view.put_str(inner.x, row, text, palette.accent_style().bold());
    row.saturating_add(1)
}

/// Draws the ASCII fallback portrait.
fn compose_hooded_figure(view: &mut CanvasView, palette: &Palette, image_enabled: bool) {
    let width = view.width();
    let height = view.height();
    if width < 6 || height < 4 {
        return;
    }
    let centre = width / 2;
    let head = (height / 3).max(1);

    // A soft halo behind the figure, denser nearer the head. The vertical axis
    // is squashed because a cell is roughly twice as tall as it is wide.
    let radius = (f32::from(width) / 2.2).max(3.0);
    let inner = Style::new().with_fg(palette.accent_alt_dim);
    let outer = palette.muted_style();
    for y in 0..height {
        for x in 0..width {
            let dx = f32::from(x) - f32::from(centre);
            let dy = (f32::from(y) - f32::from(head)) * 0.55;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance < radius * 0.5 {
                view.put_char(x, y, '░', inner);
            } else if distance < radius {
                view.put_char(x, y, '·', outer);
            }
        }
    }

    view.put_str_centered(
        view.bounds(),
        head,
        ".-.",
        Style::new().with_fg(palette.text),
    );
    view.put_str_centered(
        view.bounds(),
        head + 1,
        "(o o)",
        Style::new().with_fg(palette.text),
    );
    view.put_str_centered(
        view.bounds(),
        head + 2,
        "|:::",
        Style::new().with_fg(palette.text),
    );
    view.put_str_centered(
        view.bounds(),
        head + 3,
        "/|_|\\",
        Style::new().with_fg(palette.text),
    );
    for row in (head + 4)..height.saturating_sub(1) {
        view.put_str_centered(
            view.bounds(),
            row,
            "/___\\",
            Style::new().with_fg(palette.text_muted),
        );
    }

    // Say why the plate is not a picture, so the difference is diagnosable.
    let note = if image_enabled {
        "no image backend"
    } else {
        "image disabled"
    };
    view.put_str_centered(
        view.bounds(),
        height.saturating_sub(1),
        note,
        palette.muted_style(),
    );
}

/// Draws the vale: sky, ridge, treeline, ground, fire, and drifting mist.
///
/// Everything animated is a pure function of `time`, so the scene is identical
/// across runs and testable without a terminal.
fn compose_vale(view: &mut CanvasView, palette: &Palette, time: f32) {
    let width = view.width();
    let height = view.height();
    if width < 12 || height < 6 {
        return;
    }
    let w = i32::from(width);
    let h = i32::from(height);
    let horizon = ((h as f32) * 0.62) as i32;

    let sky_style = Style::new().with_fg(palette.accent_alt_dim).dim();
    let bright_star = Style::new().with_fg(palette.accent_alt);
    let ridge_style = Style::new().with_fg(palette.text_muted);
    let ridge_top = Style::new().with_fg(palette.text_muted).bold();
    let tree_style = Style::new().with_fg(palette.nature);
    let ground_style = Style::new().with_fg(palette.text_muted).dim();
    let path_style = Style::new().with_fg(palette.text);
    let ember_style = Style::new().with_fg(palette.effect);
    let ember_dim = Style::new().with_fg(palette.effect_dim);
    let mist_style = Style::new().with_fg(palette.accent_alt_dim);

    // Stars.
    let stars = ((w * h) / 90).clamp(6, 48);
    for i in 0..stars {
        let x = (hash(i as u32 * 7 + 1) % w as u32) as i32;
        let top = (horizon * 45 / 100).max(1);
        let y = (hash(i as u32 * 11 + 3) % top as u32) as i32;
        let phase = unit(i as u32 * 13 + 5) * std::f32::consts::TAU;
        let twinkle = (time * 1.7 + phase).sin() * 0.5 + 0.5;
        let style = if twinkle > 0.7 {
            bright_star
        } else {
            sky_style
        };
        let ch = if twinkle > 0.85 { '*' } else { '.' };
        view.put_char(x as u16, y as u16, ch, style);
    }

    // A moon, high and to the right.
    let moon_x = (w * 4 / 5) as u16;
    let moon_y = (h / 8).max(1) as u16;
    view.put_str(moon_x, moon_y, "O", bright_star.bold());

    // The ridge: a silhouette with a lighter crest.
    for x in 0..w {
        let t = x as f32 / w as f32;
        let crest = ((t * 7.0).sin() * 0.5 + 0.5) * (h as f32 * 0.16)
            + ((t * 17.0).cos() * 0.5 + 0.5) * (h as f32 * 0.06);
        let top = horizon - 1 - crest as i32;
        for y in top.max(0)..horizon {
            let style = if y == top { ridge_top } else { ridge_style };
            let ch = if y == top { '^' } else { '#' };
            view.put_char(x as u16, y as u16, ch, style);
        }
    }

    // The treeline, just below the ridge.
    let mut x = 1;
    while x < w - 1 {
        let lean = hash(x as u32 * 5 + 9) % 2 == 0;
        let base = horizon;
        view.put_char(
            x as u16,
            base as u16,
            if lean { '/' } else { '\\' },
            tree_style,
        );
        view.put_char(
            (x + 1) as u16,
            base as u16,
            if lean { '\\' } else { '/' },
            tree_style,
        );
        if base + 1 < h {
            view.put_char(x as u16, (base + 1) as u16, '|', tree_style);
        }
        x += 3 + (hash(x as u32 * 3 + 2) % 3) as i32;
    }

    // Ground and the path leading away.
    let centre = w / 2;
    for y in (horizon + 1)..h {
        for xx in 0..w {
            view.put_char(xx as u16, y as u16, '.', ground_style);
        }
        let half = 1 + (y - horizon) / 3;
        for xx in (centre - half).max(0)..=(centre + half).min(w - 1) {
            view.put_char(xx as u16, y as u16, ',', path_style);
        }
    }

    // The campfire, with a flicker and a few rising embers.
    let fire_x = centre - w / 4;
    let fire_y = (h - 3).max(horizon + 1);
    if fire_x > 0 && fire_y > horizon {
        let flicker =
            ((time * 9.0).sin() * 0.5 + 0.5) * 0.65 + ((time * 23.0).sin() * 0.5 + 0.5) * 0.35;
        let (ch, style) = if flicker > 0.74 {
            ('*', ember_style.bold())
        } else if flicker > 0.42 {
            ('x', ember_style)
        } else {
            ('+', ember_dim)
        };
        view.put_char(fire_x as u16, fire_y as u16, ch, style);
        view.put_char((fire_x - 1) as u16, fire_y as u16, '/', ember_dim);
        view.put_char((fire_x + 1) as u16, fire_y as u16, '\\', ember_dim);

        for i in 0..3u32 {
            let rise = (time * (0.8 + i as f32 * 0.35) + unit(i * 17 + 3)).rem_euclid(3.0);
            let ey = fire_y - 1 - rise as i32;
            let ex = fire_x + ((i as i32 % 2) * 2 - 1);
            if ey > horizon && ex > 0 && ex < w {
                view.put_char(ex as u16, ey as u16, '·', ember_dim);
            }
        }
    }

    // Drifting mist along the bottom rows.
    let mist_rows = 2.min(height.saturating_sub(1));
    for row in 0..mist_rows {
        let y = h - 1 - i32::from(row);
        let speed = 0.9 + row as f32 * 0.5;
        let count = (width / 4).max(2);
        for i in 0..count {
            let base = unit(u32::from(i) * 23 + u32::from(row) * 31) * w as f32;
            let x = ((base + time * speed * 4.0) % w as f32) as i32;
            if x >= 0 && x < w {
                view.put_char(x as u16, y as u16, '~', mist_style);
            }
        }
    }
}

/// A deterministic integer hash.
fn hash(seed: u32) -> u32 {
    let mut value = seed.wrapping_add(0x9E37_79B9);
    value = (value ^ (value >> 16)).wrapping_mul(0x85EB_CA6B);
    value = (value ^ (value >> 13)).wrapping_mul(0xC2B2_AE35);
    value ^ (value >> 16)
}

/// A deterministic value in `[0, 1)` derived from a seed.
fn unit(seed: u32) -> f32 {
    (hash(seed) >> 8) as f32 / 16_777_216.0
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{StageLayout, StageScreen};
    use crate::input::{Key, KeyCode};
    use crate::render::{Canvas, Color, Frame};
    use crate::terminal::{Capabilities, ImageBackend, PresentStats};
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            image_backend: ImageBackend::None,
            stats: PresentStats {
                width: 100,
                height: 30,
                cells_written: 640,
                runs: 9,
                images_drawn: 0,
                ..PresentStats::default()
            },
            frame_time: Duration::from_millis(8),
            animating: true,
        }
    }

    /// Composes the study into a canvas and returns it with the image placements.
    fn compose_into(
        screen: &StageScreen,
        width: u16,
        height: u16,
        context: &FrameContext,
    ) -> (Canvas, Vec<crate::render::ImagePlacement>) {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(context.palette.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, context);
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
    fn regions_do_not_overlap() {
        let layout = StageLayout::compute(crate::render::Rect::from_size(100, 30));
        assert_eq!(layout.portrait.x, layout.scene.right() + 1);
        assert_eq!(layout.portrait.bottom(), layout.detail.y);
        assert_eq!(layout.header.bottom(), layout.scene.y);
        assert_eq!(layout.scene.bottom(), layout.status.y);
    }

    #[test]
    fn every_region_stays_inside_the_viewport() {
        for (width, height) in [(80u16, 24u16), (100, 30), (200, 60), (81, 25)] {
            let area = crate::render::Rect::from_size(width, height);
            let layout = StageLayout::compute(area);
            for region in [
                layout.header,
                layout.scene,
                layout.portrait,
                layout.detail,
                layout.status,
            ] {
                assert!(
                    region.right() <= area.right() && region.bottom() <= area.bottom(),
                    "{region:?} escapes {area:?}"
                );
            }
        }
    }

    #[test]
    fn the_whole_viewport_is_painted_opaque() {
        let screen = StageScreen::new();
        let (canvas, _) = compose_into(&screen, 100, 30, &context());
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                assert_ne!(
                    Some(Color::Default),
                    canvas.get(x, y).map(|cell| cell.style.bg),
                    "cell ({x}, {y}) would show the terminal background"
                );
            }
        }
    }

    #[test]
    fn the_study_names_itself_and_its_panels() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2)); // finish the reveal
        let (canvas, _) = compose_into(&screen, 100, 30, &context());
        let text = frame_text(&canvas);
        assert!(text.contains("GRIMMOIRE"), "missing identity");
        assert!(text.contains("the vale at dusk"), "missing scene");
        assert!(text.contains("portrait"), "missing portrait panel");
        assert!(text.contains("render path"), "missing diagnostics");
    }

    #[test]
    fn the_scene_animates_over_time() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2));
        let (before, _) = compose_into(&screen, 100, 30, &context());
        screen.update(Duration::from_millis(400));
        let (after, _) = compose_into(&screen, 100, 30, &context());
        assert_ne!(frame_text(&before), frame_text(&after));
    }

    #[test]
    fn pausing_stops_the_scene_and_the_animation_demand() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2)); // finish the reveal
        assert!(screen.is_animating(), "the scene moves by default");

        screen.handle_key(Key::char(' '));
        assert!(!screen.is_animating());
        assert!(!screen.is_moving());
    }

    #[test]
    fn an_image_is_placed_only_when_a_backend_exists() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2));

        let with_backend = FrameContext {
            image_backend: ImageBackend::Sixel,
            ..context()
        };
        let (_, images) = compose_into(&screen, 100, 30, &with_backend);
        assert_eq!(1, images.len(), "expected a portrait placement");

        let (fallback, images) = compose_into(&screen, 100, 30, &context());
        assert!(images.is_empty());
        assert!(
            frame_text(&fallback).contains("no image backend"),
            "the fallback must explain itself"
        );
    }

    #[test]
    fn the_portrait_can_be_switched_off() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2));
        let with_backend = FrameContext {
            image_backend: ImageBackend::Sixel,
            ..context()
        };
        screen.handle_key(Key::char('g'));
        assert!(!screen.show_image());

        let (canvas, images) = compose_into(&screen, 100, 30, &with_backend);
        assert!(images.is_empty());
        assert!(frame_text(&canvas).contains("image disabled"));
    }

    #[test]
    fn repeat_keys_do_not_toggle_state() {
        let mut screen = StageScreen::new();
        screen.handle_key(Key::char('g').with_repeat());
        assert!(screen.show_image());
        screen.handle_key(Key::char(' ').with_repeat());
        assert!(screen.is_moving());
    }

    #[test]
    fn escape_leaves_and_help_opens() {
        let mut screen = StageScreen::new();
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Esc)),
            Command::Pop
        ));
        assert!(matches!(
            screen.handle_key(Key::char('h')),
            Command::Push(_)
        ));
    }

    #[test]
    fn composition_is_deterministic() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(3));
        let (first, _) = compose_into(&screen, 100, 30, &context());
        let (second, _) = compose_into(&screen, 100, 30, &context());
        assert_eq!(frame_text(&first), frame_text(&second));
    }

    #[test]
    fn composition_is_safe_across_sizes() {
        let mut screen = StageScreen::new();
        screen.update(Duration::from_secs(2));
        for width in [0, 1, 20, 40, 80, 100, 240] {
            for height in [0, 1, 6, 12, 24, 40] {
                let mut canvas = Canvas::new(width, height);
                canvas.clear_with(Palette::EXTENDED.backdrop());
                let mut images = Vec::new();
                let mut frame = Frame::begin(&mut canvas, &mut images);
                screen.compose(&mut frame, &context());
            }
        }
    }

    #[test]
    fn the_study_refuses_to_draw_a_mangled_layout() {
        let screen = StageScreen::new();
        let (canvas, _) = compose_into(&screen, 30, 10, &context());
        assert!(frame_text(&canvas).contains("enlarge the window"));
    }
}
