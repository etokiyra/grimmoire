//! A DEC sixel encoder.
//!
//! Sixel is the graphics protocol used by this project's chosen terminal for
//! image-backed content (`foot`, and many others). This module is a pure
//! function from an [`Image`] to the exact bytes a terminal expects; it performs
//! no I/O and has no dependency, so it is fully testable without a terminal.
//!
//! # Encoding choices
//!
//! * The palette is a **fixed 6×6×6 RGB cube** (216 entries), so an image can
//!   never exceed the protocol's 256-colour limit and no adaptive quantiser is
//!   needed. Only colours actually present are defined.
//! * Each six-pixel band is emitted per colour with run-length encoding, and
//!   empty columns are skipped, which keeps large dark areas cheap.
//! * Raster attributes are always emitted so the terminal knows the exact pixel
//!   size, which the caller has already matched to a rectangle of character
//!   cells.
//!
//! The visible cost of the fixed cube is banding in very smooth gradients. That
//! is an acceptable trade for a proof of concept, and the place to improve if
//! image-backed art becomes central.

use crate::render::{Image, Rgb};

/// The device-control-string introducer.
const DCS: &[u8] = b"\x1bP";
/// The string terminator.
const ST: &[u8] = b"\x1b\\";
/// The character offset for a sixel data byte (`?` encodes zero).
const DATA_BASE: u8 = 63;
/// Sixels are six pixels tall.
const BAND_HEIGHT: u32 = 6;
/// Levels per channel in the fixed palette cube.
const CUBE_LEVELS: u32 = 6;
/// Cube entries: 6 × 6 × 6.
const CUBE_SIZE: usize = (CUBE_LEVELS * CUBE_LEVELS * CUBE_LEVELS) as usize;

/// Encodes `image` as a complete sixel sequence.
#[must_use]
pub fn encode(image: &Image) -> Vec<u8> {
    let width = image.width();
    let height = image.height();

    let mut out: Vec<u8> = Vec::with_capacity((width as usize) * 8 + 64);
    out.extend_from_slice(DCS);
    // P1;P2;P3 = 0;0;0, then raster attributes: "Pan;Pad;Ph;Pv.
    out.extend_from_slice(b"0;0;0q\"1;1;");
    out.extend_from_slice(width.to_string().as_bytes());
    out.push(b';');
    out.extend_from_slice(height.to_string().as_bytes());

    if width == 0 || height == 0 {
        out.extend_from_slice(ST);
        return out;
    }

    let (palette, slots) = quantise(image);
    write_palette(&mut out, &palette);

    let width_usize = width as usize;
    let mut band_colours: Vec<bool> = vec![false; palette.len()];
    let mut masks: Vec<u8> = vec![0; width_usize];

    let mut band_start = 0;
    while band_start < height {
        // Which palette slots appear in this band, in slot order.
        band_colours.iter_mut().for_each(|used| *used = false);
        for offset in 0..BAND_HEIGHT {
            let y = band_start + offset;
            if y >= height {
                break;
            }
            let row = (y as usize) * width_usize;
            for x in 0..width_usize {
                let slot = slots[row + x] as usize;
                band_colours[slot] = true;
            }
        }

        for (slot, used) in band_colours.iter().enumerate() {
            if !used {
                continue;
            }
            out.push(b'#');
            out.extend_from_slice(slot.to_string().as_bytes());
            let last = fill_masks(
                &slots,
                width_usize,
                height,
                band_start,
                slot as u16,
                &mut masks,
            );
            write_run_lengths(&mut out, &masks[..=last]);
            out.push(b'$');
        }
        out.push(b'-');
        band_start += BAND_HEIGHT;
    }

    out.extend_from_slice(ST);
    out
}

/// Builds the palette and the per-pixel slot map.
fn quantise(image: &Image) -> (Vec<Rgb>, Vec<u16>) {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut slot_of = [u16::MAX; CUBE_SIZE];
    let mut palette: Vec<Rgb> = Vec::new();
    let mut slots: Vec<u16> = Vec::with_capacity(width * height);

    for y in 0..height {
        for x in 0..width {
            let index = cube_index(image.pixel(x as u32, y as u32));
            let slot = match slot_of[index] {
                u16::MAX => {
                    let slot = palette.len() as u16;
                    slot_of[index] = slot;
                    palette.push(cube_colour(index));
                    slot
                }
                existing => existing,
            };
            slots.push(slot);
        }
    }

    (palette, slots)
}

/// Fills `masks` with the six-pixel column masks for `slot` in the band, and
/// returns the index of the last non-empty column.
fn fill_masks(
    slots: &[u16],
    width: usize,
    height: u32,
    band_start: u32,
    slot: u16,
    masks: &mut [u8],
) -> usize {
    let mut last = 0usize;
    for (x, mask) in masks.iter_mut().enumerate() {
        let mut value = 0u8;
        for offset in 0..BAND_HEIGHT {
            let y = band_start + offset;
            if y >= height {
                break;
            }
            if slots[(y as usize) * width + x] == slot {
                value |= 1 << offset;
            }
        }
        *mask = value;
        if value != 0 {
            last = x;
        }
    }
    last
}

/// Writes a run-length-encoded run of sixel data bytes.
fn write_run_lengths(out: &mut Vec<u8>, masks: &[u8]) {
    let mut index = 0usize;
    while index < masks.len() {
        let value = masks[index];
        let mut run = 1usize;
        while index + run < masks.len() && masks[index + run] == value {
            run += 1;
        }
        let ch = DATA_BASE + value;
        if run >= 4 {
            out.push(b'!');
            out.extend_from_slice(run.to_string().as_bytes());
            out.push(ch);
        } else {
            for _ in 0..run {
                out.push(ch);
            }
        }
        index += run;
    }
}

/// Writes the palette definitions, as percentages.
fn write_palette(out: &mut Vec<u8>, palette: &[Rgb]) {
    for (slot, colour) in palette.iter().enumerate() {
        out.push(b'#');
        out.extend_from_slice(slot.to_string().as_bytes());
        out.extend_from_slice(b";2;");
        out.extend_from_slice(percent(colour.r).to_string().as_bytes());
        out.push(b';');
        out.extend_from_slice(percent(colour.g).to_string().as_bytes());
        out.push(b';');
        out.extend_from_slice(percent(colour.b).to_string().as_bytes());
    }
}

/// Maps a channel to the protocol's 0–100 percentage range.
fn percent(channel: u8) -> u32 {
    (u32::from(channel) * 100 + 127) / 255
}

/// The fixed-cube index for a colour.
fn cube_index(colour: Rgb) -> usize {
    let level = |channel: u8| ((u32::from(channel) * (CUBE_LEVELS - 1) + 127) / 255) as usize;
    let r = level(colour.r);
    let g = level(colour.g);
    let b = level(colour.b);
    r * CUBE_LEVELS as usize * CUBE_LEVELS as usize + g * CUBE_LEVELS as usize + b
}

/// The representative colour for a fixed-cube index.
fn cube_colour(index: usize) -> Rgb {
    let levels = CUBE_LEVELS as usize;
    let step = 255 / (CUBE_LEVELS - 1);
    let r = (index / (levels * levels)) % levels;
    let g = (index / levels) % levels;
    let b = index % levels;
    Rgb::new(
        (r as u32 * step) as u8,
        (g as u32 * step) as u8,
        (b as u32 * step) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::{DATA_BASE, ST, encode};
    use crate::render::{Image, Rgb};

    fn starts_with_dcs(bytes: &[u8]) -> bool {
        bytes.starts_with(b"\x1bP")
    }

    fn ends_with_st(bytes: &[u8]) -> bool {
        bytes.ends_with(ST)
    }

    fn palette_entries(bytes: &[u8]) -> usize {
        String::from_utf8_lossy(bytes)
            .split('#')
            .filter(|part| part.contains(";2;"))
            .count()
    }

    #[test]
    fn the_sequence_is_well_formed() {
        let bytes = encode(&Image::solid(8, 8, Rgb::new(10, 20, 30)));
        assert!(starts_with_dcs(&bytes));
        assert!(ends_with_st(&bytes));
        assert!(String::from_utf8_lossy(&bytes).contains("\"1;1;8;8"));
    }

    #[test]
    fn an_empty_image_still_produces_a_valid_sequence() {
        let bytes = encode(&Image::new(0, 0));
        assert!(starts_with_dcs(&bytes));
        assert!(ends_with_st(&bytes));
    }

    #[test]
    fn the_palette_never_exceeds_the_protocol_limit() {
        // A maximally varied image: every cube entry appears at least once.
        let mut image = Image::new(216, 1);
        for i in 0..216u32 {
            let step = 51;
            image.set_pixel(
                i,
                0,
                Rgb::new(
                    ((i / 36) % 6 * step) as u8,
                    ((i / 6) % 6 * step) as u8,
                    (i % 6 * step) as u8,
                ),
            );
        }
        let bytes = encode(&image);
        assert!(palette_entries(&bytes) <= 256);
        assert_eq!(216, palette_entries(&bytes));
    }

    #[test]
    fn only_colours_actually_used_are_defined() {
        let bytes = encode(&Image::solid(4, 4, Rgb::new(255, 0, 0)));
        assert_eq!(1, palette_entries(&bytes));
    }

    #[test]
    fn solid_images_use_run_length_encoding() {
        let bytes = encode(&Image::solid(64, 6, Rgb::new(255, 255, 255)));
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.contains('!'), "expected an RLE marker: {text:?}");
    }

    #[test]
    fn data_bytes_stay_inside_the_sixel_alphabet() {
        let bytes = encode(&Image::vertical_gradient(
            32,
            12,
            Rgb::new(0, 0, 0),
            Rgb::new(255, 255, 255),
        ));
        for &byte in &bytes {
            let ok = byte == 0x1b
                || byte == b'\\'
                || byte == b'"'
                || byte == b'#'
                || byte == b';'
                || byte == b'!'
                || byte == b'$'
                || byte == b'-'
                || byte == b'P'
                || byte.is_ascii_digit()
                || (DATA_BASE..=DATA_BASE + 63).contains(&byte);
            assert!(ok, "unexpected byte {byte:#x}");
        }
    }

    #[test]
    fn encoding_is_deterministic() {
        let image = Image::radial(24, 24, Rgb::new(255, 200, 80), Rgb::new(10, 8, 20));
        assert_eq!(encode(&image), encode(&image));
    }

    #[test]
    fn bands_are_separated_by_newline_markers() {
        // 13 rows => three bands => at least two band separators.
        let bytes = encode(&Image::solid(4, 13, Rgb::new(255, 255, 255)));
        let separators = bytes.iter().filter(|byte| **byte == b'-').count();
        assert!(
            separators >= 2,
            "expected band separators, got {separators}"
        );
    }
}
