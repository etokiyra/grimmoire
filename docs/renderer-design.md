# Renderer design

This document records the renderer architecture, the backend evaluation that led
to it, the constraints of rendering inside a terminal, and the extension points
left for later. It describes what is implemented today, not aspirations.

## 1. The constraint that shapes everything

A terminal emulator renders a **grid of character cells**, each with one glyph,
a foreground colour, and a background colour. A program does not address pixels
unless the terminal implements a graphics protocol, and even then the program is
placing an image, not drawing into the pixel buffer.

So "custom renderer" here means: a program that owns its own drawing model,
layout, frame composition, and animation, and emits the cell grid efficiently —
not a program that draws arbitrary pixels. Everything below follows from that.

Three consequences are worth stating plainly:

- **Resolution is sub-cell at best.** Characters such as half-blocks and Braille
  can approximate higher resolution, but their appearance depends on the font.
  Grimmoire stays ASCII-first rather than depending on that.
- **The cell grid can desynchronise** if a glyph is not exactly one column wide
  (combining marks, CJK, emoji). The renderer must enforce a width policy.
- **Output cost is real.** Rewriting a large screen every frame is wasteful, so
  the presenter diffs against the previous frame.

## 2. Backend evaluation

### Terminal libraries

| Option | Capability | Compatibility | Complexity | Maintenance | Verdict |
| --- | --- | --- | --- | --- | --- |
| `crossterm` | Raw mode, alternate screen, resize + key events with timeouts, ANSI colour, cursor control | Unix **and** Windows | Moderate; small, well-documented API | Actively maintained | **Chosen** |
| `termion` | Similar lifecycle and input primitives | Unix only | Low | Less active | Rejected (no Windows, less active) |
| `ratatui` | Full TUI framework: its own buffer, layouts, widgets, backends | Cross-platform via backends | High, and it **owns the drawing model** | Very active | Rejected (contradicts a custom renderer) |
| `termwiz` | Terminal toolkit from the WezTerm project | Unix + Windows | High; geared to its ecosystem | Active (WezTerm) | Rejected (larger than needed) |

Decision: **`crossterm` is the sole terminal library**, and the only direct
dependency besides `unicode-width`. It handles exactly the fiddly, platform
specific parts (raw mode, alternate screen, input, escape sequences) while
leaving the drawing model to the project. `ratatui` in particular was rejected on
purpose: adopting it would mean Grimmoire does not have its own renderer.

### Graphics protocols

Investigated, from the projects' own documentation:

| Protocol | Mechanism | Support | Notes |
| --- | --- | --- | --- |
| Kitty graphics | APC `ESC _G<control>;<base64><ESC \`; PNG/RGB/Zlib; Unicode placeholders; animation frames; z-index | kitty, Ghostty, WezTerm, Konsole (partial) | Rich and documented. Terminal-implemented features vary. Unknown terminals ignore APC, which is comparatively safe. |
| iTerm2 inline images | OSC `1337;File=...` with base64 payload | iTerm2, WezTerm, some others | Simple, but terminal-specific and without layout metadata. |
| DEC sixel | DCS `ESC P ... q`, per-pixel runs | xterm (`-ti`), mlterm, foot, WezTerm, Contour, iTerm2, Windows Terminal 1.22+ | Widest reach of the true graphics protocols, but **not** supported by kitty, and image placement/scroll behaviour is awkward. |
| Half/quadrant blocks, Braille | Ordinary text glyphs + fg/bg colour | Universal | Sub-cell illusion only; font-dependent. Grimmoire's ASCII-first art already covers this space. |

Ecosystem evidence: the `ratatui-image` crate unifies sixel, Kitty, and iTerm2
with a half-block fallback, and it must (a) guess the protocol from environment
variables or *query* the terminal, (b) query the font's pixel size, and (c) mark
image regions so the TUI does not overwrite them. That is the shape of the work
any graphics support would require.

**Decision: do not implement a graphics protocol in this phase.** Rationale:

1. Grimmoire is ASCII-first; its identity does not require images.
2. Support is fragmented, and multiplexers (tmux, zellij) interfere with these
   protocols, so a fallback would be needed anyway.
3. Correct placement needs font metrics obtained by terminal queries.
4. No current game requirement needs pixels; the cost/benefit is poor.

**Extension point.** The capability boundary (§4) already carries a
`GraphicsSupport` value detected from environment hints and reported in the
diagnostics overlay. It is **informational only**: no rendering path consumes it.
If a future phase justifies graphics, the work is to add a renderer backend that
emits the protocol and to teach the presenter to skip cells covered by an image.
The probe is deliberately best-effort (environment hints, not terminal queries);
making it authoritative is part of that future work.

## 3. Architecture

```
+---------------------------------------------------------------+
| app::App        clock, screen stack, transition, frame buffer  |
+---------------+-------------------------------+---------------+
| ui::*         screens, layout, theme, art     | terminal::*   |
|  Screen trait, Showcase, HelpOverlay,         |  Terminal     |
|  Layout, Palette, transition curtain          |  Presenter    |
|                                               |  Capabilities |
+-----------------------------------------------+---------------+
| render::*   Canvas, CanvasView, Cell, Style, Color, Rect, glyph|
+---------------------------------------------------------------+
| anim::*   Animator, Animation, Easing                         |
| input::*  Key, KeyCode, Event   (terminal-independent)        |
+---------------------------------------------------------------+
```

Dependencies point downward. `render` and `anim` have no terminal dependency at
all; `terminal` is the only module that touches the real terminal; `ui` composes
frames and never performs I/O.

### Rendering core (`src/render/`)

- `Cell { ch, style }` — one glyph plus its style. `Cell::BLANK` is a styled space.
- `Style { fg, bg, bold, dim, italic, underline }` and `Color` (16 ANSI names,
  256-colour index, 24-bit RGB) — backend-independent.
- `Rect` — layout intent only: `contains`, `intersection`, `inset`, `centered_x/y`,
  `is_empty`. All arithmetic saturates rather than panicking.
- `Canvas` — a `Vec<Cell>` grid with clipping drawing primitives:
  `put_char`, `put_str`, `put_str_centered`, `fill`, `fill_style`, `clear_with`,
  `draw_border`, `draw_titled_border`, plus composition (`crop`, `blit`) and
  resizing (`resize`, `copy_from`).
- `CanvasView` — a clipped, local-coordinate window onto a `Canvas`
  (`Canvas::view(area)`). This is the mechanism for independent UI regions: each
  panel draws in local coordinates and cannot write outside its rectangle, even
  nested (`CanvasView::sub`).
- `glyph` — the text width policy (see §5).

**Composition and layering.** There is no separate layer type. Layering is
explicit paint order over a shared frame:

1. The application clears the frame to the palette backdrop.
2. The screen stack composes **bottom to top**; later screens draw over earlier
   ones.
3. Off-screen composition uses `Canvas::crop` / `Canvas::blit`, where a later
   blit wins on overlap.

This keeps the API small while giving deterministic ordering and explicit overlap
semantics, and it is covered by tests (`blit_later_wins`,
`compose_runs_bottom_to_top`, the overlay tests).

### Presentation (`src/terminal/presenter.rs`)

`Presenter` owns the previous frame and writes only changed cells, grouped into
contiguous runs and positioned with absolute cursor moves. It writes to any
`Write`, so its byte output is asserted in tests without a terminal.

Measured on a 120x40 frame (4800 cells), from `examples/diff_stats.rs` during
development:

| Frame | Cells written | Runs | Bytes |
| --- | --- | --- | --- |
| First (full redraw) | 4800 | 40 | 5111 |
| One cell changed | 1 | 1 | 39 |
| Unchanged | 0 | 0 | 16 |

The "unchanged" cost is only the attribute resets emitted around an otherwise
empty frame. These numbers are from the presenter's own statistics, not a
wall-clock benchmark, and they are the reason the frame buffer is kept across
frames.

### Animation (`src/anim/`)

Animations are functions of **elapsed time**, not frame counts. `Animator<K>`
owns one `Duration` clock that the application advances with `tick(delta)`;
`Animation` interpolates between two values over a duration with optional delay
and easing. This makes progress deterministic and testable — tests advance the
clock by exact durations and assert values, completion, and cancellation, with no
sleeping and no terminal.

The application caps a single step (`MAX_DELTA`) so a stall cannot make an
animation jump, and pauses motion when nothing is animating.

### Screens (`src/ui/screen.rs`)

`Screen` handles keys, advances its own animations, and composes itself.
`ScreenStack` routes input to the **active** screen only and composes **all**
screens bottom to top, so an overlay draws over the screen beneath it. Screens
never mutate the stack directly: `handle_key` returns a `Command`
(`None`/`Quit`/`Push`/`Pop`) and the stack applies it, which keeps control flow
explicit and borrow-checker friendly.

Transitions live above the stack (`src/ui/transition.rs`): after a push or pop the
application starts a curtain animation and overlays it on the composed frame.
Screens therefore know nothing about transitions.

### Capabilities (`src/terminal/capabilities.rs`)

`Capabilities` reports colour support (16/256/truecolour, derived from
`crossterm`'s colour-count query) and a best-effort graphics hint. The colour
result selects the palette (`Palette::BASIC` or `Palette::EXTENDED`). The
graphics result is reported, not used (§2).

## 4. Boundary rules

- `render` and `anim` must not depend on `terminal` or on a terminal library.
- Only `terminal` may perform terminal I/O or emit escape sequences.
- `ui` may read `FrameContext` (capabilities, palette, present statistics, frame
  time) but performs no I/O and no game rules.
- `input` types are terminal-independent; the backend translates library events
  into them.
- Screens never write to the terminal; they compose into a `Canvas`.

## 5. Constraints and policies

### Unicode width: ASCII-first, enforced

`render::glyph` classifies each `char` with `unicode-width` (UAX #11):

- width **1** — drawable;
- width **0** (combining marks, joiners) and control characters — dropped;
- width **2+** (CJK, emoji) — dropped.

`put_char` / `put_str` never place an undrawable glyph, and the presenter also
substitutes a space for any non-single-width glyph it finds, so the cursor can
never advance by more than one column per cell. Dropping rather than substituting
a placeholder keeps the columns of surviving glyphs exactly as callers expect.

This is a documented limitation: the cell model has no continuation cell, so
double-width glyphs are not rendered. Supporting them properly (a continuation
flag, and clearing both halves when either is overwritten) is a clean future
extension, not a rewrite.

### Frame boundaries and scrolling

Every run is preceded by an absolute cursor move, and the presenter never relies
on relative cursor advancement across a run boundary. Writing the last cell of
the last row only arms the terminal's pending-wrap flag; the next cursor move
clears it, so no scrolling occurs. A resize (or an explicit `invalidate`) clears
the screen and starts a fresh diff baseline, so content from a larger previous
frame cannot linger.

### Resize

Layout is recomputed from the frame bounds every frame (`Layout::compute`), so
resizing is handled implicitly. The application detects a size change, resizes
the buffer, notifies screens, and invalidates the presenter for a full redraw.
Three modes exist: `Wide` (two columns), `Stacked` (one column), and `Minimal`
(too small for the interface). Below workable heights, regions are dropped rather
than shrunk into unusable slivers.

## 6. Measured and verified behaviour

- **Interactive**, in a real pseudo-terminal (`100x30`, resized to `60x18`
  mid-run): enters/leaves the alternate screen, hides/restores the cursor, clears
  on resize, draws the identity, opens the help overlay, switches scene, pauses,
  and quits with exit status 0.
- **Idle CPU**, measured over a 2-second window: **0.000 CPU-seconds** while
  paused versus **0.020 CPU-seconds** (about 1% of one core, ~30 fps) while
  animating.
- **Diffing**: see the table in §3.

Not verified / not claimed: restoration under `SIGKILL` or power loss cannot be
guaranteed by any program; graphics protocols were not exercised because no
supporting terminal was available in the development environment, and none is
implemented.

## 7. Non-goals

- No terminal emulator, and no pixel-level drawing without a graphics protocol.
- No widget toolkit or retained scene graph.
- No speculative graphics support, diffed-damage-region tracking, or wide-glyph
  continuation cells until a phase needs them.
