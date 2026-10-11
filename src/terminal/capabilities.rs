//! Runtime detection of what the current terminal can do.
//!
//! Colour is the only thing decided here; graphics capability lives in
//! [`super::graphics`], because it is a policy (how to behave when Kitty is
//! missing) rather than a measurement.
//!
//! Detection is best-effort and never authoritative. Colour support selects a
//! palette and nothing else, so a wrong guess is cosmetic.

use crossterm::style::available_color_count;

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

/// What the current terminal can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities {
    /// Detected colour support.
    pub colors: ColorSupport,
}

impl Capabilities {
    /// Detects capabilities from the environment.
    #[must_use]
    pub fn detect() -> Self {
        Capabilities {
            colors: color_support_from(available_color_count()),
        }
    }

    /// A conservative baseline used when detection is unavailable or undesired.
    ///
    /// Deliberately colour-free so tests never depend on the environment.
    #[must_use]
    pub const fn conservative() -> Self {
        Capabilities {
            colors: ColorSupport::Ansi16,
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

#[cfg(test)]
mod tests {
    use super::{Capabilities, ColorSupport, color_support_from};

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
    fn the_conservative_baseline_claims_nothing() {
        assert_eq!(ColorSupport::Ansi16, Capabilities::conservative().colors);
    }
}
