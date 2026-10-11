//! Runtime detection of what the current terminal can do.
//!
//! Detection is deliberately best-effort and only informs presentation choices
//! and diagnostics; nothing here changes the rendering model. The colour result
//! selects the palette, and the graphics result is reported to the user without
//! being used, because Grimmoire renders in cells (see `docs/renderer-design.md`).

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

    /// Whether the palette has more than the classic 16 colours available.
    #[must_use]
    pub const fn has_extended_palette(self) -> bool {
        matches!(self, ColorSupport::Ansi256 | ColorSupport::TrueColor)
    }
}

/// A terminal graphics protocol that was detected but is not used for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GraphicsSupport {
    /// No known graphics protocol was detected.
    #[default]
    None,
    /// The Kitty graphics protocol.
    Kitty,
    /// The iTerm2 inline image protocol.
    Iterm2,
    /// DEC sixel graphics.
    Sixel,
}

impl GraphicsSupport {
    /// A short human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            GraphicsSupport::None => "none",
            GraphicsSupport::Kitty => "kitty",
            GraphicsSupport::Iterm2 => "iterm2",
            GraphicsSupport::Sixel => "sixel",
        }
    }
}

/// The capabilities of the current terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Detected colour support.
    pub colors: ColorSupport,
    /// Detected graphics protocol, if any (informational only today).
    pub graphics: GraphicsSupport,
}

impl Capabilities {
    /// Detects capabilities from the environment. Reads process environment.
    #[must_use]
    pub fn detect() -> Self {
        let colors = color_support_from(crossterm::style::available_color_count());
        let graphics = graphics_support_from(
            &std::env::var("TERM").unwrap_or_default(),
            &std::env::var("TERM_PROGRAM").unwrap_or_default(),
            std::env::var_os("KITTY_WINDOW_ID").is_some(),
        );
        Capabilities { colors, graphics }
    }

    /// A conservative baseline used when detection is unavailable or undesired.
    #[must_use]
    pub const fn conservative() -> Self {
        Capabilities {
            colors: ColorSupport::Ansi16,
            graphics: GraphicsSupport::None,
        }
    }
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
/// This is a hint, not an authoritative query: a terminal can support a protocol
/// without advertising it, and multiplexers can swallow the protocol. Authoritative
/// detection requires querying the terminal with control sequences, which the
/// project has not needed yet.
fn graphics_support_from(term: &str, term_program: &str, kitty_window: bool) -> GraphicsSupport {
    if kitty_window || term.contains("kitty") || term.contains("ghostty") {
        return GraphicsSupport::Kitty;
    }
    if term_program.contains("WezTerm") {
        // WezTerm speaks the Kitty protocol as well as its own.
        return GraphicsSupport::Kitty;
    }
    if term_program.contains("iTerm") {
        return GraphicsSupport::Iterm2;
    }
    if term.contains("foot") || term.contains("mlterm") || term.contains("sixel") {
        return GraphicsSupport::Sixel;
    }
    GraphicsSupport::None
}

#[cfg(test)]
mod tests {
    use super::{
        Capabilities, ColorSupport, GraphicsSupport, color_support_from, graphics_support_from,
    };

    #[test]
    fn colour_counts_map_to_support_levels() {
        assert_eq!(ColorSupport::Ansi16, color_support_from(8));
        assert_eq!(ColorSupport::Ansi16, color_support_from(16));
        assert_eq!(ColorSupport::Ansi256, color_support_from(256));
        assert_eq!(ColorSupport::TrueColor, color_support_from(u16::MAX));
    }

    #[test]
    fn extended_palette_reports_correctly() {
        assert!(!ColorSupport::Ansi16.has_extended_palette());
        assert!(ColorSupport::Ansi256.has_extended_palette());
        assert!(ColorSupport::TrueColor.has_extended_palette());
    }

    #[test]
    fn graphics_hints_are_recognised() {
        assert_eq!(
            GraphicsSupport::Kitty,
            graphics_support_from("xterm-kitty", "", false)
        );
        assert_eq!(
            GraphicsSupport::Kitty,
            graphics_support_from("xterm-256color", "WezTerm", false)
        );
        assert_eq!(
            GraphicsSupport::Iterm2,
            graphics_support_from("xterm-256color", "iTerm.app", false)
        );
        assert_eq!(
            GraphicsSupport::Sixel,
            graphics_support_from("foot", "", false)
        );
        assert_eq!(
            GraphicsSupport::None,
            graphics_support_from("xterm-256color", "Apple_Terminal", false)
        );
    }

    #[test]
    fn conservative_default_uses_the_basic_palette() {
        assert_eq!(
            Capabilities::conservative(),
            Capabilities {
                colors: ColorSupport::Ansi16,
                graphics: GraphicsSupport::None
            }
        );
    }
}
