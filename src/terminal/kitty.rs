//! The Kitty graphics protocol.
//!
//! This module is a pure encoder: it turns intent ("transmit this PNG", "show
//! image 3 in this rectangle", "forget image 3") into the exact bytes Kitty
//! expects. It performs no I/O and holds no state, so the protocol can be
//! verified in tests without a terminal.
//!
//! # Choices
//!
//! * **Data is transmitted inline** (`t=d`) as base64 PNG (`f=100`). Kitty also
//!   accepts a *file path* (`t=f`), which is cheaper on the wire, but it assumes
//!   the client and terminal share a filesystem — false over SSH and in
//!   sandboxes — and a failed file read becomes an unread response. Inline PNG
//!   bytes have no such assumption.
//! * **The PNG is never decoded.** Its bytes are copied into the escape sequence
//!   exactly as they are, so no image codec is needed anywhere in the project.
//! * **Responses are suppressed** (`q=2`). Responses arrive on the input stream,
//!   and this application reads input as key events; an unread acknowledgement
//!   would surface as garbage keystrokes.
//! * **Placements are identified** (`i` + `p`). Re-sending the same pair
//!   replaces the placement in place, so moving or resizing an image needs no
//!   delete and causes no flicker.
//! * **Placement happens at the cursor.** Kitty draws an image at the current
//!   cursor cell, so every placement moves the cursor there first and then asks
//!   for the cursor to stay put (`C=1`).
//!
//! Reference: <https://sw.kovidgoyal.net/kitty/graphics-protocol/>

use std::io::{self, Write};

use crossterm::cursor::MoveTo;
use crossterm::queue;

use crate::render::Rect;

/// The Application Programming Command introducer for graphics.
pub const APC: &str = "\x1b_G";
/// The String Terminator that ends an APC.
const ST: &[u8] = b"\x1b\\";
/// The largest base64 payload Kitty accepts in one chunk.
const MAX_CHUNK: usize = 4096;

/// The graphics protocol version this encoder speaks, for diagnostics.
pub const PROTOCOL: &str = "kitty graphics (PNG, inline)";

/// Suppress every response: responses are delivered on the input stream.
const QUIET: &str = "q=2";

/// Transmits PNG bytes, defining image `id`. The image is not displayed yet.
///
/// The payload is chunked and the PNG bytes are passed through untouched.
pub fn transmit_png<W: Write>(out: &mut W, id: u32, png: &[u8]) -> io::Result<u32> {
    let encoded = base64(png);
    let chunks: Vec<&str> = if encoded.is_empty() {
        vec![""]
    } else {
        let bytes = encoded.as_bytes();
        bytes
            .chunks(MAX_CHUNK)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect()
    };

    let mut bytes_written = 0u32;
    for (index, chunk) in chunks.iter().enumerate() {
        let last = index + 1 == chunks.len();
        let more = if last { 0 } else { 1 };
        if index == 0 {
            write!(out, "{APC}a=t,f=100,{QUIET},i={id},m={more};")?;
        } else {
            write!(out, "{APC}m={more};")?;
        }
        out.write_all(chunk.as_bytes())?;
        out.write_all(ST)?;
        bytes_written = bytes_written.saturating_add(chunk.len() as u32);
    }
    Ok(bytes_written)
}

/// Displays image `id` at `area`, a rectangle of cells.
///
/// Kitty draws an image at the **current cursor cell**, so the cursor is moved to
/// the rectangle's top-left corner first and then told to stay there (`C=1`).
/// Both `c` (columns) and `r` (rows) are given so the rectangle is exactly
/// predictable: Kitty letterboxes an image to preserve its aspect ratio rather
/// than distorting it. `placement` identifies the placement so a later call
/// replaces it in place, without flicker.
pub fn place<W: Write>(out: &mut W, id: u32, placement: u32, area: Rect) -> io::Result<()> {
    queue!(out, MoveTo(area.x, area.y))?;
    write!(
        out,
        "{APC}a=p,{QUIET},i={id},p={placement},c={},r={},C=1,z=0;\x1b\\",
        area.width, area.height
    )
}

/// Removes one placement, keeping the image data for later reuse.
pub fn delete_placement<W: Write>(out: &mut W, id: u32, placement: u32) -> io::Result<()> {
    write!(out, "{APC}a=d,d=p,{QUIET},i={id},p={placement};\x1b\\")
}

/// Removes every image and placement belonging to this application.
///
/// Used on shutdown and before a full redraw, because the clear-screen escape
/// code is specified to clear images as well, which invalidates our view of what
/// the terminal is holding.
pub fn delete_all<W: Write>(out: &mut W) -> io::Result<()> {
    write!(out, "{APC}a=d,d=A,{QUIET};\x1b\\")
}

/// Standard base64 with padding.
///
/// The protocol carries payloads base64-encoded to keep control bytes out of the
/// escape sequence. This is the only reason the project needs an encoder, so it
/// is deliberately written here rather than taken as a dependency.
#[must_use]
pub fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from);
        let b2 = chunk.get(2).copied().map(u32::from);
        let triple = (b0 << 16) | (b1.unwrap_or(0) << 8) | b2.unwrap_or(0);

        out.push(ALPHABET[(triple >> 18) as usize & 0x3f] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3f] as char);
        if b1.is_some() {
            out.push(ALPHABET[(triple >> 6) as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
        if b2.is_some() {
            out.push(ALPHABET[triple as usize & 0x3f] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{MAX_CHUNK, base64, delete_all, delete_placement, place, transmit_png};
    use crate::render::Rect;

    fn text(bytes: &[u8]) -> String {
        String::from_utf8_lossy(bytes).into_owned()
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!("", base64(b""));
        assert_eq!("Zg==", base64(b"f"));
        assert_eq!("Zm8=", base64(b"fo"));
        assert_eq!("Zm9v", base64(b"foo"));
        assert_eq!("Zm9vYmFy", base64(b"foobar"));
        assert_eq!("AAEC", base64(&[0, 1, 2]));
    }

    #[test]
    fn transmission_declares_png_and_suppresses_responses() {
        let mut out = Vec::new();
        transmit_png(&mut out, 7, b"png-bytes").expect("transmit");
        let sequence = text(&out);

        assert!(sequence.starts_with("\x1b_Ga=t,f=100,q=2,i=7,m=0;"));
        assert!(sequence.ends_with("\x1b\\"));
        assert!(sequence.contains(&base64(b"png-bytes")));
    }

    #[test]
    fn an_empty_image_still_produces_a_valid_sequence() {
        let mut out = Vec::new();
        transmit_png(&mut out, 1, b"").expect("transmit");
        assert_eq!("\x1b_Ga=t,f=100,q=2,i=1,m=0;\x1b\\", text(&out));
    }

    #[test]
    fn large_payloads_are_chunked_with_a_final_terminator() {
        // Enough bytes that the base64 form spans several chunks.
        let payload = vec![0xab; MAX_CHUNK * 3];
        let mut out = Vec::new();
        transmit_png(&mut out, 3, &payload).expect("transmit");
        let sequence = text(&out);

        let chunks = sequence.matches("\x1b_G").count();
        assert!(chunks > 2, "expected chunking, got {chunks} chunk(s)");
        // Exactly one chunk is marked final.
        assert_eq!(1, sequence.matches("m=0;").count());
        assert_eq!(chunks - 1, sequence.matches("m=1;").count());
        // Continuation chunks carry no image id or format, only the more flag.
        assert!(sequence.contains("\x1b_Gm=1;"));
    }

    #[test]
    fn a_chunked_payload_round_trips_to_the_original_bytes() {
        let payload: Vec<u8> = (0..9000u32).map(|i| (i % 251) as u8).collect();
        let mut out = Vec::new();
        transmit_png(&mut out, 4, &payload).expect("transmit");

        // Reassemble: strip every APC wrapper and decode the payloads in order.
        let sequence = String::from_utf8_lossy(&out).into_owned();
        let mut encoded = String::new();
        for part in sequence.split("\x1b_G").skip(1) {
            let body = part.split("\x1b\\").next().unwrap_or("");
            let payload = body.split_once(';').map(|(_, p)| p).unwrap_or("");
            encoded.push_str(payload);
        }
        assert_eq!(base64(&payload), encoded);
    }

    #[test]
    fn placement_moves_the_cursor_to_the_rectangle_first() {
        let mut out = Vec::new();
        place(&mut out, 5, 2, Rect::new(3, 4, 24, 11)).expect("place");
        // Kitty draws at the cursor cell, so the move must come first (1-based).
        assert!(
            text(&out).starts_with("\x1b[5;4H"),
            "expected a cursor move first, got {:?}",
            text(&out)
        );
        assert_eq!(
            "\x1b[5;4H\x1b_Ga=p,q=2,i=5,p=2,c=24,r=11,C=1,z=0;\x1b\\",
            text(&out)
        );
    }

    #[test]
    fn deletion_forms_are_distinct() {
        let mut placement = Vec::new();
        delete_placement(&mut placement, 9, 3).expect("delete");
        assert_eq!("\x1b_Ga=d,d=p,q=2,i=9,p=3;\x1b\\", text(&placement));

        let mut all = Vec::new();
        delete_all(&mut all).expect("delete");
        assert_eq!("\x1b_Ga=d,d=A,q=2;\x1b\\", text(&all));
    }

    #[test]
    fn every_sequence_is_a_well_formed_apc() {
        let mut out = Vec::new();
        transmit_png(&mut out, 1, b"data").expect("transmit");
        place(&mut out, 1, 1, Rect::new(0, 0, 4, 2)).expect("place");
        delete_all(&mut out).expect("delete");

        let sequence = text(&out);
        for command in sequence.split("\x1b_G").skip(1) {
            assert!(command.contains('\x1b'), "command is not terminated");
        }
        assert_eq!(3, sequence.matches("\x1b_G").count());
    }
}
