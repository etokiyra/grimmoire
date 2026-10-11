# Roadmap

This roadmap is organised into practical phases. It is a direction, not a
promise, and it will change. **The story, setting, characters, and detailed
mechanics are explicitly undecided** and are not committed to by anything below.

Each phase should ship something small, correct, and verifiable.

## Phase 0 — Project foundation (done)

- Cargo application project named `grimmoire`, committing `Cargo.lock`.
- Terminal backend around `crossterm`: raw mode, alternate screen, input,
  guaranteed restore.
- A small, pure, cell-based renderer.
- Unit tests, formatting/Clippy/test/release gates, and CI.

## Phase 1 — Rendering core (done)

- Explicit frame composition separated from terminal output.
- Drawing primitives with clipping: styled text, fills, ASCII borders, subregions.
- A clipped, local-coordinate `CanvasView` for independent visual regions, and
  `crop` / `blit` composition with deterministic paint-order layering.
- An enforced ASCII-first text width policy (`unicode-width`), so the cell grid
  cannot desynchronise.
- **Diffed presentation**: only changed cells are written, grouped into runs;
  unchanged frames write nothing. Full redraw on resize or invalidation.
- A time-based animation system with durations, delays, easing, concurrent
  tracks, completion, cancellation, and a manually advanced, testable clock.
- A screen stack with push/pop, input routed to the active screen, and layered
  composition.
- Responsive layouts (two-column, stacked, minimal) recomputed from frame bounds.
- Runtime capability detection (colour depth) selecting the palette; a
  best-effort graphics-protocol probe reported in diagnostics only.
- An interactive renderer showcase with four visible time-based effects and a
  help & diagnostics overlay.
- Design document: `docs/renderer-design.md`.

Still open from this area (deliberately deferred, not forgotten):

- Wide/zero-width glyph support via continuation cells.
- Damage-rectangle tracking rather than per-cell diffs.
- A reusable widget vocabulary; the showcase draws directly with primitives.

## Phase 2 — Interaction and shell (partly done)

Done:

- Screen/mode stack with push/pop and a transition effect.
- Capability detection and responsive handling of resizing, including very small
  terminals.

Remaining:

- Configurable key bindings loaded from a config file.
- A settings screen and a screen/mode history (back, not just pop).
- Consistent small-terminal and resize behaviour across every future screen.
- Optional: request terminal focus/colour-depth queries rather than env hints.

## Phase 3 — World and exploration

- A world/area model and a minimal map traversal loop.
- ASCII-art environment tilesets and scene composition.
- Text-log/status presentation suitable for exploration and dialogue.

## Phase 4 — Dialogue and story

- A dialogue system with ASCII portraits and branching choices.
- Story, setting, and characters defined here (currently undecided) with
  authoring-friendly data.

## Phase 5 — Magic combat

- Turn-based, magic-only combat: spell resolution, resources, and enemy behaviour.
- Combat presentation built on the animation system.

## Phase 6 — Progression and items

- Hybrid progression: discoveries/story milestones plus gameplay mastery.
- A skill tree.
- Items focused on exploration, puzzles, quests, and storytelling.

## Phase 7 — Persistence and release

- Persistent save files with versioning and migration.
- A real release strategy: packaging, versioned tags, and a release pipeline
  (deliberately deferred until then).
- Decide and add the project license.

## Cross-cutting, ongoing

- Keep dependencies minimal and justified (currently `crossterm` and
  `unicode-width`).
- Keep the render/animation/terminal boundaries intact; see `AGENTS.md`.
- Keep documentation, the design document, and the changelog current.
