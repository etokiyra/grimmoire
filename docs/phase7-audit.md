# Phase 7 audit — before any change

Recorded from the code at `5873bb3` before editing, so later claims can be checked.

## How frames are scheduled today

- `App::event_loop` computes `continuous = stack.is_animating() || transition active`.
  If continuous it polls input for **16 ms**; otherwise for `min(stack.next_frame(),
  200 ms)`. It then advances by the real elapsed delta (capped at 60 ms) and
  composes when `dirty || had_event || frame_due`.
- `frame_due` is `continuous || stack.next_frame() <= elapsed_since_last_frame`.
- So the loop is **not capped at 30 FPS, and not at 60**: it composes on change,
  plus whatever the screens request. There is no drift correction: the 16 ms wait
  is *added to* composition time, so a "60 FPS" loop actually runs at
  `1 / (16 ms + compose)`.

## What actually animates

- `anim::Animator` exists, is time-based and well tested, and is used by the app
  for the screen-transition curtain and by the presentation study.
- The game world animates from **one free-running scalar**: `ExplorationScreen.phase`,
  advanced every `update`. Water ripples, and the character frame is
  `(phase * 8.0) as usize % 2`.
- `ExplorationScreen::is_animating()` returns true **only during area travel**.
  Walking therefore requests no continuous frames at all: the walk cycle advances
  only when input events or the 220 ms ambient tick happen to compose a frame. The
  walk cycle rate is unrelated to the movement cadence (which is 60–133 ms per
  step, i.e. 7–17 steps a second).
- The world has no motes, no flames, no environmental motion beyond water, and no
  falloff other than the three-step shade ramp. The NPC has a two-frame sway
  driven by the same global phase.

## Input, and what it guarantees

- Kitty keyboard protocol is enabled when the terminal answers, so press / repeat /
  release are all visible; the protocol is popped on every exit path.
- Movement is event-driven and per-axis paced from one screen speed
  (`WALK_SPEED` 150 px/s, sprint 300), so both axes cross the screen at the same
  rate. `MIN_INTERVAL` is 45 ms. There is no timer-driven stepping.
- `pump_events` blocks for the timeout on the *first* event, then drains the rest
  with zero wait, so a burst cannot starve rendering for more than one frame.

## Rendering path

- `ui::scenery` draws one glyph per cell: neighbour-aware walls, positional
  variation, three-step ambient shading, water that moves.
- Characters are **single-colour**: `draw_art_at` takes one `Color`, so a figure
  cannot have hair, skin, robe, trim and eyes in different colours.
- The player and the NPC are both 3 cells tall; the player is 3 wide, the NPC 5.
  They are generic: `(o)` under a caret, and `(oo)` over `|###|`.
- Draw order is terrain → props → characters. There is no feedback layer: a
  blocked step and a successful interaction are indistinguishable on screen.
- Off-screen culling is done inside `screen_of` for terrain but props and
  characters are still iterated over the whole area.

## Measured baseline

`cargo run --release --example bench` at 120×40: compose 0.28 ms, full redraw
0.7 ms (4800 cells, 40 runs, 37 KiB), unchanged frame 0.11 ms / **16 bytes**.

So a 60 Hz animation loop would cost roughly `60 × 0.28 ms` ≈ 17 ms of CPU a
second during composition, plus whatever cells actually changed. That is
affordable; the reason the game does not do it today is that nothing has asked
for it.

## What this phase therefore has to fix

1. Give characters real, multi-colour, directional art with distinct identities.
2. Give the world life: motes, flames, shimmer, pulses — layered, culled, and
   never over characters, props or exits.
3. Make animation time-based and *requested*: a 60 Hz loop while something is
   moving, transitions included, with a slower ambient tick when idle.
4. Make the loop deadline-based so 60 Hz means 60 Hz.
5. Give movement feedback: a blocked step and a successful interaction should
   look different.
6. Animate overlays in and out rather than only via the full-screen curtain.
