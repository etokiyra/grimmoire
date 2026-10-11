# Renderer design

This document records Grimmoire's rendering architecture, the evidence behind it,
the constraints of drawing inside a terminal, and the extension points left open.
It describes what is implemented today, not aspirations.

It was rewritten for the Kitty-first phase. §2 records what the audit found before
any change; §3 records the protocol decision and the evidence behind it.

## 1. The constraint that shapes everything

A terminal emulator renders a **grid of character cells**, each with one glyph, a
foreground colour, and a background colour. A program does not address pixels
unless the terminal implements a graphics protocol, and even then it is *placing
an image*, not drawing into a pixel buffer. Three consequences follow:

- **Resolution is sub-cell at best by default.** Block and box glyphs (`█`, `░`)
  are one column wide, so they *can* be used as colour swatches and shading —
  confirmed against `unicode-width`, which treats East Asian Ambiguous characters
  as one column. Photographic imagery needs a graphics protocol.
- **The cell grid desynchronises** if a glyph is not exactly one column wide. The
  renderer enforces a width policy at draw time and again at the output boundary.
- **Output cost is real.** Rewriting a large screen is wasteful, so the presenter
  diffs against the previous frame.

## 2. What the audit found

Verified in code and on screen before anything changed; the full record is in
[`phase5-audit.md`](phase5-audit.md).

**The world occupied a fraction of the screen.** `axis_camera` centred any area
smaller than the view, and both areas were far smaller than a normal terminal
(46 × 16 and 34 × 12). At the reference size the Ruined Chapel covered roughly
**17% of the viewport area**, as a small island surrounded by empty margins. Four
rows of chrome (header, message, two-row status bar) framed it.

**Movement felt poor, for four separate reasons.**

1. A uniform 110 ms cooldown gated *every* movement source, so a fresh tap inside
   the window was **dropped** rather than queued.
2. There was no notion of a held direction and therefore no way to stop promptly.
3. The Kitty keyboard protocol was never enabled, so `KeyEventKind` was always
   `Press` and the `repeat` flag Phase 3 added was never set. The guard that was
   supposed to stop auto-repeat re-triggering discrete actions was dead code.
4. Because the camera centred a small map, walking around the middle of a room
   produced no visual change at all. This, more than the timing, is what made
   movement feel dead.

**Incidental findings.** The sixel backend required the terminal to report cell
metrics in pixels, which many terminals do not; the capability probe could not
distinguish "no graphics" from "could not tell"; the theme had no separators,
focus, interactive, player, or warning tokens.

## 3. The protocol decision: Kitty, and only Kitty

Kitty is the supported terminal for this phase. That is a product decision — visual
quality and control over breadth of compatibility — and it is implemented as one.

The protocol was chosen on evidence, not on documentation:

| Claim | How it was checked |
| --- | --- |
| Kitty is installed here | `kitty 0.49.2` at `/usr/bin/kitty`, plus `kitten` |
| Kitty renders images here | `kitten icat assets/cellar.png` in a real Kitty window, captured with `grim` and inspected |
| Kitty speaks the protocol | `crossterm`'s live query returned `Ok(true)`; the running binary reports `graphics  kitty (confirmed)` |
| Kitty reports key releases | the same query gives `keys  press/repeat/release`, and holding a direction visibly repeats |

Sixel was removed rather than kept as a second backend. The instruction is explicit
that theoretical flexibility is not a reason to carry two paths, and the second path
was the one with the worse layout model: sixel has no concept of cells, so it needs
the terminal's pixel cell size, which is exactly the thing a pty and many terminals
do not report. Kitty places **by cell** (`c` and `r`), so Grimmoire no longer needs
cell metrics at all.

### What Kitty is asked to do

- **Inline PNG, transmitted once** (`a=t,f=100,t=d`), because a file path (`t=f`)
  assumes a shared filesystem — false over SSH and in sandboxes — and a failed file
  read is reported as an unread response. The PNG bytes are copied into the escape
  sequence untouched: **no image codec exists anywhere in the project**.
- **Placements with ids** (`i` + `p`). Re-sending the same pair replaces the
  placement in place, so moving an image needs no delete and does not flicker.
- **Cell-aligned rectangles** (`c`/`r`), so an image occupies a known number of
  cells and the cell layer can be told not to write there. Kitty letterboxes rather
  than distorting.
- **Placement at the cursor.** Kitty draws at the current cursor cell, so every
  placement moves the cursor to the rectangle's top-left corner first and then asks
  for the cursor to stay put (`C=1`). Getting this wrong is invisible until an image
  silently lands in the wrong place; it was found by live verification (§7).
- **Responses suppressed** (`q=2`). Acknowledgements arrive on the *input* stream,
  and this application reads input as key events — an unread acknowledgement would
  surface as garbage keystrokes.

## 4. Chosen architecture

```
+---------------------------------------------------------------------+
| app::App   clock, screen stack, transition, canvas, image placements |
+------------------+-------------------------------+------------------+
| ui::*  screens, layout, theme, art, stage        | terminal::*      |
|        Screen trait -> Frame                     |   Terminal       |
|                                                  |   Presenter      |
|                                                  |   Capabilities   |
|                                                  |   Graphics       |
|                                                  |   kitty (pure)   |
+------------------+-------------------------------+------------------+
| render::*  Canvas, CanvasView, Frame, Cell, Style, Color, Rect,     |
|            ImagePlacement, AssetId, glyph                           |
+---------------------------------------------------------------------+
| anim::*    Animator, Animation, Easing                              |
| input::*   Key, KeyCode, KeyKind, Event  (terminal-independent)     |
| assets::*  PNG loading and validation (the only module that reads   |
|            image files)                                             |
+---------------------------------------------------------------------+
```

Dependencies point downward. `render` and `anim` have no terminal dependency;
`terminal` is the only module that touches the terminal or emits protocol bytes;
`ui` composes frames and never performs I/O.

### Separation of cell rendering from image rendering

A screen composes into a `Frame`, which bundles two kinds of content rendered by
very different machinery:

- **cells**, drawn through `Canvas` views; and
- **images**, recorded as `ImagePlacement { area: Rect, asset: AssetId }` —
  intent, not protocol, and not even bytes.

A screen never emits graphics bytes and never sees image data: it names an asset
handle and a rectangle, and composes a sensible fallback in that rectangle for
when nothing is painted. `FrameContext::graphics` and `FrameContext::art` tell it
which case it is in, so the two renderings are chosen deliberately.

`terminal::kitty` is a **pure encoder**: intent in, bytes out, no I/O and no state,
so the protocol is verified in unit tests without a terminal.

`terminal::graphics::Graphics` owns the lifecycle and is the only place that
decides when to transmit, place, or delete:

- An image's data is transmitted **once** and kept under a stable id. Showing it
  again costs nothing.
- A placement is identified by `(image, placement)`, so an unchanged rectangle
  emits nothing and a moved one is replaced without flicker.
- A **full redraw discards the graphics state**, because the clear-screen escape
  code is specified to clear images too. Data is retransmitted and placements are
  re-issued. This is what makes resizing safe.
- `Graphics::clear` deletes every image and placement the application created, and
  runs before a resize and on shutdown — and again in `Terminal::leave` and in the
  panic hook, so no exit path can leave graphics behind.

The presenter decides *when*: it hands the graphics layer the frame's placements
and a `reset` flag, and writes what came back.

Worked example of the whole chain, for one step of movement while a portrait is on
screen: the camera shifts, ~250 cells change, the presenter writes those 250 cells
in ~170 runs — and emits **zero** image bytes, because the portrait did not move.

### Capability states

Startup is strict, and the policy lives in `terminal::graphics::classify` (pure,
and unit tested):

| Evidence | Result | Behaviour |
| --- | --- | --- |
| The terminal answered the live Kitty protocol query | `Confirmed` | run |
| `GRIMMOIRE_ASSUME_KITTY=1` (development override) | `Forced` | run, reported as forced |
| `KITTY_WINDOW_ID`, `TERM`, or `TERM_PROGRAM` look like Kitty, but nothing answered | `Unverified` | run, with a warning on stderr |
| Nothing suggests Kitty | `Unsupported` | **print an actionable error and exit** |

`$TERM` alone is never treated as proof. `KITTY_WINDOW_ID` is set by Kitty itself
for its children, so it is evidence; it is still not confirmation, which is what the
live query is for. The override exists so the cell layer can be inspected in a
terminal that cannot paint images — including this project's own pty smoke test —
and it is documented as development-only.

Colour detection is separate and purely cosmetic: it selects the palette and
nothing else, so a wrong guess is never fatal.

### Background ownership

- The theme is explicit and role-based: `background`, `surface`, `surface_raised`,
  `border`, `separator`, `text`, `text_secondary`, `text_muted`, `accent`,
  `accent_dim`, `accent_alt`, `accent_alt_dim`, `effect`, `effect_dim`, `focus`,
  `interactive`, `player`, `character`, `nature`, `relic`, `warning`, and the three
  `shade_*` steps of the ambient ramp.
- The application fills every cell of the frame with the backdrop before composing,
  and `Canvas::set` **inherits** that background into foreground-only draws, so a
  draw can never reset a cell to the terminal's own background.
- Panels, bars, and overlays paint their surface explicitly; everything else keeps
  the viewport colour.
- This is asserted twice: a unit test walks every cell of a composed frame and
  requires a non-default background, and a pixel sample from a real terminal
  confirms the colour.
- Window opacity is the emulator's business and is not claimed.

### Composition and the camera

The world is composed across the **entire viewport**; the HUD is a two-row overlay
painted *over* it rather than rows taken from it. The camera follows the player and
clamps to the area's edges; an area smaller than the view is centred, which only
happens for a pathologically small map. Both areas are larger than the reference
viewport by construction, and a content test enforces that.

Floors are tinted by squared distance from the player, which reads as a pool of
light, gives the space depth, and costs no geometry: one `match` in the tile loop.

### Minimum viewport

`ui::viewport` owns the policy: **80 × 24**, the classic terminal and the size the
game and the study are composed for. Below it the application suspends normal
rendering, paints a dedicated legible screen, and **swallows input** so the player
cannot act blind. Screens are not composed at all while suspended, so nothing can
be caught half-drawn, and all game state is untouched. Play resumes on its own when
the window grows, and the presenter is invalidated so the terminal is repainted
from scratch. The threshold is one constant in one file.

Fullscreen detection is deliberately *not* attempted, and Grimmoire cannot maximise
the user's window. It enforces its own minimum and says so.

## 5. The input model

Movement is an explicit state machine (`ui::motion`), not a throttle on the raw key
stream, because the three things a player notices are distinct:

- a **fresh press** steps immediately, with no gate and no latency;
- **auto-repeat** steps again only once the cadence (125 ms, ~8 steps a second) has
  elapsed, so a fast terminal cannot make the player sprint;
- a **release** stops the walk at once.

The critical design decision is that **steps are taken on events, never on a timer**.
An earlier revision self-drove the cadence while a direction was held; live testing
showed that a press with no release — exactly what injected or pasted input looks
like — then walked ~3 steps before a stall guard caught it. Event-driven motion
makes a tap exactly one step and makes a runaway *impossible by construction*.

Two further rules fall out of that:

- A **repeat that does not continue an existing hold is ignored**, so only a press
  starts moving and a release is final even if stray repeats arrive.
- A **stall guard** clears the reported held state after 400 ms of silence, well
  above any plausible auto-repeat interval. It never moves anyone, so a lost release
  cannot leave the player walking.

Discrete actions (`e`, help, screen changes) are separate from movement entirely and
fire only on a non-repeat press, so holding a key cannot interact over and over.

The Kitty keyboard protocol is requested in `Terminal::enter` only when the terminal
answers for it, and released on every exit path.

## 6. Assets

Three PNGs, generated by a committed script (`tools/generate-assets.sh`): an
arrival plate for each area and a portrait. They are original, produced
procedurally from gradients and shapes, so the repository contains no third-party
imagery, there are no usage rights to track, and no image decoder is needed to read
them — `assets::Assets` validates the PNG signature, and Kitty does the decoding.

Artwork is used where a picture materially helps and nowhere else:

- an **arrival plate**, shown after the travel curtain when you enter an area, and
  gone again a second and a half later;
- a **portrait** beside the speaker in the dialogue overlay.

The world itself is character cells. Converting terrain into images would cost far
more than it bought, and would make the game unreadable wherever an image failed.

Assets are required: if the files are missing the application names them and exits,
rather than starting with a subtly broken look.

## 7. Verification

**Visually verified in a real Kitty session** (0.49.2, driven through Kitty's own
remote-control socket and captured with `grim`, then inspected):

| Check | Result |
| --- | --- |
| Launches into the playable world at 120 × 40 | Ruined Chapel, world edge to edge |
| The world dominates the viewport | only two rows are HUD; the rest is environment |
| Movement, in every direction, one press one step | `70,44` → `70,43` for one `w` |
| Holding a direction walks | repeats step at the cadence; a burst inside it does not |
| Travel between both areas | chapel → cellar (`20,12`) → chapel (`70,49`) |
| The arrival plate renders | the cellar plate is painted, with its caption beneath |
| Cells compose with the image | the world is visible around the plate; no remnants |
| Dialogue opens as an overlay | panel, speaker title, lines, close hint |
| The dialogue portrait renders | drawn beside the text, world visible around the panel |
| Interactive objects are discoverable | prompt `e  speak with the Hooded Figure` |
| The keyboard protocol is released on exit | Kitty reports flags `?0u` afterwards |
| Images are deleted on exit | `ESC_Ga=d,d=A ESC\` is emitted; no graphics remain |
| The terminal is restored | alternate screen left, cursor shown again |

**Background ownership, by pixel sampling:** the `foot` control (nothing drawn)
samples `gray(36)`; Grimmoire samples `srgb(18,18,18)`, the theme background.

**Pseudo-terminal smoke test** (`tools/smoke.sh`, 10 checks, all passing):
enters the alternate screen, hides the cursor, composes the world and its controls,
suspends below the minimum viewport, resumes after the resize, shows the cursor,
leaves the alternate screen, deletes its images, and exits 0.

**Unit tests:** 281, including the Kitty encoder's framing, chunking, base64,
deletion forms, and cursor-movement prefix; the graphics lifecycle (transmit once,
place, move without retransmitting, reset after a clear, clear, and skip a missing
asset); the movement state machine (immediate press, cadence, release, direction
change, stray repeats, the stall guard); the presenter's reservation and repaint
rules; the camera and full-viewport composition at many sizes; the minimum viewport;
content connectivity (every walkable tile reachable, entities reachable, doorways on
doors, arrival tiles clear); and background inheritance.

**Not verified, and not claimed:** behaviour under tmux/zellij; other Kitty-family
terminals; macOS or Windows; the *study* screen's `g` portrait toggle, which still
cannot be exercised by key injection; and **resizing the Kitty window while an
image is on screen**. That last one is covered indirectly — the pty smoke test
suspends and resumes across a resize, and the graphics state is unit-tested to be
discarded and rebuilt after a clear — but the live pixel result was not observed,
because this environment could not resize the window on command.

## 8. Measurements

From `cargo run --release --example bench` on this machine, at a 120 × 40 frame,
with the world composed and one image placed (cells are 10 × 22 px here). Every
number is measured, not estimated.

| Operation | Cost |
| --- | --- |
| Compose the world (120 × 40) | 0.083 ms |
| Present a full redraw with one image | 0.56 ms, 2880 cells in 60 runs, 107 KiB |
| Present an unchanged frame | 0.078 ms, **16 bytes, zero images** |
| Present one movement step (scrolled camera) | 0.23 ms, 253 cells in 168 runs, 8 KiB, **zero images** |
| Idle, and while stepping | below measurement resolution (0.00% of one core) |

The unchanged-frame and movement rows are the point of the design: ordinary play
never retransmits an image. A full session transmits each asset once, so the 232 KiB
of PNG on disk crosses the wire once, as roughly 310 KiB of base64.

## 9. Migration path

- **A second graphics protocol** would be a new encoder module plus a variant in the
  capability policy. `Frame`, `ImagePlacement`, `AssetId`, and the presenter's
  reservation rule are protocol-neutral.
- **Real artwork** means replacing the generated PNGs; nothing in the code changes.
  A decoder dependency is still not needed, because the terminal decodes.
- **Animated imagery** is possible: `kitty` already knows how to chunk a payload,
  and the placement key forces a full repaint when an image changes.
- **Deferred renderer work**: wide/zero-width glyph continuation cells, and
  damage-rectangle tracking instead of per-cell diffing.

## 10. Non-goals

- No terminal emulator, and no pixel drawing without a graphics protocol.
- No GPU window: the terminal premise holds until a requirement breaks it.
- No second graphics protocol until one is demonstrated to be necessary.
- No widget toolkit, retained scene graph, or ECS.
