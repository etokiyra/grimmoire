//! The renderer's terminal-independent colour palette.
//!
//! Drawing code only ever refers to [`Color`]; the terminal backend is the sole
//! place that translates these values into concrete escape sequences. This keeps
//! frame composition free of any backend dependency and easy to test.

/// A colour that can be applied to a cell's foreground or background.
///
/// The named variants mirror the classic 16-colour ANSI palette. `Light*`
/// variants are the bright/high-intensity versions of their base colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    /// Leave the terminal's configured default colour in place.
    #[default]
    Default,
    /// Standard black.
    Black,
    /// Standard red.
    Red,
    /// Standard green.
    Green,
    /// Standard yellow.
    Yellow,
    /// Standard blue.
    Blue,
    /// Standard magenta.
    Magenta,
    /// Standard cyan.
    Cyan,
    /// Standard white (the "light grey" slot on most terminals).
    White,
    /// Bright black / dark grey.
    DarkGrey,
    /// Bright red.
    LightRed,
    /// Bright green.
    LightGreen,
    /// Bright yellow.
    LightYellow,
    /// Bright blue.
    LightBlue,
    /// Bright magenta.
    LightMagenta,
    /// Bright cyan.
    LightCyan,
    /// A colour from the terminal's 256-colour palette.
    Indexed(u8),
    /// A 24-bit truecolour value.
    Rgb(u8, u8, u8),
}
