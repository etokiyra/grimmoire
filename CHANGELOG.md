# Changelog

All notable changes to Grimmoire are documented here. The project is in early
development and has **no tagged release yet**; everything is under `Unreleased`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project intends to follow [Semantic Versioning](https://semver.org/) once
it starts releasing.

## [Unreleased]

### Added — exploration prototype

- A pure world model (`src/game/`): bounded tile areas, terrain, entities,
  doorways, and a player position with a facing direction, with no rendering or
  I/O.
- Two connected, hand-authored areas — the Ruined Chapel and the Sunken Cellar —
  with distinct layouts, authored as ASCII rows.
- Tile-based movement (`w`/`a`/`s`/`d` and arrow keys) with collision against
  terrain, area boundaries, and entities, plus turning to face a tile without
  moving.
- A deterministic interaction system (`e`): examine objects, speak to NPCs, and
  use doorways, with clear feedback when there is nothing to use.
- An NPC dialogue panel implemented as a pushed screen, so input routing prevents
  movement and re-triggering while it is open.
- A camera that follows the player and centres areas smaller than the view, with
  a responsive layout and a graceful fallback for very small terminals.
- A status area showing area name, position, facing, discovery counts, and
  contextual prompts, plus a one-line feedback strip.
- Minimal versioned persistence (`src/save.rs`) of the current area, position, and
  discovered areas, saved on quit and on travel. Loading is fail-soft: anything
  unrecognised starts a new game, and writes are renamed into place atomically.
- A curtain effect when moving between areas.

### Added — rendering core

- `Canvas` drawing primitives with clipping and saturation (styled text, fills,
  ASCII and titled borders), `Canvas::crop` / `Canvas::blit` composition, and a
  clipped, local-coordinate `CanvasView` for independent visual regions.
- An enforced ASCII-first text width policy (`render::glyph`) using
  `unicode-width`, so the cell grid cannot desynchronise.
- Diffed presentation (`terminal::Presenter`): only changed cells are written,
  grouped into contiguous runs.
- A time-based animation system (`anim`) with durations, delays, easing,
  concurrent keyed tracks, completion, cancellation, and a manually advanced clock.
- A screen stack (`ui::Screen`) with push/pop, input routed to the active screen,
  bottom-to-top composition, and a curtain transition on screen changes.
- Runtime capability detection selecting between a portable and a richer palette.
- Responsive layouts and an idle-aware application loop.
- Tests across the renderer, animation, screen stack, and presenter, plus
  `docs/renderer-design.md`.
- The renderer showcase, retained as a development screen reachable with `F2`.

### Changed

- The application now launches directly into the playable exploration prototype;
  the renderer showcase is a secondary screen rather than the entry point.
- The help overlay takes a per-screen list of key bindings instead of a fixed one.
- Screens gained an `on_exit` hook so they can persist state when the application
  shuts down, and the stack notifies all screens.
- Input events carry an auto-repeat flag; the backend reports it, discrete actions
  ignore it, and movement is paced by a fixed step interval so held keys behave
  the same on every terminal.
- The renderer showcase now leaves its screen with `Esc` (pop, or quit at the
  root) instead of quitting directly.

### Removed

- The Phase 1 title screen (superseded by the showcase in the previous milestone).

### Notes

- No license has been chosen yet, so none is included.
- The story, setting, characters, and detailed mechanics remain undecided. All
  current area names, entities, and dialogue are placeholder content.
- No graphics protocol is implemented; detection is informational only.
- There is no combat, inventory, quest, or skill system.
