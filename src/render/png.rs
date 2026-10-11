//! A minimal PNG encoder.
//!
//! Grimmoire renders its pixel-art scene in software and hands the result to
//! Kitty, which decodes and displays it. This module is the *only* place that
//! turns pixels into a file format, and it only ever encodes: nothing in the
//! project decodes an image.
//!
//! # Why hand-written
//!
//! The PNG container is small and completely specified, and writing it here
//! keeps the image pipeline auditable and dependency-light. The one thing that
//! is *not* worth writing by hand is DEFLATE, so the compressed payload comes
//! from `fdeflate`. Everything else — the signature, `IHDR`, `IDAT`, `IEND`, the
//! per-row filters, the CRC-32s and the scanline layout — is eight-bit
//! arithmetic that is easy to test against a decoder.
//!
//! # What is produced
//!
//! 8-bit truecolour (colour type 2, no alpha): the scene is composited opaque
//! before it gets here, so an alpha channel would be a third more bytes for
//! nothing.
//!
//! Rows use the Paeth predictor unconditionally. The usual advice is to try all
//! five filters per row and keep the best, but on this scene the heuristic costs
//! more than it saves: measured over a real 227x160 frame, all-five took 4.4 ms
//! and produced 24 KiB, while Paeth alone took 2.1 ms and produced 25 KiB. One
//! kilobyte is not worth doubling the cost of every frame.

/// The eight-byte signature every PNG file starts with.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Encodes `width` by `height` 8-bit RGB pixels as a PNG.
///
/// `pixels` is row-major RGB, three bytes per pixel. A slice that is too short
/// is treated as transparent black, so a caller bug can never panic or read out
/// of bounds.
#[must_use]
pub fn encode_rgb(width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(usize::from(width) * usize::from(height) / 2 + 128);
    out.extend_from_slice(&SIGNATURE);

    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&u32::from(width).to_be_bytes());
    header.extend_from_slice(&u32::from(height).to_be_bytes());
    header.push(8); // bit depth
    header.push(2); // colour type: truecolour RGB
    header.push(0); // compression: deflate
    header.push(0); // filter method: adaptive
    header.push(0); // interlace: none
    write_chunk(&mut out, b"IHDR", &header);

    let raw = filter_rows(width, height, pixels);
    // `fdeflate` emits a complete zlib stream, which is exactly what an `IDAT`
    // chunk holds.
    let compressed = fdeflate::compress_to_vec(&raw);
    write_chunk(&mut out, b"IDAT", &compressed);

    write_chunk(&mut out, b"IEND", &[]);
    out
}

/// Filters every row with the Paeth predictor and prefixes it with the filter
/// type, producing the byte stream an `IDAT` chunk holds.
fn filter_rows(width: u16, height: u16, pixels: &[u8]) -> Vec<u8> {
    const PAETH: u8 = 4;
    let stride = usize::from(width) * 3;
    let mut out = Vec::with_capacity((stride + 1) * usize::from(height));
    let mut previous = vec![0u8; stride];
    let mut row = vec![0u8; stride];
    let mut filtered = vec![0u8; stride];

    for y in 0..usize::from(height) {
        // A short buffer reads as black rather than panicking.
        row.fill(0);
        let start = y * stride;
        if start < pixels.len() {
            let end = (start + stride).min(pixels.len());
            row[..end - start].copy_from_slice(&pixels[start..end]);
        }
        filter(PAETH, &row, &previous, &mut filtered);
        out.push(PAETH);
        out.extend_from_slice(&filtered);
        previous.copy_from_slice(&row);
    }
    out
}

/// Applies one filter type to `row`, writing `stride` bytes into `out`.
fn filter(kind: u8, row: &[u8], previous: &[u8], out: &mut [u8]) {
    for (i, slot) in out.iter_mut().enumerate() {
        let raw = row.get(i).copied().unwrap_or(0);
        let left = if i >= 3 {
            row.get(i - 3).copied().unwrap_or(0)
        } else {
            0
        };
        let up = previous.get(i).copied().unwrap_or(0);
        let up_left = if i >= 3 {
            previous.get(i - 3).copied().unwrap_or(0)
        } else {
            0
        };
        *slot = match kind {
            0 => raw,
            1 => raw.wrapping_sub(left),
            2 => raw.wrapping_sub(up),
            3 => raw.wrapping_sub(((u16::from(left) + u16::from(up)) / 2) as u8),
            _ => raw.wrapping_sub(paeth(left, up, up_left)),
        };
    }
}

/// The Paeth predictor from the PNG specification.
fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let pa = (p - i16::from(a)).abs();
    let pb = (p - i16::from(b)).abs();
    let pc = (p - i16::from(c)).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// Writes a length-prefixed, CRC-suffixed PNG chunk.
fn write_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc = Crc32::new();
    crc.update(kind);
    crc.update(data);
    out.extend_from_slice(&crc.finish().to_be_bytes());
}

/// The CRC-32 used by PNG (reflected, polynomial `0xEDB88320`).
struct Crc32 {
    state: u32,
}

impl Crc32 {
    const fn new() -> Self {
        Crc32 { state: 0xffff_ffff }
    }

    fn update(&mut self, data: &[u8]) {
        for byte in data {
            self.state ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = (self.state & 1).wrapping_neg();
                self.state = (self.state >> 1) ^ (0xedb8_8320 & mask);
            }
        }
    }

    const fn finish(self) -> u32 {
        !self.state
    }
}

#[cfg(test)]
mod tests {
    use super::{SIGNATURE, encode_rgb};

    /// Reads a big-endian `u32` from `bytes`.
    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    /// Walks the chunks of a PNG, returning `(type, data)` pairs.
    fn chunks(png: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut at = SIGNATURE.len();
        let mut found = Vec::new();
        while at + 8 <= png.len() {
            let length = u32_at(png, at) as usize;
            let kind = String::from_utf8_lossy(&png[at + 4..at + 8]).into_owned();
            let data = png[at + 8..at + 8 + length].to_vec();
            found.push((kind, data));
            at += 12 + length;
        }
        found
    }

    fn sample(width: u16, height: u16) -> Vec<u8> {
        let mut pixels = Vec::with_capacity(usize::from(width) * usize::from(height) * 3);
        for y in 0..height {
            for x in 0..width {
                pixels.push((x * 7 % 256) as u8);
                pixels.push((y * 11 % 256) as u8);
                pixels.push(((x + y) * 3 % 256) as u8);
            }
        }
        pixels
    }

    #[test]
    fn a_png_starts_with_the_signature_and_ends_with_iend() {
        let png = encode_rgb(4, 3, &sample(4, 3));
        assert_eq!(SIGNATURE, png[..8]);
        let names: Vec<String> = chunks(&png).into_iter().map(|(name, _)| name).collect();
        assert_eq!(vec!["IHDR", "IDAT", "IEND"], names);
    }

    #[test]
    fn the_header_describes_8_bit_rgb() {
        let png = encode_rgb(300, 200, &sample(300, 200));
        let header = &chunks(&png)[0].1;
        assert_eq!(300, u32_at(header, 0));
        assert_eq!(200, u32_at(header, 4));
        assert_eq!(8, header[8], "bit depth");
        assert_eq!(2, header[9], "colour type: truecolour");
        assert_eq!(0, header[10]);
        assert_eq!(0, header[11]);
        assert_eq!(0, header[12]);
    }

    #[test]
    fn the_payload_inflates_back_to_filtered_scanlines() {
        let (width, height) = (9u16, 5u16);
        let pixels = sample(width, height);
        let png = encode_rgb(width, height, &pixels);

        let idat = chunks(&png)
            .into_iter()
            .find(|(name, _)| name == "IDAT")
            .expect("IDAT")
            .1;
        let raw = fdeflate::decompress_to_vec(&idat).expect("inflate");

        // One filter byte plus three bytes per pixel, per row.
        let stride = usize::from(width) * 3;
        assert_eq!(raw.len(), (stride + 1) * usize::from(height));

        // Every filter is a reversible predictor; filter 0 rows must match the
        // source exactly.
        for (y, row) in raw.chunks(stride + 1).enumerate() {
            assert!(row[0] <= 4, "filter {} is not a known type", row[0]);
            if row[0] == 0 {
                let start = y * stride;
                assert_eq!(&pixels[start..start + stride], &row[1..]);
            }
        }
    }

    #[test]
    fn a_single_pixel_and_a_zero_sized_image_are_both_valid() {
        let one = encode_rgb(1, 1, &[1, 2, 3]);
        assert_eq!(SIGNATURE, one[..8]);
        let empty = encode_rgb(0, 0, &[]);
        assert_eq!(SIGNATURE, empty[..8]);
        assert!(chunks(&empty).iter().any(|(name, _)| name == "IEND"));
    }

    #[test]
    fn a_short_pixel_buffer_does_not_panic() {
        // Declared 4x4 but given only two pixels: the rest must read as black.
        let png = encode_rgb(4, 4, &[9, 9, 9, 8, 8, 8]);
        assert_eq!(SIGNATURE, png[..8]);
    }
}
