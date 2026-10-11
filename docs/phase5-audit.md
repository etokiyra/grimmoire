# Phase 5 audit: why the game felt wrong

Recorded before any change, so the fixes can be checked against the diagnosis.

## 1. The world occupied a small fraction of the screen

Verified in code and on screen (a 117x40 kitty window, screenshot inspected):

- `ui::exploration::axis_camera` **centres any area smaller than the view**:

  ```rust
  if area <= view { -((view - area) / 2) } else { clamp around the focus }
  ```

  The Ruined Chapel is 46x16 and the Sunken Cellar 34x12, so at any realistic
  terminal both are drawn as a small island with large empty margins. The world
  covered roughly **17% of the viewport area**.
- `ExplorationLayout::compute` reserved **four rows** of chrome: a header row, a
  message row, and a two-row status bar. Combined with the centring camera, the
  playable area read as a decorative frame around a stamp.

Root cause: **the areas are smaller than the viewport, and the camera was written
to centre rather than to scroll.** No amount of layout tuning fixes this; the
world has to be bigger than the screen, and the camera has to follow.

## 2. Movement felt poor

Verified in code:

```rust
fn try_move(&mut self, direction: Direction) {
    if self.move_cooldown > Duration::ZERO { return; }   // (a)
    if self.world.move_player(direction) { self.move_cooldown = MOVE_INTERVAL; }  // (b)
}
```

- **(a) Input loss.** The 110 ms cooldown gated *every* source. A fresh tap
  inside the window was dropped outright rather than queued, so quick taps -
  the normal way to move a step or two - silently did nothing.
- **(b) One flat rate.** Presses and auto-repeats were treated identically, so
  tapping and holding felt the same and neither could be tuned.
- **No key-release reporting.** `Terminal::enter` never enabled the Kitty
  keyboard protocol, so `KeyEventKind` was always `Press` on a legacy-encoding
  terminal. The `Key::repeat` flag Phase 3 added was therefore **never set**,
  which also meant the "ignore auto-repeat for discrete actions" guard was dead
  code - holding `e` could interact repeatedly.
- **No notion of a held direction**, so nothing could stop promptly on release,
  and there was no basis for a repeat cadence.
- **Movement did not move the world.** Because the camera centred a small map,
  walking around the middle of the room produced no visual change at all. This,
  more than the timing, is what made movement feel dead.

## 3. Incidental findings

- `terminal::capabilities::GraphicsSupport` could not distinguish "no graphics"
  from "could not tell", and the sixel backend additionally required the terminal
  to report cell metrics in pixels, which many terminals (and this environment's
  pty) do not.
- The exploration screen's own 24x8 fallback is unreachable: the application-level
  viewport policy suspends at 80x24 first.
- The theme had accents and text but no separators, focus, interactive, warning,
  or player tokens, so screens invented colours locally.

## Direction taken

- **Kitty is the target.** Kitty 0.49.2 is installed in this environment, so live
  verification is possible. Sixel is removed rather than kept as a second backend.
- **The world becomes larger than the viewport** in both areas, and the camera
  scrolls. The HUD becomes an overlay drawn *over* the world, so the world fills
  the whole viewport.
- **Movement becomes an explicit state machine**: immediate step on a fresh
  press, a tuned repeat cadence while held, prompt stop on release, using the
  Kitty keyboard protocol when the terminal reports event types.
