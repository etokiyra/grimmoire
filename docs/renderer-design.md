# Renderer design

This document records Grimmoire's rendering architecture, the evidence behind it,
the constraints of drawing inside a terminal, and the extension points left open.
It describes what is implemented today, not aspirations.

It was substantially revised after a rendering-architecture review; §2 records
what the audit actually found, including a defect that had been shipped.

## 1. The constraint that shapes everything

A terminal emulator renders a **grid of character cells**, each with one glyph,
a foreground colour, and a background colour. A program does not address pixels
unless the terminal implements a graphics protocol, and even then it is placing
an image, not drawing into a pixel buffer.

So "custom renderer" means: a program that owns its drawing model, layout, frame
composition, and animation, and emits the cell grid efficiently — with image
support layered on only where the terminal can actually do it. Three consequences
follow:

- **Resolution is sub-cell at best by default.** Block and box glyphs (`█`, `░`)
  are one column wide, so they *can* be used as colour swatches and shading —
  confirmed against `unicode-width`, which treats East Asian Ambiguous
  characters as one column. Photographic imagery needs a graphics protocol.
- **The cell grid desynchronises** if a glyph is not exactly one column wide. The
  renderer enforces a width policy.
- **Output cost is real.** Rewriting a large screen every frame is wasteful, so
  the presenter diffs against the previous frame.

## 2. What the audit found

The pipeline was mapped end to end: `ui` composes a `Canvas` → `Presenter` diffs
it against the previous frame → `Terminal` writes escape sequences. Verified
findings:

**Good, and kept.** Cell-based composition with clipped `CanvasView` regions;
deterministic paint-order layering plus bottom-to-top screen composition; a
diffing presenter; time-based animation with an injectable clock; a screen stack;
capability detection; tests that run without a terminal.

**Defect: backgrounds leaked.** The theme had a `background` colour and the
application cleared the frame with it, but **50 call sites** drew text with a
foreground-only style. Because `Style::bg` defaulted to `Color::Default`, the
presenter emitted `ResetColor` for those cells, so the terminal's own background —
or the desktop, on a transparent terminal — showed through the game's text.

The fix is a surface model in `Canvas::set`: a cell whose style leaves the
background at `Color::Default` **inherits the background already in the grid**.
The application paints the viewport once and every later draw keeps it. This is
verified by unit tests and by sampling pixels from a real terminal (§7).

**Gaps found.** No surface hierarchy in the theme (only accents and text); no
image capability at all; no minimum-viewport policy (each screen had its own
ad-hoc "too small" fallback); the capability probe could not distinguish "not
available" from "we could not tell".

## 3. Options considered

| Option | What it buys | What it costs | Verdict |
| --- | --- | --- | --- |
| **A. Cell renderer only** | Portability, simplicity, full diffing, no capability risk | No photographic imagery; sub-cell detail limited to block glyphs | **Kept as the baseline** |
| **B. Hybrid: cells + a graphics protocol** | Real images for portraits and scenes, composable with panels and text | Needs a protocol implementation, cell metrics, image lifecycle handling, and a fallback | **Adopted, one protocol** |
| **C. GPU window (pixels)** | Conventional sprites, arbitrary effects | Abandons the terminal-native premise and its distribution: a windowing stack, font rendering, input handling, and a completely different install story | **Rejected** — no requirement needs it, and the prompt only authorised abandoning the terminal premise with evidence |

For option B the candidate protocols were Kitty graphics, iTerm2 inline images,
and DEC sixel.

| Protocol | Placement | Support | Evidence available here |
| --- | --- | --- | --- |
| Kitty | Cell-aligned, Unicode placeholders, z-index, animation frames | kitty, Ghostty, WezTerm, Konsole (partial) | **None** — no kitty-family terminal in the environment |
| iTerm2 | Cursor-anchored inline images | iTerm2, WezTerm | **None** |
| **Sixel** | Cursor-anchored, pixel-sized | `foot`, xterm (`-ti`), mlterm, Contour, WezTerm, iTerm2, Windows Terminal 1.22+, not kitty | **Verified at runtime** — `foot` 1.28 in this environment renders sixel (screenshot captured and inspected) |

**Decision: implement sixel, and only sixel.** Kitty has the better layout model,
but a protocol cannot be adopted on documentation alone — and the instructions
explicitly forbid choosing on a heuristic or a successful compilation. Sixel is
the one protocol whose behaviour could be observed here, it has the widest reach
of the true-graphics protocols, and its limitations are known and containable.

The layout cost of that choice is real and is paid explicitly: sixel has no
notion of cells, so an image must be resized to `columns × cell_width` by
`rows × cell_height` pixels, which requires the terminal to report its cell size.
When it does not, no backend is selected.

## 4. Chosen architecture

```
+---------------------------------------------------------------------+
| app::App   clock, screen stack, transition, canvas, image placements |
+------------------+-------------------------------+------------------+
| ui::*  screens, layout, theme, art, stage        | terminal::*      |
|        Screen trait -> Frame                     |   Terminal       |
|                                                  |   Presenter      |
|                                                  |   Capabilities   |
|                                                  |   ImageBackend   |
|                                                  |   sixel::encode  |
+------------------+-------------------------------+------------------+
| render::*  Canvas, CanvasView, Frame, Cell, Style, Color, Rect,     |
|            Image, glyph                                             |
+---------------------------------------------------------------------+
| anim::*   Animator, Animation, Easing                               |
| input::*  Key, KeyCode, Event   (terminal-independent)              |
+---------------------------------------------------------------------+
```

Dependencies point downward. `render` and `anim` have no terminal dependency;
`terminal` is the only module that touches the terminal or emits protocol bytes;
`ui` composes frames and never performs I/O.

### Separation of cell rendering from image rendering

This is the central new boundary. A screen composes into a `Frame`, which bundles
two kinds of content that are rendered by very different machinery:

- **cells**, drawn through `Canvas` views; and
- **images**, recorded as `ImagePlacement { area: Rect, image: Arc<Image> }` —
  intent, not protocol.

A screen never emits graphics bytes. It says "an image belongs in this rectangle"
and composes a sensible fallback in the same rectangle for when nothing is
painted. `FrameContext::image_backend` tells it which case it is in, so the two
renderings are chosen deliberately rather than guessed.

The presenter is the only component that knows what the terminal was last told:

1. Cells inside an image rectangle are **never written**, so text can never shred
   a picture.
2. If the set of placements changed — including one disappearing — the whole
   screen is repainted, which bounds the damage of the reservation rule. A stale
   save of "what is under the image" therefore cannot persist.
3. Images are emitted after the cell pass, at absolute cursor positions, and only
   when they changed or after a full redraw. The cursor is repositioned afterwards
   because sixel leaves it somewhere unhelpful.

`ImageBackend` is an enum (`None`, `Sixel`), not a trait: there is one
implementation, and a trait would be speculation. Encoding is a pure function
(`terminal::sixel::encode`) with its own tests, so the protocol is verified
without a terminal.

### Capability states

`GraphicsSupport` is three-valued — `Unknown`, `Unsupported`,
`Likely(protocol)` — because treating "we could not tell" as support is how a
program paints protocol bytes into a terminal that renders them as garbage.
`ImageBackend::select` only chooses sixel when the hint says sixel **and** cell
metrics are known. A detected-but-unimplemented protocol (kitty, iTerm2) selects
no backend rather than a half-working one. `GRIMMOIRE_NO_GRAPHICS=1` forces
`Unsupported`.

### Background ownership

- The theme is explicit: `background`, `surface`, `surface_raised`, `border`,
  `text`, `text_muted`, `accent`, `accent_alt`, `effect`, `nature`, `relic`.
- The application fills every cell of the frame with the backdrop before
  composing, and `Canvas::set` inherits that background into foreground-only
  draws (§2).
- Panels, headers, and bars paint their surface explicitly, so a component that
  wants a raised panel gets one, and everything else keeps the viewport colour.
- The requirements are met for the *rendered viewport*. Window opacity is the
  emulator's business and is not claimed.

### Minimum viewport

`ui::viewport` owns the policy: **80 × 24**, the classic terminal and the size the
study and the game are composed for. Below it the application suspends normal
rendering, paints a dedicated legible screen, and **swallows input** so the player
cannot act blind. Screens are not composed at all while suspended, so nothing can
be caught half-drawn, and all game state is untouched. Play resumes on its own
when the window grows. The threshold is one constant in one file.

Fullscreen detection is deliberately *not* attempted: a large window and a
maximised one are treated identically, because the terminal cannot report the
difference reliably.

## 5. Graphics limits and honest compatibility

- **Primary target: a sixel-capable terminal.** Verified on `foot` 1.28 in this
  environment; sixel is also implemented by xterm (`-ti`), mlterm, Contour,
  WezTerm, iTerm2, and Windows Terminal 1.22+.
- **Fallback: always available.** With no backend the study draws an ASCII
  portrait and the game is unaffected. No code path requires graphics.
- **Kitty and iTerm2 are not implemented.** They are detected and reported, but
  select no backend. There is no kitty-family terminal here, so no runtime
  verification was possible and none was claimed.
- **Multiplexers** (tmux, zellij) may swallow sixel. Not tested here.
- **Cell metrics are mandatory.** Terminals that do not report pixel dimensions
  disable image drawing rather than guessing, because a wrong cell size produces
  images that overlap neighbouring panels.
- **Palette banding.** The encoder uses a fixed 6×6×6 cube, so smooth gradients
  band. Acceptable for a proof of concept; an adaptive quantiser is the upgrade.

## 6. Measurements

All figures from `cargo run --release --example bench_stage` on this machine
(187 × 46 viewport, portrait placed in a 19 × 16 cell panel; cells are 10 × 22 px
per the terminal's report). Every number is measured, not estimated.

| Operation | Cost |
| --- | --- |
| Compose one study frame (cells only) | 0.049 ms |
| Scale the portrait to the panel (190 × 352 px) | 0.44 ms |
| Encode that as sixel (~4 KiB) | 1.20 ms |
| Present a full redraw (7866 cells, 69 runs, 1 image) | 3.58 ms, 17 KiB |
| Present an unchanged frame | 0.107 ms, 16 bytes |
| Present a frame after 100 ms of scene motion | 0.164 ms, **68 cells in 38 runs, 2019 bytes, 0 images** |

The last row is the point of the hybrid design: while the scene animates at
30 fps, the image is *not* re-encoded or re-sent, and only the ~68 cells that
actually changed are written.

CPU, measured through a pseudo-terminal over a 2-second window (cells only, since
a pty reports no pixel size): **2.5% of one core** while animating at 30 fps, and
**0.0%** while paused.

## 7. Verification

**Visually verified in a real terminal** (`foot`, captured with `grim` and
inspected):

- The study renders as designed: header, scene, portrait, detail, and status
  regions, with the theme's surface hierarchy visible.
- The sixel portrait is painted inside its panel.
- The ASCII fallback renders cleanly with **no image remnants**.
- The game still renders correctly with the new theme.
- The suspend screen appears at 85 × 21 and is legible.

**Background ownership, by pixel sampling:**

| Frame | Sampled colour |
| --- | --- |
| `foot` with nothing drawn (control) | `gray(36)` — the terminal's own default |
| Grimmoire | `srgb(18,18,18)` — the theme background (indexed 233) |

**Image/cell coexistence, by pixel diff:** two frames 1.2 s apart differ by **0
pixels** in the portrait region and by **1300 pixels** in the animating scene
region — the picture is intact while cells animate around it.

**Pseudo-terminal smoke test** (11 checks, all passing): suspends below the
minimum, does not draw the game while suspended, **swallows movement input**
(verified through the save file: four `w` presses while suspended produced no
movement, and exactly one step happened after the resize), resumes after resize,
clears on resize, composites the help overlay, leaves the alternate screen,
restores the cursor, and exits 0.

**Unit tests:** 265 in total, including the sixel encoder's framing, palette
limit, RLE and band structure; background inheritance and full-viewport opacity;
image placement, reservation, re-emission and clearing; capability tri-state and
backend selection; layout non-overlap; and suspend-screen safety at absurd sizes.

**Not verified, and not claimed:**

- Kitty or iTerm2 rendering (no terminal available; not implemented).
- The runtime `g` portrait toggle, because key injection was unavailable in this
  environment (no `ydotoold`; `/dev/uinput` requires root). Both end states were
  verified as separate runs, and the clear path is covered by presenter tests.
- Behaviour under tmux/zellij, and on macOS or Windows.

## 8. Migration path

- **Another graphics protocol.** Add a variant to `ImageProtocol`/`ImageBackend`
  and an encoder; `Frame`, `ImagePlacement`, and the presenter's reservation rule
  are protocol-neutral and would not change. Kitty in particular would *remove*
  the cell-metrics requirement, because it can place by cell.
- **Better image quality.** Replace the fixed cube in `terminal/sixel.rs` with an
  adaptive quantiser; `Image` and the present path are unaffected.
- **Animated image content.** The placement key already forces a full repaint when
  an image changes, so per-frame animation would work but re-encode each time;
  the encoder cost above is the budget to work against.
- **Real artwork.** `ui::art::portrait` is procedural placeholder art. A decoder
  would be a new dependency; it is not needed until real assets exist.
- **Deferred renderer work.** Wide/zero-width glyph continuation cells, and
  damage-rectangle tracking instead of per-cell diffing.

## 9. Non-goals

- No terminal emulator, and no pixel drawing without a graphics protocol.
- No GPU window: the terminal premise holds until a requirement breaks it.
- No second graphics protocol until one is demonstrated to be necessary.
- No widget toolkit or retained scene graph.
