//! Ornaments: the glyph vocabulary the whole interface is built from.
//!
//! One module decides what a frame, a rule, a selector and a marker look like, so
//! that the title screen, a conversation, the journal and the HUD are visibly the
//! same game rather than four panels that happen to be in the same program.
//!
//! Everything here is a glyph the cell grid can hold — one column wide, no
//! continuation cell, nothing that reflows a line. `render::glyph` is the
//! authority on that, and the tests here assert every ornament against it.

use crate::render::{CanvasView, Rect, Style, glyph};

/// The marker beside the selected row of a menu or a list of choices.
pub const SELECTOR: char = '▸';
/// A filled diamond, used to open and close a heading.
pub const DIAMOND: char = '◆';
/// A hollow diamond, for a quieter marker.
pub const DIAMOND_OPEN: char = '◇';
/// A four-pointed star, for titles and discoveries.
pub const STAR: char = '✦';
/// A softer star, for secondary marks.
pub const STAR_OPEN: char = '✧';
/// A small round mark, for lists and rules of dots.
pub const DOT: char = '·';
/// A heavy horizontal rule.
pub const RULE: char = '─';
/// A lighter horizontal rule.
pub const RULE_LIGHT: char = '╌';
/// A vertical rule.
pub const BAR: char = '│';
/// The four corners of a heavy frame, clockwise from the top left.
pub const CORNERS: [char; 4] = ['╭', '╮', '╰', '╯'];
/// The four corners of a double-ruled frame, clockwise from the top left.
pub const DOUBLE_CORNERS: [char; 4] = ['╔', '╗', '╚', '╝'];
/// A decorative flourish, for the end of a heading.
pub const FLOURISH: char = '❖';
/// Marks that drift through the title screen's sky, in the order they twinkle.
pub const MOTES: &[char] = &['·', '✦', '✧', '·', '✧', '·'];
/// Runes, for anything that is meant to look written rather than drawn.
pub const RUNES: &[char] = &['ᚠ', 'ᚱ', 'ᚦ', 'ᚨ', 'ᚲ', 'ᛊ', 'ᛏ'];

/// Draws a horizontal rule across `width`, with an ornament at each end.
///
/// The ends are what stop a rule reading as a stray line of text: a rule is a
/// drawn thing, and it should look like one.
pub fn rule(view: &mut CanvasView, x: u16, y: u16, width: u16, style: Style) {
    if width < 2 {
        return;
    }
    for column in x..x.saturating_add(width) {
        view.put_char(column, y, RULE, style);
    }
    view.put_char(x, y, DIAMOND_OPEN, style);
    view.put_char(
        x.saturating_add(width).saturating_sub(1),
        y,
        DIAMOND_OPEN,
        style,
    );
}

/// Draws `text` centred on row `y`, with a star at each side.
pub fn heading(view: &mut CanvasView, y: u16, text: &str, style: Style) {
    let bounds = view.bounds();
    let width = glyph::display_width(text);
    if width == 0 || width + 6 > bounds.width {
        return;
    }
    let start = bounds
        .x
        .saturating_add(bounds.width.saturating_sub(width + 4) / 2);
    view.put_char(start, y, STAR_OPEN, style);
    view.put_str(start.saturating_add(2), y, text, style);
    view.put_char(start.saturating_add(width + 3), y, STAR_OPEN, style);
}

/// Draws a frame around `area` with corner ornaments and a centred title.
///
/// The title is set into the top edge, so a panel is titled rather than merely
/// labelled, and the corners are rounded so the frame reads as a plate rather
/// than as a box drawn with the same glyphs the world uses for architecture.
pub fn plate(
    view: &mut CanvasView,
    area: Rect,
    title: Option<&str>,
    border: Style,
    title_style: Style,
) {
    if area.width < 4 || area.height < 2 {
        return;
    }
    let bounds = view.bounds();
    let left = area.x;
    let right = area.right().saturating_sub(1);
    let top = area.y;
    let bottom = area.bottom().saturating_sub(1);

    for x in left.saturating_add(1)..right {
        view.put_char(x, top, RULE, border);
        view.put_char(x, bottom, RULE, border);
    }
    for y in top.saturating_add(1)..bottom {
        view.put_char(left, y, BAR, border);
        view.put_char(right, y, BAR, border);
    }
    view.put_char(left, top, DOUBLE_CORNERS[0], border);
    view.put_char(right, top, DOUBLE_CORNERS[1], border);
    view.put_char(left, bottom, DOUBLE_CORNERS[2], border);
    view.put_char(right, bottom, DOUBLE_CORNERS[3], border);

    let Some(title) = title else {
        return;
    };
    let width = glyph::display_width(title);
    if width == 0 || width + 4 > area.width {
        return;
    }
    let start = left.saturating_add(area.width.saturating_sub(width + 4) / 2);
    view.put_char(start, top, DIAMOND, title_style);
    view.put_str(start.saturating_add(2), top, title, title_style);
    view.put_char(start.saturating_add(width + 3), top, DIAMOND, title_style);
    let _ = bounds;
}

/// The rune for an index, for anything that should look inscribed.
#[must_use]
pub fn rune(index: usize) -> char {
    RUNES[index % RUNES.len()]
}

/// The mote for an index, for drifting background detail.
#[must_use]
pub fn mote(index: usize) -> char {
    MOTES[index % MOTES.len()]
}

#[cfg(test)]
mod tests {
    use super::{BAR, CORNERS, DIAMOND, DIAMOND_OPEN, DOT, DOUBLE_CORNERS, FLOURISH, MOTES};
    use super::{RULE, RULE_LIGHT, RUNES, SELECTOR, STAR, STAR_OPEN};
    use crate::render::glyph;

    #[test]
    fn every_ornament_is_one_column_wide() {
        // A two-column ornament would push the rest of its line one cell right and
        // desynchronise the frame; a zero-width one would vanish silently.
        for ch in [
            SELECTOR,
            DIAMOND,
            DIAMOND_OPEN,
            STAR,
            STAR_OPEN,
            DOT,
            RULE,
            RULE_LIGHT,
            BAR,
            FLOURISH,
        ] {
            assert!(glyph::is_drawable(ch), "{ch:?} is not one column wide");
        }
        for ch in CORNERS
            .iter()
            .chain(DOUBLE_CORNERS.iter())
            .chain(MOTES)
            .chain(RUNES)
        {
            assert!(glyph::is_drawable(*ch), "{ch:?} is not one column wide");
        }
    }

    #[test]
    fn every_ornament_is_a_visible_glyph() {
        for ch in RUNES.iter().chain(MOTES.iter()) {
            assert!(
                !ch.is_whitespace() && !ch.is_control(),
                "{ch:?} is invisible"
            );
        }
    }
}
