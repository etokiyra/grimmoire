# Changelog

All notable changes to Grimmoire are documented here. The project is in early
development and has **no tagged release yet**; everything is under `Unreleased`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project intends to follow [Semantic Versioning](https://semver.org/) once
it starts releasing.

## [Unreleased]

### Added

- **Renderer core.** `Canvas` drawing primitives with clipping and saturation
  (styled text, fills, ASCII and titled borders), `Canvas::crop` / `Canvas::blit`
  composition with deterministic paint-order layering, and a clipped,
  local-coordinate `CanvasView` (including nested views) for independent visual
  regions.
- **Text width policy.** `render::glyph` classifies glyphs with `unicode-width`
  and enforces an ASCII-first rule: glyphs that are not exactly one column wide
  are dropped so the cell grid cannot desynchronise. Added `unicode-width` as a
  direct dependency.
- **Diffed presentation.** `terminal::Presenter` writes only changed cells,
  grouped into contiguous runs, and clears plus re-baselines on resize or
  invalidation. It writes through any `Write`, so its output is tested without a
  terminal.
- **Animation system.** `anim::Animator` and `anim::Animation`: durations,
  delays, easing curves, concurrent keyed tracks, completion, cancellation, and a
  manually advanced clock so progress is deterministic under test.
- **Screen stack.** `ui::Screen` and `ui::ScreenStack` with push/pop, input routed
  to the active screen, and bottom-to-top composition; plus a full-screen curtain
  transition on screen changes.
- **Capability detection.** Colour support (16/256/truecolour) selects between a
  portable and a richer palette; a best-effort graphics-protocol hint is reported
  in diagnostics only.
- **Responsive layouts.** Two-column, stacked, and minimal modes recomputed from
  frame bounds each frame, with graceful degradation on small terminals.
- **Interactive renderer showcase** replacing the Phase 1 title screen: the
  Grimmoire banner with a reveal animation, three animated ASCII scenes
  (Emberfall, Sigil, Starfield), a pulsing highlight, a scene-change wipe, an
  information panel with live diagnostics, a status bar, and a help &
  diagnostics overlay.
- **Terminal-independent input.** `input::Key` / `input::Event`, with the backend
  translating library events (and skipping mouse, paste, and key releases).
- Tests across the new architecture: clipping and bounds, layer order and
  overlap, diff output and determinism, animation progression and cancellation,
  screen push/pop and input routing, resize and small-screen layouts.
- `docs/renderer-design.md`: backend evaluation, graphics-protocol investigation,
  architecture, constraints, measured numbers, and extension points.

### Changed

- Split terminal lifecycle (`terminal::Terminal`) from frame presentation
  (`terminal::Presenter`).
- Rewrote the application loop around an explicit clock, idle-aware polling, and
  a reusable frame buffer.
- Replaced the ad-hoc colour constants with a capability-aware `Palette`.

### Removed

- The Phase 1 `ui::title` screen, replaced by the showcase.

### Notes

- No license has been chosen yet, so none is included.
- The story, setting, characters, and detailed mechanics remain undecided.
- No graphics protocol is implemented; detection is informational only.

## [0.0.0] - Phase 1

### Added

- Cargo application project named `grimmoire` (binary crate `grimmoire`), with
  `Cargo.lock` committed.
- Terminal backend around `crossterm` entering/leaving raw mode and the alternate
  screen, with restore on normal exit, on the error path, and from a panic.
- Pure, cell-based rendering primitives: `Canvas`, `Cell`, `Style`, `Color`, and
  `Rect`, with ASCII border drawing and centred-text helpers.
- A title screen with an interactive menu and a status line.
- An input → update → compose → present loop.
- Unit tests, GitHub Actions CI (formatting, Clippy, tests, release build), and
  documentation: `README.md`, `AGENTS.md`, `ROADMAP.md`, `CHANGELOG.md`.
