//! Runtime detection of what the current terminal can do.
//!
//! Detection is best-effort and never authoritative, so every value here is
//! expressed as something a caller can act on *safely*:
//!
//! * colour support selects the palette;
//! * graphics support is three-valued, so "we could not tell" is never confused
//!   with "yes" or "no"; and
//! * cell metrics are `Option`, because a terminal that cannot report pixel
//!   dimensions cannot size an image to a rectangle of cells.
//!
//! Nothing here changes the rendering model on its own.

use crossterm::style::available_color_count;
use crossterm::terminal::window_size;

/// How many colours the terminal can display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorSupport {
    /// The classic 8/16-colour ANSI palette.
    #[default]
    Ansi16,
    /// The extended 256-colour palette.
    Ansi256,
    /// 24-bit direct colour.
    TrueColor,
}

impl ColorSupport {
    /// A short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ColorSupport::Ansi16 => "16-colour",
            ColorSupport::Ansi256 => "256-colour",
            ColorSupport::TrueColor => "truecolour",
        }
    }

    /// Whether more than the classic 16 colours are available.
    #[must_use]
    pub const fn has_extended_palette(self) -> bool {
        matches!(self, ColorSupport::Ansi256 | ColorSupport::TrueColor)
    }
}

/// A terminal graphics protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    /// The Kitty graphics protocol.
    Kitty,
    /// The iTerm2 inline image protocol.
    Iterm2,
    /// DEC sixel graphics.
    Sixel,
}

impl GraphicsProtocol {
    /// A short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            GraphicsProtocol::Kitty => "kitty",
            GraphicsProtocol::Iterm2 => "iterm2",
            GraphicsProtocol::Sixel => "sixel",
        }
    }
}

/// What is known about the terminal's graphics capabilities.
///
/// The distinction matters: acting on [`GraphicsSupport::Unknown`] as if it were
/// support is how a program ends up painting protocol bytes into a terminal that
/// renders them as garbage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphicsSupport {
    /// Nothing in the environment hinted either way.
    #[default]
    Unknown,
    /// Graphics were explicitly disabled, or the terminal is known not to
    /// support any protocol we can use.
    Unsupported,
    /// The environment suggests this protocol; it is a hint, not a promise.
    Likely(GraphicsProtocol),
}

impl GraphicsSupport {
    /// A short human-readable label for diagnostics.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            GraphicsSupport::Unknown => "unknown",
            GraphicsSupport::Unsupported => "none",
            GraphicsSupport::Likely(GraphicsProtocol::Kitty) => "kitty?",
            GraphicsSupport::Likely(GraphicsProtocol::Iterm2) => "iterm2?",
            GraphicsSupport::Likely(GraphicsProtocol::Sixel) => "sixel?",
        }
    }

    /// The protocol hinted at, if any.
    #[must_use]
    pub const fn protocol(self) -> Option<GraphicsProtocol> {
        match self {
            GraphicsSupport::Likely(protocol) => Some(protocol),
            _ => None,
        }
    }
}

/// The pixel size of one character cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellMetrics {
    /// Cell width in pixels.
    pub width: u16,
    /// Cell height in pixels.
    pub height: u16,
}

impl CellMetrics {
    /// Reads the character cell size from the terminal, if it reports it.
    ///
    /// Returns `None` when the terminal does not report pixel dimensions, which
    /// is common (and always the case on some platforms). Callers must treat that
    /// as "graphics unavailable" rather than guessing, because a wrong cell size
    /// produces images that overlap neighbouring panels.
    #[must_use]
    pub fn detect() -> Option<CellMetrics> {
        let window = window_size().ok()?;
        CellMetrics::from_window(window.columns, window.rows, window.width, window.height)
    }

    /// Derives cell metrics from a terminal window-size report.
    #[must_use]
    pub fn from_window(columns: u16, rows: u16, width: u16, height: u16) -> Option<CellMetrics> {
        if columns == 0 || rows == 0 || width == 0 || height == 0 {
            return None;
        }
        let cell_width = width / columns;
        let cell_height = height / rows;
        if cell_width == 0 || cell_height == 0 {
            return None;
        }
        Some(CellMetrics {
            width: cell_width,
            height: cell_height,
        })
    }
}

/// What the current terminal can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Detected colour support.
    pub colors: ColorSupport,
    /// Detected graphics support.
    pub graphics: GraphicsSupport,
    /// The character cell size in pixels, when the terminal reports it.
    pub cell_metrics: Option<CellMetrics>,
}

impl Capabilities {
    /// Detects capabilities from the environment and the terminal.
    #[must_use]
    pub fn detect() -> Self {
        let colors = color_support_from(available_color_count());
        let graphics = graphics_support_from(
            &std::env::var("TERM").unwrap_or_default(),
            &std::env::var("TERM_PROGRAM").unwrap_or_default(),
            std::env::var_os("KITTY_WINDOW_ID").is_some(),
            graphics_disabled(),
        );
        let cell_metrics = CellMetrics::detect();
        Capabilities {
            colors,
            graphics,
            cell_metrics,
        }
    }

    /// A conservative baseline used when detection is unavailable or undesired.
    ///
    /// Deliberately colour- and graphics-free so tests never depend on the
    /// environment or the terminal.
    #[must_use]
    pub const fn conservative() -> Self {
        Capabilities {
            colors: ColorSupport::Ansi16,
            graphics: GraphicsSupport::Unknown,
            cell_metrics: None,
        }
    }
}

/// Whether the user or environment has explicitly disabled graphics.
fn graphics_disabled() -> bool {
    matches!(
        std::env::var("GRIMMOIRE_NO_GRAPHICS").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

/// Maps a colour count from the terminal library onto [`ColorSupport`].
fn color_support_from(count: u16) -> ColorSupport {
    if count == u16::MAX {
        ColorSupport::TrueColor
    } else if count >= 256 {
        ColorSupport::Ansi256
    } else {
        ColorSupport::Ansi16
    }
}

/// Guesses a graphics protocol from well-known environment hints.
///
/// This is a hint, not a query. A terminal can support a protocol without
/// advertising it, and a multiplexer can swallow the protocol entirely, so the
/// result is only ever used to *consider* a backend, never to assume one works.
fn graphics_support_from(
    term: &str,
    term_program: &str,
    kitty_window: bool,
    disabled: bool,
) -> GraphicsSupport {
    if disabled {
        return GraphicsSupport::Unsupported;
    }
    if term == "dumb" || term.is_empty() {
        return GraphicsSupport::Unsupported;
    }
    // Kitty and iTerm2 are detected so the diagnostics can say what was found,
    // even though this build only implements sixel.
    if kitty_window || term.contains("kitty") || term.contains("ghostty") {
        return GraphicsSupport::Likely(GraphicsProtocol::Kitty);
    }
    if term_program.contains("WezTerm") {
        return GraphicsSupport::Likely(GraphicsProtocol::Kitty);
    }
    if term_program.contains("iTerm") {
        return GraphicsSupport::Likely(GraphicsProtocol::Iterm2);
    }
    if term.contains("foot")
        || term.contains("mlterm")
        || term.contains("sixel")
        || term.contains("contour")
    {
        return GraphicsSupport::Likely(GraphicsProtocol::Sixel);
    }
    GraphicsSupport::Unknown
}

#[cfg(test)]
mod tests {
    use super::{
        Capabilities, CellMetrics, ColorSupport, GraphicsProtocol, GraphicsSupport,
        color_support_from, graphics_support_from,
    };

    #[test]
    fn colour_counts_map_to_support_levels() {
        assert_eq!(ColorSupport::Ansi16, color_support_from(8));
        assert_eq!(ColorSupport::Ansi16, color_support_from(16));
        assert_eq!(ColorSupport::Ansi256, color_support_from(256));
        assert_eq!(ColorSupport::TrueColor, color_support_from(u16::MAX));
    }

    #[test]
    fn sixel_hints_are_recognised() {
        assert_eq!(
            GraphicsSupport::Likely(GraphicsProtocol::Sixel),
            graphics_support_from("foot", "", false, false)
        );
        assert_eq!(
            GraphicsSupport::Likely(GraphicsProtocol::Sixel),
            graphics_support_from("xterm-sixel", "", false, false)
        );
    }

    #[test]
    fn other_protocols_are_detected_but_distinguished() {
        assert_eq!(
            GraphicsSupport::Likely(GraphicsProtocol::Kitty),
            graphics_support_from("xterm-kitty", "", false, false)
        );
        assert_eq!(
            GraphicsSupport::Likely(GraphicsProtocol::Iterm2),
            graphics_support_from("xterm-256color", "iTerm.app", false, false)
        );
    }

    #[test]
    fn unknown_and_unsupported_are_not_the_same() {
        assert_eq!(
            GraphicsSupport::Unknown,
            graphics_support_from("xterm-256color", "", false, false)
        );
        assert_eq!(
            GraphicsSupport::Unsupported,
            graphics_support_from("dumb", "", false, false)
        );
        assert_eq!(
            GraphicsSupport::Unsupported,
            graphics_support_from("foot", "", false, true)
        );
    }

    #[test]
    fn cell_metrics_derive_from_a_window_report() {
        let metrics = CellMetrics::from_window(100, 30, 800, 600).expect("metrics");
        assert_eq!(8, metrics.width);
        assert_eq!(20, metrics.height);
    }

    #[test]
    fn cell_metrics_reject_unusable_reports() {
        assert_eq!(None, CellMetrics::from_window(0, 30, 800, 600));
        assert_eq!(None, CellMetrics::from_window(100, 30, 0, 0));
        assert_eq!(None, CellMetrics::from_window(2000, 30, 800, 600));
    }

    #[test]
    fn the_conservative_baseline_claims_nothing() {
        let capabilities = Capabilities::conservative();
        assert_eq!(ColorSupport::Ansi16, capabilities.colors);
        assert_eq!(GraphicsSupport::Unknown, capabilities.graphics);
        assert_eq!(None, capabilities.cell_metrics);
    }
}
