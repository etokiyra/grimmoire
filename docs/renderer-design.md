# Renderer design

This document records Grimmoire's rendering architecture, the evidence behind it,
the constraints of drawing inside a terminal, and the extension points left open.
It describes what is implemented today, not aspirations.

It was rewritten for the ASCII-scenery phase. §2 records what the audit found and
§3 records the direction decision, including the one that was reversed.

## 1. The constraint that shapes everything

A terminal emulator renders a **grid of character cells**, each with one glyph, a
foreground colour, and a background colour. A program does not address pixels
unless the terminal implements a graphics protocol, and even then it is *placing
an image*, not drawing into a pixel buffer. Three consequences follow:

- **Resolution is one glyph per cell.** Block and box glyphs (`█`, `▓`, `▀`, `·`)
  are one column wide and are legitimate drawing elements — confirmed against
  `unicode-width`, which treats East Asian Ambiguous characters as one column.
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
**17% of the viewport**, as a small island surrounded by empty margins. Four rows
of chrome framed it.

**Movement felt poor, for four separate reasons.**

1. A uniform 110 ms cooldown gated *every* movement source, so a fresh tap inside
   the window was **dropped** rather than queued.
2. There was no notion of a held direction and therefore no way to stop promptly.
3. The Kitty keyboard protocol was never enabled, so `KeyEventKind` was always
   `Press` and the `repeat` flag was dead code.
4. Because the camera centred a small map, walking around the middle of a room
   produced no visual change at all.

**The remaining complaint, in this phase's words:** horizontal movement felt too
slow beside vertical movement, which felt too fast, and there was no sprint.
§6 explains why, and what was done about it.

## 3. The direction decision

**Grimmoire is an ASCII game, and that is the identity rather than a limitation.**
The world is drawn as characters, one glyph per cell, by a scenery renderer that
Grimmoire owns.

An earlier revision of this phase rendered the world as a software pixel-art
image and had Kitty display it behind the cell layer. That was reverted on
instruction: it looked like a different game, and the ASCII wall of the project
was better served by investing in the character renderer. The pixel renderer
survives as an **optional interface extra** behind the `--pixel-ui` development
flag, off by default (§7). Nothing in the default path emits a graphics sequence
at all, and a test asserts it.

### Why the terminal is still Kitty

Kitty remains the supported terminal because of the **keyboard** protocol, which
is what makes "hold to walk, release to stop" possible at all: it reports key
releases, and `REPORT_EVENT_TYPES` is what turns an auto-repeat into something
distinguishable from a fresh press. Colour depth is the other reason.

Because the world no longer needs images, the capability check is best understood
as "this terminal speaks the keyboard protocol", which is exactly what
`crossterm`'s live query measures. `GRIMMOIRE_ASSUME_KITTY=1` remains the
documented development override, and the pty smoke test uses it.

## 4. Chosen architecture

```
+---------------------------------------------------------------------+
| app::App   clock, screen stack, transition, canvas, image placements |
+------------------+-------------------------------+------------------+
| ui::*                                                             |
|   scenery  the ASCII world renderer                               |
|   theme    roles, surfaces, and the ambient shade ramp            |
|   motion   press, hold, release, sprint, per-axis cadence         |
|   screens  exploration, dialogue, help, study, showcase           |
|                                     +----------------------------+ |
|                                     | terminal::*                | |
|                                     |   Terminal, Presenter      | |
|                                     |   Capabilities, Graphics   | |
|                                     |   kitty (pure encoder)     | |
|                                     +----------------------------+ |
+---------------------------------------------------------------------+
| render::*  Canvas, CanvasView, Frame, Cell, Style, Color, Rect,      |
|            ImagePlacement, AssetId, glyph, png (optional UI only)    |
+---------------------------------------------------------------------+
| anim::*    Animator, Animation, Easing                              |
| input::*   Key, KeyCode, KeyKind, Event  (terminal-independent)      |
| game::*    areas, entities, world, dialogue  (pure)                  |
| save::*    a small versioned format                                  |
+---------------------------------------------------------------------+
```

Dependencies point downward. `render`, `anim`, `game`, and `ui` have no terminal
dependency; `terminal` is the only module that touches the terminal or emits
protocol bytes.

## 5. The ASCII scenery renderer

`ui::scenery` draws the world into a cell view. It is pure: a world and a palette
in, cells out. Three things make it read as a place rather than as a field of
symbols.

### Structure, from neighbours

A wall is not `#` repeated. Its glyph is chosen from what surrounds it:

| Situation | Glyph | Why |
| --- | --- | --- |
| Open floor above | `▀` | the wall's lit cap |
| Open floor below | `▄` | its shadowed base |
| Open floor beside | `▓` | a dressed side face |
| Surrounded | `#` | interior masonry |
| Floor on all four sides | `║` | a free-standing pillar |

That single rule is what turns a chapel's colonnade, a crypt's chambers and a
corridor's edges into architecture. A worked test asserts a cap, a base and a
pillar are three different glyphs.

### Variation, from a hash

Floors, paths, water, rubble and moss pick their glyph from a small set using a
hash of the tile's **world** position, not of the frame or the call order. The
same tile therefore looks the same every time it is drawn, so the world never
shimmers as the camera moves, and neighbouring tiles differ. The variation is
deliberately restrained — a floor is one surface with grit on it, not static.

### Characters as art

The player and the people they meet are small ASCII figures, three cells square,
rather than a single coloured letter:

```
 ^      ^      ^
(o)    (.)    (o>      and the hooded figure: (oo)
/|\    /|\    /|\                             |###|
```

They have a facing direction (four variants, with west mirrored from east), a
two-frame walk cycle, and a distinct palette role. The staff-bearing wizard and
the dark, still, two-eyed hooded figure are not confusable at a glance.

### Light, from distance

Every cell is shaded by its distance from the player, using the theme's three-step
ambient ramp as the background and the `dim` attribute for the far distance. The
effect is a pool of light around the wizard that falls away into shadow — the
same trick the cell renderer used before, kept because it is cheap and it works.

Aerial perspective is not attempted, and neither is field of view. The whole area
is drawn every frame; only the visible window is composed.

## 6. The input model

Movement is an explicit state machine (`ui::motion`), not a throttle on the raw
key stream:

- a **fresh press** steps immediately, with no gate and no latency;
- **auto-repeat** steps again only once the cadence has elapsed;
- a **release** stops the walk at once;
- **shift** makes the hold a sprint.

The critical decision is that **steps are taken on events, never on a timer**. An
earlier revision self-drove the cadence while a direction was held; live testing
showed that a press with no release — exactly what injected or pasted input looks
like — then walked ~3 steps before a stall guard caught it. Event-driven motion
makes a tap exactly one step and makes a runaway *impossible by construction*. A
repeat that does not continue a hold is ignored, and a stall guard clears the
reported held state after 400 ms of silence without ever taking a step.

### Why the two axes do *not* share a cadence

This is the fix for "horizontal feels slow, vertical feels fast".

A step moves the view exactly one cell. Terminal cells are about twice as tall as
they are wide — measured here at **9 × 20 pixels**. At one cadence, a step north
would cross more than twice the screen distance of a step east, so holding a
direction looked fast vertically and sluggish horizontally.

The fix is to move at a constant **screen speed** rather than a constant step
rate:

| Direction | Step | Cadence | Screen speed |
| --- | --- | --- | --- |
| north / south | 20 px | 133 ms | 150 px/s |
| east / west | 9 px | 60 ms | 150 px/s |

Both axes are derived from one constant (`WALK_SPEED`, with `SPRINT_SPEED` at
twice it), and the cell size comes from the terminal, so this adapts to the font
instead of being tuned for one. A floor of 45 ms per step keeps a very narrow
terminal from asking for more steps a second than a keyboard can deliver.

## 7. The optional pixel interface

`--pixel-ui` (or `GRIMMOIRE_PIXEL_UI=1`) enables a small software pixel renderer
for **interface pieces only**: the dialogue portrait, and the study's portrait
panel. It is off by default, the game never depends on it, and the default path
emits no graphics sequence.

The pixel layer is a flat RGB canvas, sprites authored as ASCII, and a PNG
encoder (`render::png`) that hands the bytes to Kitty. The PNG container is
hand-written — signature, `IHDR`, `IDAT`, `IEND`, Paeth filtering, CRC-32 — and
only DEFLATE comes from a dependency (`fdeflate`, chosen for PNG scanline data).
Nothing in the project decodes an image: Kitty does that.

This exists so the pixel work is inspectable rather than thrown away, and so a
future phase can decide to use it deliberately.

## 8. Verification

**Visually verified in a real Kitty session** (0.49.2, driven through Kitty's own
remote-control socket and captured with `grim`, then inspected):

| Check | Result |
| --- | --- |
| Launches into the playable world | Ruined Chapel, world edge to edge |
| The world is ASCII | a cathedral: nave, colonnade, side chapels, apse, graveyard |
| The player is a figure, not a letter | `(o)` with a hood and a walk cycle |
| The hooded figure is distinct | `(oo)` over a dark cloak, in its own colour |
| One press is exactly one step | `70,44` → `70,43` for one `w` |
| A burst cannot sprint the player | ten presses inside one cadence moved one tile |
| Sprint is real | the burst with shift moved one tile; the indicator appears |
| Travel between both areas | chapel → cellar (`20,12`) → chapel |
| The cellar is a different place | four chambers, one flooded, bridges between |
| Dialogue is an ASCII overlay | panel, title, ASCII bust, lines, close hint |
| The interface does not cover the world | the world is visible around the panel |
| No graphics are sent by default | asserted by the smoke test |
| The terminal is restored | alternate screen left, cursor shown, images deleted |

**Pseudo-terminal smoke test** (`tools/smoke.sh`, 12 checks, all passing): enters
the alternate screen, hides the cursor, composes the ASCII world, draws the
player, shows the controls, suspends below the minimum viewport, resumes after the
resize, shows the cursor, leaves the alternate screen, **sends no graphics by
default**, cleans up images, and exits 0. It runs the game under a scratch
`XDG_DATA_HOME`, so it can never read or write a real save.

**Unit tests:** 333, including the scenery renderer (structure, variation
stability, water motion, every glyph one column wide, every cell shaded), the
movement state machine (immediate press, cadence, release, direction change, stray
repeats, the stall guard, sprint, and equal screen speed on both axes), the
presenter's diffing, the camera, the minimum viewport, the game's rules, and the
save format.

**Not verified, and not claimed:** behaviour under tmux/zellij; other Kitty-family
terminals; macOS or Windows; resizing while a *pixel* interface piece is on
screen (the pty smoke test covers suspend and resume, and the graphics state is
unit-tested to be discarded and rebuilt after a clear).

## 9. Measurements

From `cargo run --release --example bench` on this machine, at a 120 × 40 frame
with the world composed and shown. Every number is measured.

| Operation | Cost |
| --- | --- |
| Compose a frame (scenery + interface) | 0.28 ms |
| Present a full redraw | 0.7 ms, 4800 cells in 40 runs, 37 KiB |
| Present an unchanged frame | 0.11 ms, **16 bytes** |
| Present one movement step (camera scrolled) | 1.1 ms, 3207 cells in 913 runs |
| Idle | no frames at all until the world's own detail ticks, ~4 times a second |

The presenter numbers move by a few hundred microseconds with machine load; the
composition number does not. For comparison, the reverted pixel path composed a
frame in **4.7 ms** — around seventeen times the cost — because it had to
rasterise and PNG-encode forty thousand pixels every frame. ASCII is not a
compromise here: it is cheaper *and* it is the look the project wants.

## 10. Migration path and non-goals

- **More art** means more glyph tables and more neighbour rules in `ui::scenery`;
  nothing else changes.
- **A tile-drawing DSL** would be the natural next step if the scenery rules grow
  past a handful of matches.
- **The pixel interface** could be extended to more of the UI, or removed, without
  touching the game.
- **Deferred renderer work**: wide/zero-width glyph continuation cells, and
  damage-rectangle tracking instead of per-cell diffing.

Non-goals: no terminal emulator; no widget toolkit, retained scene graph, or ECS;
no second terminal backend; no GPU window. Gradients and photographic art are not
attempted, because the identifier is a glyph grid.
