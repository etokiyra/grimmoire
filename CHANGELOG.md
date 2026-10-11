# Changelog

All notable changes to Grimmoire are documented here. The project is in early
development and has **no tagged release yet**; everything is under `Unreleased`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project intends to follow [Semantic Versioning](https://semver.org/) once
it starts releasing.

## [Unreleased]

### Added — rendering architecture and visual freedom

- **Opaque, application-owned background.** `Canvas::set` now inherits the
  background already in a cell when a draw leaves it at `Color::Default`, so the
  viewport is painted once and every later draw keeps it. This fixes a real
  defect: foreground-only draws used to reset the background, letting the
  terminal's theme (or a transparent desktop) show through text. Verified by
  sampling pixels from a real terminal.
- **Theme roles.** The palette is now expressed as `background`, `surface`,
  `surface_raised`, `border`, `text`, `text_muted`, `accent`, `accent_alt`,
  `effect`, `nature`, `relic`, with role helpers (`surface_style`, `raised_style`,
  `text_style`, `muted_style`, `border_style`, `accent_style`, `effect_style`).
- **`Frame` composition boundary.** Screens compose into a `Frame` that bundles
  the cell canvas with `ImagePlacement`s, separating cell rendering from
  graphics-protocol rendering. Screens record intent and never emit protocol
  bytes.
- **Optional sixel graphics backend.** A pure `sixel::encode` implementation
  (fixed 6×6×6 palette, run-length encoded bands, raster attributes), an
  `ImageBackend` selection rule, CPU-side `Image`s with procedural constructors,
  and a `CellMetrics` report so an image can be sized to a rectangle of cells.
- **Presenter image handling.** Image rectangles are reserved from cell writes,
  images are re-emitted when they change or after a full redraw, the cursor is
  repositioned afterwards, and a disappearing image forces a full repaint so no
  pixels are left behind.
- **Three-valued capability detection.** `GraphicsSupport` distinguishes
  `Unknown`, `Unsupported`, and `Likely(protocol)`. A detected-but-unimplemented
  protocol (kitty, iTerm2) selects no backend rather than a half-working one, and
  `GRIMMOIRE_NO_GRAPHICS=1` forces graphics off.
- **Minimum-viewport policy** (`ui::viewport`, 80×24): below it the application
  suspends normal rendering, paints a legible explanation, swallows input, and
  preserves all game state, resuming automatically when the window grows.
- **The presentation study** (`--study`, or `F3` from the game): an opaque themed
  prototype with a scene panel, an animated ASCII vale (stars, ridge, treeline,
  path, fire, drifting mist), an image-backed portrait plate with an ASCII
  fallback, a theme gallery, live render diagnostics, a reveal transition, and a
  pause control.
- Procedural placeholder portrait art (`ui::art::portrait`).
- Tests for background inheritance and full-viewport opacity, image placement and
  reservation, image re-emission and clearing, backend selection, capability
  states, the sixel encoder (framing, palette limit, RLE, bands, determinism),
  study layout and determinism, and suspend-screen safety.

### Changed

- The theme's field names changed to the role vocabulary; `Palette::muted()` is
  now `muted_style()`.
- `Screen::compose` takes a `&mut Frame` instead of a `&mut Canvas`.
- `PresentStats` reports the number of images drawn.
- `unicode-width`'s default `cjk` feature is disabled (only the unused `*_cjk`
  variants need it), keeping the dependency minimal.

### Notes

- No license has been chosen yet, so none is included.
- The story, setting, characters, and detailed mechanics remain undecided. All
  current area names, entities, dialogue, and art are placeholder content.
- Rendering decisions, the audit that prompted them, measured numbers, and the
  honest limits of the graphics support are recorded in
  `docs/renderer-design.md`.
- Kitty and iTerm2 graphics are not implemented; sixel is verified on `foot`.

### Added — exploration prototype

- A pure world model (`src/game/`): bounded tile areas, terrain, entities,
  doorways, and a player position with a facing direction.
- Two connected, hand-authored areas — the Ruined Chapel and the Sunken Cellar.
- Tile-based movement with collision, turning to face a tile, and a deterministic
  interaction system (`e`) for examining, speaking, and travelling.
- An NPC dialogue panel implemented as a pushed screen.
- A following camera, a responsive layout, and a status area.
- Minimal versioned persistence of the current area, position, and discovered
  areas, saved on quit and on travel, with strict fail-soft loading.

### Added — rendering core

- `Canvas` drawing primitives with clipping, `crop`/`blit` composition, and a
  clipped local-coordinate `CanvasView`.
- An enforced text width policy, diffed presentation, a time-based animation
  system, a screen stack, and responsive layouts.
- `docs/renderer-design.md`.
