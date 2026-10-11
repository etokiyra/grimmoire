//! The Grimmoire colour palette.
//!
//! Names are thematic so screen code reads like art direction rather than raw
//! colour values. Values are chosen to stay legible on both light and dark
//! terminals by leaning on bright ANSI variants for anything that carries text.

use crate::render::Color;

/// Deep arcane purple used for titles and sigils.
pub const ARCANE: Color = Color::LightMagenta;
/// Warm parchment tone used for body text.
pub const PARCHMENT: Color = Color::LightYellow;
/// Ember red used for warnings and emphasis.
pub const EMBER: Color = Color::LightRed;
/// Cold aether blue used for secondary text and hints.
pub const FROST: Color = Color::LightCyan;
/// Muted grey used for de-emphasised chrome such as footers.
pub const SHADOW: Color = Color::DarkGrey;
/// Deep woodland green reserved for future environment work.
pub const MOSS: Color = Color::LightGreen;
