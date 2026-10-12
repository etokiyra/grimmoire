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

`ui::scenery` draws the world into a cell view. It is pure: a world, a palette, a
time and a pose in; cells out. Five things make it read as a place.

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
corridor's edges into architecture.

### Variation, from a hash

Floors, paths, water, rubble and moss pick their glyph from a small set using a
hash of the tile's **world** position, not of the frame or the call order. The
same tile therefore looks the same every time it is drawn, so the world never
shimmers as the camera moves, and neighbouring tiles differ. The variation is
deliberately restrained — a floor is one surface with grit on it, not static.

### Inlay, for the wide open floors

Several rooms in both areas are large and empty, and left alone they read as
graph paper. Each area therefore carries a **motif**: the chapel has a rose cut
around its crossing plus a coarse lattice out to the edges; the cellar has
concentric rings around its sigil. It is drawn only on plain, walkable floor that
holds no entity, no doorway and no player — never over something that matters.

### Life, from position and time

- **Candles** gutter along wall faces that open onto floor, in a deterministic
  scatter, cycling through a small flame glyph set.
- **Motes** drift over bare floor. They are anchored to the *world*, so they
  scroll with it and read as dust in a room rather than dirt on the screen, and
  they are sampled from a slowly shifting lattice — which is how they drift with
  no per-mote state at all. A few of them are blossom rather than dust, which is
  most of what makes the air read as anime.
- **Sparkles** are thrown off an interaction or an arrival, flying outward as the
  pulse fades. They are deterministic and only ever land on bare, unoccupied
  floor.
- **Water** carries two wave trains at different speeds plus occasional crests,
  so the surface rolls instead of repeating one stripe.
- **The sigil** turns through two phases and pulses in the effect colour.

Every one of these is a function of `(position, time)`. None uses randomness, the
frame count, or anything from the machine — so the same world and the same time
always produce the same frame.

### Figures, in several inks, at the scale of the props

Characters are not single coloured letters, and they are not giants either. A
figure occupies **three cells** — the same as a doorway, a tree or the sigil — so
she stands in the world rather than towering over it. Chibi proportions do the
rest: a tall hat and a face with two big eyes take most of the figure, and the
dress underneath is one row.

A figure is ASCII art plus a **legend**: each character in its rows names an ink,
and each cast member carries its own `Inks` value. That is what lets one small
figure have pink hair, warm skin, a violet dress with white lace, big bright eyes
and a star wand at once.

Because the sprite is five cells wide, the world is authored to match it: **no
passage the player can stand in is narrower than three cells**, and the routes
walked constantly are drawn wider. A one-cell corridor would put her shoulders
through the wall on both sides. [`map-design.md`](map-design.md) states the rule
and the widths in use; `areas::MIN_PASSAGE` and the generator enforce it.

### The cast is a table, not a special case

`ui::cast` holds every character's art, inks and dialogue bust in one place. The
world model names a **role** (`Figure::Lantern`), the interface decides what that
looks like, and `ui::scenery` never learns who it is drawing. That is what let the
cast go from one hardcoded silhouette to five distinct characters without the
renderer growing a branch per character — and adding a sixth is a variant, a table
entry, and nothing else.

The same holds for props: an object names a `Prop`, and one table maps it to art
and ink. Before, objects were told apart by their single drawing character, which
meant adding one meant editing a match in the renderer and hoping no two shared a
glyph.

### Speaking: portraits, expressions and choices

A conversation is drawn on a plate with the speaker's face in an inset of its
own. The inset is darker than the panel on purpose: Ysolde is a hood with no face
and is drawn in inks that are almost the colour of the surface, so a portrait
needs a background that is deliberately *behind* it. Every character's portrait
is eight rows tall with the face on row three, and row three is swapped for one
of four eye rows to change the expression — the same art, four moods.

Lines arrive a character at a time and any key that would advance completes them
first, so the reveal is never in the way of reading quickly. With motion off the
lines are simply there.

Options sit at the bottom of the plate rather than under the lines, so moving the
selector never makes the text jump, and the hint on the last row says what the
keys do for the state the conversation is actually in.

### Opening: the title screen

The title is a sky, a moon, a horizon with the chapel on it, and the name. Every
part of it is a function of one number — how long the screen has been open — so
the stars twinkle on a hash of their position and the clock, the runes rise, the
name reveals left to right and the menu assembles a row at a time. Nothing is
stored and nothing drifts.

The name is drawn with a vertical ramp, gold at the top and ember at the base,
with a one-row shadow so it sits on the sky rather than in it.

### Place: material and air

Two knobs give an area its identity, and both are small:

- **`Ground`** — the glyphs, ink and grit of the open floor. The wood is grass and
  leaf litter, the village is ash and brick-dust, the shrine is pale polished
  slab, the catacombs are bone dust. The floor is the largest surface on screen,
  so it is the loudest cue for *where* the player is.
- **`Air`** — what drifts. The same mote system, with a different particle:
  pollen and blossom in the wood, ash and the odd cinder in the village, cold
  silver in the shrine, bone dust in the catacombs. Density differs too, so a wood
  is busier than a crypt.

A third, the **motif** inlaid into open floors, was already there.

```
 ^ ^                  ___
(●:●)   the bunny    (o@o)   Ysolde
/#+#\   and wand     \###/   (no skin at all:
                               cold light in a cowl)
```

Both are two head rows plus a robe row, so a walk cycle costs two hem rows per
direction rather than a second full figure. The stride alternates per *step* and
swishes the robe, and the wand's star twinkles.

**The profiles are authored rather than mirrored, and the eye sits toward the
direction of travel.** A two-row head has almost nothing to say which way it is
facing, so that eye position is the whole cue — the first attempt put the eye in
the same place for both profiles and read as a missing eye rather than as a
turned head. An idle figure blinks on a slow clock and does not ask for a frame
while it does so.

### Draw order, and feedback

Terrain, then inlay, then props, then ambience, then figures. A **pulse** — the
transient effects in `ui::effects` — is not a layer: it is a *style* change applied
wherever it lands, so feedback can never hide a glyph. A blocked step underlines
the cell that refused it; an interaction flashes what answered; an arrival marks
where you now stand.

## 6. Animation and the frame loop

### What animates, and what drives it

- The **walk stride** advances once per step, taken from the movement model, so
  the legs keep pace with the world whether a step takes 60 ms or 133, and stop
  dead when movement stops. A tap keeps its stride for the length of one cadence
  rather than snapping back to idle.
- **Sprint** is a separate stage: the same stride at twice the rate, plus a
  lean and a short wake drawn behind the figure.
- **Idle** breathes and blinks on its own slow clock.
- **Ambient detail** (water, motes, flames, the sigil) advances with the world
  clock.
- **Overlays** animate themselves: the dialogue panel rises from the bottom edge,
  brightens as it settles, and slides away again on dismissal.

All of it is a function of elapsed time, advanced explicitly. Nothing depends on
how many frames were drawn, so a slow machine does not slow the animation down
and a fast one does not speed it up.

### The loop

The application asks the screens how soon they want another frame. While
something is animating — a step, a fading pulse, a transition — that is
`FRAME_INTERVAL`, sixty a second. While the world is merely breathing it is a
slower ambient tick. When nothing is happening at all, it is input.

The animation clock is **deadline based**: each frame is scheduled from the
previous frame's deadline rather than by sleeping a fixed interval after doing
the work, so composition time never accumulates into drift. The wait is also
clamped to one frame interval, because a deadline set while the game was idle must
never be inherited by the first frame of a step — that is exactly how a "60 Hz"
loop quietly becomes a 30 Hz one.

**Sixty updates a second is not sixty distinct frames on screen.** A frame is
composed and diffed every tick, but only the cells that actually changed are
written. On a still frame that is nothing at all; while walking in a room that
fits the viewport, it is a handful of cells.

### What the cell grid cannot do

A step moves the camera one whole cell, and a cell is a cell: there is no
sub-cell offset in a terminal. Camera motion is therefore inherently chunky at
the tile boundary, and this phase did not pretend otherwise. What was improved is
everything around it: the stride is locked to steps so the figure never looks
disconnected from the motion, both axes cross the screen at the same speed, and
the frame loop now runs fast enough that a transition or a fade is smooth.

## 7. The procedural atmosphere layer

`ui::atmosphere` is the one addition the cell grid has had since the first
renderer: a **background layer** that can hold a gradient. Nothing above it
changed, and the game itself does not use it yet.

### Why it exists

The grid composes one glyph, one foreground colour and one background colour per
cell, so a smooth change of colour has nowhere to live. Every "gradient" before
this was three discrete steps out of the palette, and that is the ceiling the
VinZ investigation identified: see
[`renderer-investigation.md`](renderer-investigation.md).

### How one cell carries two colours

The field is written as the upper half block, `▀`, whose **foreground** is the
colour of the top half of the cell and whose **background** is the colour of the
bottom half. A field `h` rows tall is therefore sampled `2h` times, which is what
allows a 29-row terminal to draw a 58-step gradient.

That is a technique, not a dependency: it is around thirty lines against our own
`Canvas` and it was written from the concept.

### What survives composition, exactly

This is the part that is easy to assume and wrong to assume. The rule the canvas
already has — a draw that leaves `Style::bg` at [`Color::Default`] inherits the
background underneath — decides everything:

| What lands on the cell | Glyph | Foreground | Background |
| --- | --- | --- | --- |
| nothing (open field) | `▀` | the upper sample | the lower sample |
| a scenery glyph, sprite glyph or stray text | that glyph | the caller's ink | **the lower sample** |
| a blank space with a foreground | `' '` | the caller's ink | the lower sample |
| anything with its own background | its own glyph | its own ink | **its own colour** |

So the field has two samples per cell in the open, **one** sample per cell
beneath a glyph, and none where a caller paints its own surface. The error under
a glyph is bounded by half a row of the ramp, because the surviving sample is
half a row below the cell's centre — which at the sizes in use is under 3 units
of colour out of 255. `ui::atmosphere`'s tests assert every row of that table
rather than describing it.

A consequence worth knowing: writing a space *does* displace the half block, so
text turns the cells it covers into flat cells in the lower sample's colour. That
is a graceful degradation, not damage, but it is why the field is at its best in
the empty spaces a screen leaves.

### Cadence: the layer does not run at the frame rate

An animated full-screen true-colour field changes almost every cell every frame,
which defeats the presenter's diff and is exactly how the first prototype spent a
tenth of a core drawing one screen. Three rules keep the layer cheap:

1. **It is cached.** The field is resolved only when the clock crosses a tick or
   the area changes. Between ticks `paint` blits a canvas that is byte-identical,
   so the diff writes nothing.
2. **The clock is quantised.** The sample time comes from a whole tick index, never
   from a continuous elapsed time, so two resolves inside one tick are identical
   bit for bit and there is nothing to flicker.
3. **The drift is bounded.** `Drift` names the rows allowed to change, and a tick
   repaints only those. The default sky drifts in its top quarter four times a
   second; the rows below it are drawn once and never retransmitted.

A screen hands the loop the layer's cadence through `Screen::next_frame`, using
`Atmosphere::next_update`, and reports `Drift::Still`'s `None` when the player has
asked for reduced motion. The atmosphere never makes a screen `is_animating`.

### Degrading

`Rgb::to_color` reduces the field to what the terminal can show: true colour, the
6×6×6 cube, or the nearest classic colour. The result is never `Color::Default`,
whatever is asked for, because a cell the field covers must own its colour or the
user's own background would show through the game.

### Smoothness is a constraint, not a hope

A gradient steeper than roughly six units of colour per vertical sample reads, at
this resolution, as a stack of horizontal lines. The first attempt at the horizon
glow changed by twenty and was plainly banded in a real capture. Two things fixed
it: **one** continuous ramp instead of a sky ramp meeting a ground ramp at the
horizon, and a glow that falls off in **both** axes — the vertical falloff spread
over half the height, and a horizontal one so that a light is a light rather than
a bright slab stretched the full width. `a_night_sky_shows_no_bands` holds the
shipped sky to seven units per sample at three terminal sizes.

## 8. The optional pixel interface

`--pixel-ui` (or `GRIMMOIRE_PIXEL_UI=1`) enables a small software pixel renderer
for **interface pieces only**: the dialogue portrait, and the study's portrait
panel. It is off by default, the game never depends on it, and the default path
emits no graphics sequence.

The pixel layer is a flat RGB canvas, sprites authored as ASCII, and a PNG encoder
(`render::png`) that hands the bytes to Kitty. The PNG container is hand-written —
signature, `IHDR`, `IDAT`, `IEND`, Paeth filtering, CRC-32 — and only DEFLATE
comes from a dependency (`fdeflate`, chosen for PNG scanline data). Nothing in the
project decodes an image: Kitty does that.

## 9. Verification

**Visually verified in a real Kitty session** (0.49.2 at font size 15, so about
150×36 cells, driven through Kitty's own remote-control socket and captured with
`grim`, then inspected):

| Check | Result |
| --- | --- |
| Launches into the playable world | Ruined Chapel, world edge to edge |
| The world is ASCII | a cathedral: nave, colonnade, side chapels, apse, graveyard |
| The mage is a figure, not a letter | silver hair, a face with two eyes, an indigo robe with a gold clasp, and a crystal staff |
| Ysolde is distinct | a cowl with a trim rim and two violet points of light; no skin at all |
| Candles and motes | flames gutter along the wall faces; dust drifts over the floor |
| The inlay reads | the crossing rose and the processional are visible as deliberate marks |
| One press is exactly one step | `70,44` → `70,43` for one `w` |
| A burst cannot sprint the player | presses inside one cadence move one tile |
| Sprint is real | the pose changes, the wake appears, and the HUD says `sprint »` |
| Blocked movement is visible | the cell that refused the step pulses and underlines |
| Travel between both areas | chapel → cellar (`20,12`) → chapel |
| The cellar is a different place | four chambers, one flooded, candle-lit |
| Dialogue rises and settles | the panel slides in, brightening, and slides away on dismiss |
| The world stays visible around it | the mage and Ysolde are both legible behind the panel |
| No graphics are sent by default | asserted by the smoke test |

**Frame rate, measured** (a pseudo-terminal at 200×50, with `GRIMMOIRE_FRAME_STATS=1`
reporting what the loop actually did): **64 frames/s walking east, 60 walking
north, 8.7 idle.** The idle figure is the ambient tick, deliberately slow.

**Pseudo-terminal smoke test** (`tools/smoke.sh`, 12 checks, all passing): enters
the alternate screen, hides the cursor, composes the ASCII world, draws the
player, shows the controls, suspends below the minimum viewport, resumes after the
resize, shows the cursor, leaves the alternate screen, **sends no graphics by
default**, cleans up images, and exits 0. It runs under a scratch `XDG_DATA_HOME`,
so it can never read or write a real save.

**Unit tests:** 373, including the scenery renderer (structure, variation
stability, water motion, inlay placement, every glyph one column wide, every cell
shaded, figures in several inks, a pulse that changes style and never glyphs), the
animation model (stride per step, settling, blinking, sprint, and identical
results for the same elapsed time in different increments), the movement state
machine, the presenter's diffing, the dialogue's rise and self-dismissal, the
camera, the minimum viewport, the game's rules, and the save format.

**Not verified, and not claimed:** behaviour under tmux/zellij; other Kitty-family
terminals; macOS or Windows; and the frame rate in a *real* Kitty window under
sustained load, which was measured only through a pseudo-terminal.

## 10. Measurements

From `cargo run --release --example bench` on this machine, at a 120 × 40 frame
with the world composed and shown. Every number is measured.

| Operation | Cost |
| --- | --- |
| Compose a frame (scenery + interface) | 0.27 ms |
| Present a full redraw | 0.4 ms, 4800 cells in 40 runs, 43 KiB |
| Present an unchanged frame | 0.05 ms, **16 bytes** |
| Present one movement step (camera scrolled) | 0.7 ms, 3275 cells in 861 runs |
| Idle CPU | 0.5% of one core, ~11 KiB/s |
| Walking CPU | ~2% of one core |

Composition did not get slower despite the figures, the inlay, the candles and the
motes: all of it is arithmetic on cells that were being written anyway, and the
draw passes visit only the tiles inside the view.

## 11. Migration path and non-goals

- **More art** means more glyph tables, more legend characters and more neighbour
  rules in `ui::scenery`; nothing else changes.
- **More effects** mean more entries in `ui::effects` and a style rule; the layer
  is already there.
- **A tile-drawing DSL** would be the natural next step if the scenery rules grow
  past a handful of matches.
- **Deferred renderer work**: wide/zero-width glyph continuation cells, and
  damage-rectangle tracking instead of per-cell diffing.

Non-goals: no terminal emulator; no widget toolkit, retained scene graph, or ECS;
no second terminal backend; no GPU window; no image in the default path. Gradients
and photographic art are not attempted, because the identifier is a glyph grid.
