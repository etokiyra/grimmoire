# Renderer investigation: a true-colour atmosphere layer

**Status: investigation and feasibility prototype. Nothing in the game has
changed.** The prototype is `examples/skyline.rs`, run with
`cargo run --release --example skyline`.

## 1. Why the current renderer looks the way it does

Not a list of defects — the model itself has four limits, and every visual
disappointment in the last three phases comes out of one of them.

1. **One glyph, one colour, one flat background per cell.** `Cell` holds a
   character and a `Style`; `Canvas::set` inherits the background from whatever
   was there. So a cell can hold *a* colour, not a *field*. A sky is impossible
   to express: there is nowhere to put a gradient.
2. **The palette is a table of roles, not a ramp.** `Palette` is ~40 named
   colours. Roles are good for "a wall is this grey"; they are useless for
   "these two points are 3% apart in luminance". Smooth light needs ramps.
3. **A tile either is scenery or is not.** `scenery::tile_art` is a `match` over
   `Tile`. There is no background/midground/foreground model, no way to
   accumulate light from several sources, and no layer under the terrain.
4. **256-colour indexed output.** Fine for flat art, and the reason every
   "gradient" in the game is really three discrete steps.

The last three phases all attacked the *content* of that model — more sprites,
more materials, more effects — and the ceiling was the model, not the content.
That is why the game still looks like glyphs on a dark field.

## 2. VinZ: what it actually does, and its licence

VinZ is a single 746-line C file (`vinz.c`), GPL-3.0, built with `make`, no
dependencies but `-lm`.

Its rendering core is three things:

- **Half-block output.** Per cell it writes
  `\033[48;2;R₂G₂B₂m\033[38;2;R₁G₁B₁m▀`: the *upper* half block's foreground is
  the colour of the top pixel and its background is the colour of the bottom
  pixel. Two vertical pixels per cell.
- **One buffer, one write.** It builds the whole frame into a `char*` with
  `snprintf` and writes it in a single `write(2)`, so there is no tearing.
- **A fixed cadence with a full redraw.** `usleep(33000)` — about 30 fps,
  everything redrawn every frame, no diffing.

The rest of it — ten 2D styles, eight raymarched 3D styles, twenty palettes, an
interactive UI, a seed system — is a *visualiser*. There is no world, no
sprites, no text layout, no input beyond "change the style", and its terminal
handling is its own. None of that is applicable to a game, and the raymarcher
would replace the ASCII world outright, which is off the table.

**What is worth taking is the technique, not the code.** The half-block idea is
a standard terminal technique, widely documented, and it is around thirty lines
to implement against our own `Canvas`. I wrote it from the concept: nothing in
the prototype is copied, ported or adapted from `vinz.c`.

**Licence.** The brief says Grimmoire's licence is undecided. It is not:
`Cargo.toml` carries `license = "GPL-3.0-or-later"` and `LICENSE` holds the
GPLv3 text, both settled in an earlier phase. VinZ is GPL-3.0. So all three
options below are available, and **no permission is needed to use the idea,
only attribution if code were copied** — which it was not.

| Option | Cost | Verdict |
| --- | --- | --- |
| Copy or port `vinz.c` | C in a Rust project, its own loop, its own terminal handling, no diffing | **Rejected.** Nothing in it survives contact with a game loop. |
| Depend on a VinZ binary | A process boundary, a foreign frame, no shared canvas, a system dependency | **Rejected.** |
| Implement the technique in our renderer | ~30 lines against `Canvas`, no dependency | **Recommended**, and prototyped below. |

## 3. Proposed architecture

One new layer, underneath everything, and no change to the layers above it:

```
  layer 3   UI          glyphs, opaque cells           (HUD, menus, panels)
  layer 2   figures     the fixed sprites, per-part inks
  layer 1   effects     motes, sparks, particles
  layer 0   scenery     architecture, glyphs
  layer -1  atmosphere  procedural true colour, drawn as half blocks
```

**The composition rule is the one the canvas already has**: a draw that leaves
`Style::bg` unset inherits whatever was underneath. So the atmosphere writes
`▀` with a foreground (the top half's colour) and a background (the bottom
half's), and every layer above simply overwrites the *character and foreground*
while keeping the background — which is exactly what `Canvas::set` already does
for the light pool. No new compositing model, no alpha, no z-buffer.

Why this is the right shape:

- **It keeps glyphs.** Characters, walls and text are untouched: they are still
  one glyph in one cell, still drawn from the fixed sprite library, still
  readable. The atmosphere only occupies cells nothing else wants.
- **It doubles the resolution only where resolution is cheap.** The sky gets 2×
  vertical resolution because it has no glyphs to carry; a wall keeps one cell,
  because a wall is a glyph.
- **It is a background, so it cannot obscure anything.** The failure mode the
  brief warns about — hard rectangular light patches — comes from writing a
  bright *background* where glyphs are; this design puts the colour under them
  and the glyph's own ink on top, which is what the chapel's light pool already
  does successfully.

## 4. The prototype

`examples/skyline.rs` — a `Screen`, driven by the real `App`, the real
`Terminal`, the real `Presenter` and the real `Canvas`. That is deliberate: the
prototype proves *integration*, not just a technique. It is a title-screen
composition:

- a true-colour sky: deep indigo → violet → magenta → a horizon glow, with an
  aurora band and value noise so the ramp is not a machine gradient;
- a moon with a body, a halo and a lit rim;
- a twinkling star field;
- distant hills, then a cathedral authored as a glyph silhouette — two spired
  towers, a gabled nave, a rose window, lit lancets, a lit doorway, columns;
- ninety rising motes in warm true colour;
- the supplied MC Wizard sprite, unmodified, standing before the doors;
- a gradient wordmark and a menu on its own scrim.

Screenshots (actual Kitty captures, not mockups):

- `docs/images/prototype-skyline.png` — the current iteration.
- `docs/images/prototype-first-pass.png` — the first pass, kept because the
  difference between them is the iteration.

### What the iteration fixed

The first pass had three problems, found by looking at it:

1. **The horizon glow filled the bottom third** and washed the ground out. The
   gradient now peaks *at* the horizon and falls away below it.
2. **The menu was unreadable over the building.** It now sits on a scrim of its
   own, on the left, so the interface wins against its own scenery.
3. **The figure was lost in the bright band.** It moved up onto the dark ground
   and gained a pool of light at its feet.

## 5. Measurements

Measured on this machine, 150×37 cells, true colour, through the real
`Presenter`:

| | Prototype | Game today |
| --- | --- | --- |
| First frame | 2.69 MB | — |
| Steady state | ~4.7 MB per 2 s (~2.4 MB/s) | ~2% of a core walking, 0.5% idle |
| Frame rate | ~35 fps, full redraw | 60 fps, diffed |
| Unchanged frame | n/a (never unchanged) | 16 bytes |
| CPU | **10% of one core** | ~2% walking |
| Resize | survives 80×24 and back | survives |
| Terminal restore | clean | clean |

**The important finding is negative and it is about the diff.** An animated
true-colour field changes *almost every cell every frame*, so the presenter's
diffing — the thing that makes the game cost half a percent of a core — stops
working. The prototype pays 5× the CPU of the whole game to draw one screen.

Three ways out, in order of preference:

1. **Quantise the sky's animation** so neighbouring frames produce identical
   cells over most of the screen; the diff then skips them and only the moving
   band is written.
2. **Give the atmosphere its own slow clock.** A sky does not need 60 fps; the
   gradient can update a few times a second while sprites and effects keep 60.
3. **Freeze the gradient and animate only a band** — the aurora, the stars, the
   glow at the horizon.

All three are cheap and none of them touches the layer model. The prototype
currently animates everything every frame, which is the worst case.

## 6. Recommendation

**Adopt the technique, not the program.** Specifically:

- **Implement the atmosphere layer in `render`** as an optional pass that writes
  half-block cells with true colour, and only where `ColorSupport::TrueColor` is
  reported. On a 256-colour terminal it degrades to sampling the field once per
  cell into the nearest indexed colour — the same picture with less smoothness.
- **Do not adopt VinZ's loop, buffer strategy, cadence or terminal handling.**
  Ours are strictly better: we diff, we schedule from a deadline, and we restore
  the terminal on every exit path.
- **Licence: no action needed.** Grimmoire is already GPL-3.0-or-later; VinZ is
  GPL-3.0; and nothing was copied, so not even attribution is owed — though it
  is recorded here as the source of the idea.

The evidence for adopting it: the prototype reaches a visual quality the current
renderer cannot reach *by construction* — a smooth sky, a glow with no banding,
and atmosphere behind architecture — in about 350 lines of example code that use
the existing `Canvas`, `Presenter` and `Screen` unchanged. The cost is CPU, and
the cost has a known fix.

## 7. Staged migration, with rollback

| Stage | Work | Rollback point |
| --- | --- | --- |
| 1 | `render::Atmosphere`: a trait for a procedural field sampled at `(u, v, t)`, plus the half-block writer, with unit tests on cell output. No game change. | Delete the module. |
| 2 | Apply it to the title screen only (`ui::title`), behind `ColorSupport`. The game keeps its current look. | Revert `title.rs`. |
| 3 | Add the slow clock and the quantisation, and re-measure. **Gate: must stay under 3% of a core.** | Revert the clock change. |
| 4 | Give the six places atmosphere fields: a sky above the chapel, mist in the woods, water in the cellar, rune-light in the shrine, spectral haze in the catacombs. | Per-area, each independent. |
| 5 | Retire the current two-step light pool in favour of the field. | Keep the pool as a fallback for 256-colour terminals. |

**Risks.** The CPU cost above; visual noise if an area's field is too busy;
true-colour terminals are assumed (the fallback must be exercised, and the smoke
test should assert the game still runs on a 256-colour terminal). None of them
threaten the game's correctness: the atmosphere cannot touch collision, the
world model or the save.

## 8. Ideas this investigation produced that are worth keeping

Independent of the atmosphere layer, and all implementable in the current
renderer:

1. **A scrim behind every overlay.** The prototype's menu only became readable
   once it had one. The journal, help and conversation panels should have the
   same, which would also let them sit over busy scenery without shrinking.
2. **Interior layout in the dialogue plate.** The portrait is currently
   top-left in a large plate; the prototype's composed panel suggests the
   portrait and the text should share a designed frame rather than a rectangle.
3. **A gradient wordmark.** The prototype's letter-by-letter colour ramp costs
   nothing and looks markedly better than one flat colour.
4. **A light pool drawn at a character's feet**, which the prototype uses to
   sit the wizard on the ground. The game draws a single dark shadow row.
5. **A "slow layer" concept in the frame loop**: layers that update at their own
   cadence rather than the frame's. Useful well beyond the sky.
6. **True colour for the existing art.** The sprite inks are 256-colour
   indices; `ArtInks` could carry RGB and reduce for basic terminals, which
   would let a character be lit by the scene rather than painted with it.

## 9. Stage 1 outcome: the layer, production-ready

Stage 1 of the migration above is done. The layer is `ui::atmosphere`; the game
does not use it yet. This section reports what was built and what was learned,
and corrects one number from the investigation.

### What the composition tests proved

**No change to the rendering interface was needed.** The rule the canvas already
has — a draw that leaves its background at `Color::Default` inherits what is
underneath — decides the whole thing, and the module's tests assert each case
rather than assuming it:

- An open cell is a `▀` carrying **two** samples, and they differ.
- A glyph drawn over the field keeps its own ink and the **lower** sample as its
  background. The upper sample is gone with the glyph slot.
- A glyph with its own background replaces both samples.
- A blank space is drawable, so it displaces the half block and leaves a flat
  cell in the lower sample's colour. Text therefore flattens the field where it
  lands; that is documented rather than wished away.
- A sprite's non-space glyphs all survive with their own ink over the field.

The practical error this leaves is bounded by half a row of the ramp — under
three units of colour at the sizes in use. The layer does **not** provide two
independent samples under a glyph, and nothing claims it does.

### The correction: the first measurement was wrong

The investigation reported the prototype at "~35 fps" by dividing bytes by cell
size. Measured properly, through a pty at 150×37, the old prototype ran at
**6.6 frames/s**, wrote **1.34 MB/s**, and cost **8.5% of one core**: it
recomputed and retransmitted the whole true-colour field every frame, so the
presenter's diff could never skip anything. The bytes estimate was right; the
frame rate derived from it was not.

### What changed to make it cheap

1. The field is **cached** and resolved only when the clock crosses a tick or the
   area changes. Between ticks `paint` blits a byte-identical canvas, so a
   settled field costs **zero bytes** — confirmed in a real Kitty session, where
   the still mode wrote nothing at all after its first frame.
2. The clock is **quantised** to a whole tick index, so nothing can flicker
   inside a tick and two resolves are bit-identical.
3. `Drift` **bounds** which rows may change. The shipped sky drifts in its top
   quarter four times a second.
4. The update cadence is the layer's own: a screen returns
   `Atmosphere::next_update` from `Screen::next_frame`.

### The correction that mattered visually: banding

The first capture of the shipped sky, sampled down one column, changed by **9 to
20 units of colour per half-row** in the glow, which reads as a stack of
horizontal lines; the sky above it changed by 1–2 units and was perfect. Two
causes, both designed out:

- **A seam at the horizon.** A sky ramp above the horizon meeting a separate
  ground ramp below it is a hard horizontal edge by construction. The field is now
  **one** continuous ramp, with the ring of stops around the horizon shaping it.
- **A glow that was a function of the row alone.** However smooth its falloff,
  that is a bright slab across the full width. The glow now falls off in `u` as
  well, so it is a light rather than a band.

Re-measured on the new capture, the worst step down a column is **5 units** (the
only larger jump in that column is a star, which is a point and is meant to be
one). `a_night_sky_shows_no_bands` holds the shipped sky to seven units per
sample at 120×29, 80×22 and 200×50.

### Measurements, after

Two methods, because one number is not enough:

- **Headless** (`cargo run --release --example skyline -- --bench`), which
  composes, diffs and presents into a `Vec<u8>` with no terminal at all. Bytes and
  write counts are exactly reproducible; times vary by 20–30% run to run on this
  machine, so they are reported as a range.

  | Mode | µs/frame | cells written/frame | bytes/frame |
  | --- | --- | --- | --- |
  | `bare` (no atmosphere, the same animation) | 86–110 | 15.7 | 543 |
  | `live` (a drifting band at 4 Hz) | 167–189 | 112.2 | 3 283 |
  | `walk` (the whole field at 12 Hz) | 450–508 | 344.4 | 9 857 |
  | `heavy` (the whole field at 30 Hz) | 884–1 011 | 560.8 | 19 039 |

  The atmosphere's own contribution in normal idle animation is `live − bare`:
  **57–103 µs per frame**, which at 60 frames a second is **0.3–0.6% of one
  core**. The acceptance target was no more than about 3%.

- **End to end** through a pty at 150×37, one core as the denominator, over
  ≥10-second windows:

  | Mode | frames/s | KiB/s | CPU, % of one core |
  | --- | --- | --- | --- |
  | `still` | — (idles) | 0.0 | 0.0–0.2 |
  | `slow` (the field alone) | 6.4 | 53 | 1.2–1.3 |
  | `bare` | 60.2 | 41 | 2.7–4.1 |
  | `live` | 58.0 | 185 | 4.1–7.8 |
  | `walk` | 57.9 | 569 | 8.4–10.5 |
  | `heavy` | 41 | 915 | 9.4–12.2 |

  The `bare` and `live` spreads overlap because CPU is counted in 10 ms ticks, so
  the percentage is the weaker of the two measurements. What is stable is the byte
  volume and the frame rate: the game holds **58 of its 60 frames a second** with
  the atmosphere running.

  **Worst case, not averaged away:** `heavy` — the whole field drifting thirty
  times a second with 240 motes and a walking figure — costs 9–12% of one core
  end to end and holds 41 fps under a pty. It is available, it is not the default,
  and it is not hidden behind an average.

### Resizing and small terminals

Driven through a pty through 150×37 → 80×24 → 40×12 → 20×6 → 200×50 → 1×1 →
150×37: the process survived every one, repainted at each (a resize is a full
redraw), and exited cleanly with its frame report. At 101×24 the scene is still
coherent; below the application's own 80×24 minimum the existing suspend screen
appears, and the atmosphere is not even resolved.

### Screenshots

Real Kitty captures, fullscreened, `grim`, no palette reduction:

| File | What it shows |
| --- | --- |
| `docs/images/atmosphere-field.png` | the field alone, no foreground glyphs |
| `docs/images/atmosphere-scene.png` | the field beneath scenery and the supplied wizard |
| `docs/images/atmosphere-overlay.png` | the field around an opaque dialogue plate |
| `docs/images/atmosphere-small.png` | the same scene at 101×24 |
| `docs/images/atmosphere-heavy.png` | the heaviest mode, a second animation state |

`docs/images/prototype-skyline.png` from the investigation is kept beside them for
comparison.

### What is honestly not done

- The layer is **not wired into the game**. No screen composes it, so the game's
  own look is unchanged, which is what stage 1 asked for.
- The 256-colour and 16-colour paths are asserted by unit test, not photographed:
  Kitty was driven in true colour throughout.
- The moon's disc has a hard edge by design; a shape is allowed one, and the
  banding test excludes the moon and the stars for that reason.
- CPU percentages are of **one core**. This machine has more than one, and the
  numbers were taken while it was otherwise busy, which is where the spread comes
  from.

### The next safest migration step

The same as the plan's stage 2, and still reversible: apply the layer to the
**title screen only**, behind `ColorSupport::TrueColor` with the nearest-indexed
fallback, and re-measure. The title is the right first host because it has a large
empty sky, no text over the field except its stars, and nothing the player must
act on. Do **not** move to per-area fields before the title's cost has been
measured on a real terminal.
