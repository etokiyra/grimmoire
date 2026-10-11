# Roadmap

This roadmap is organised into practical phases. It is a direction, not a
promise, and it will change. **The story, setting, characters, and detailed
mechanics are explicitly undecided** and are not committed to by anything below.
All current world content is placeholder.

Each phase should ship something small, correct, and verifiable.

## Phase 0 — Project foundation (done)

- Cargo application project named `grimmoire`, committing `Cargo.lock`.
- Terminal backend around `crossterm`: raw mode, alternate screen, input,
  guaranteed restore.
- Unit tests, formatting/Clippy/test/release gates, and CI.

## Phase 1 — Rendering core (done)

- Cell-based renderer with clipping, styled text, fills, ASCII borders, subregions.
- A clipped local-coordinate `CanvasView` and `crop` / `blit` composition with
  deterministic paint-order layering.
- An enforced ASCII-first text width policy.
- Diffed presentation: only changed cells are written.
- A time-based animation system with a manually advanced, testable clock.
- A screen stack with push/pop, input routed to the active screen, and layered
  composition; responsive layouts; capability detection.
- `docs/renderer-design.md`.

Still open from this area (deferred, not forgotten): wide/zero-width glyph
continuation cells; damage-rectangle tracking rather than per-cell diffs; a
reusable widget vocabulary.

## Phase 2 — Interaction and shell (partly done)

Done:

- Screen/mode stack with push/pop and a transition effect.
- Capability detection and responsive handling of resizing, including very small
  terminals.
- Terminal-independent input, including an explicit auto-repeat flag.

Remaining:

- Configurable key bindings loaded from a config file.
- A settings screen, and a screen history (back, not just pop).
- A main menu / title screen; the game currently launches straight in.

## Phase 3 — World and exploration (started)

Delivered in the current slice:

- A pure world model: bounded tile areas, terrain, entities, doorways, and a
  player position with a facing direction.
- Two connected areas with distinct layouts, authored as ASCII rows.
- Tile movement with collision against terrain, boundaries, and entities.
- A deterministic interaction system supporting examination, dialogue, and area
  travel, with clear feedback when there is nothing to use.
- An NPC dialogue panel implemented as a pushed screen.
- A camera that follows the player and centres small areas.
- A status area showing the area, position, facing, discovery progress, and hints.
- Minimal versioned persistence of area, position, and discovery.

Remaining:

- More areas, more traversal, and a reason to explore them.
- Items that can be picked up, carried, and used.
- Richer ASCII environment art and tilesets; the current layouts are functional
  rather than beautiful.
- A map or minimap, and a discovered-area listing.
- Doors and transitions with more variety (locked doors, one-way drops).
- Field-of-view or lighting, if and when it earns its complexity.

## Phase 4 — Dialogue and story

- A dialogue system with branching choices and ASCII portraits.
- Story, setting, and characters defined here (currently undecided) with
  authoring-friendly data.
- Conversation state that persists, and quests built on top.

## Phase 5 — Magic combat

- Turn-based, magic-only combat: spell resolution, resources, and enemy behaviour.
- Combat presentation built on the animation system.

## Phase 6 — Progression and items

- Hybrid progression: discoveries/story milestones plus gameplay mastery.
- A skill tree.
- Items focused on exploration, puzzles, quests, and storytelling.

## Phase 7 — Persistence and release

- A save format for the rest of the game (inventory, quests, story flags) with
  versioning and migration. The current format deliberately covers exploration
  state only.
- Manual save slots.
- A real release strategy: packaging, versioned tags, and a release pipeline
  (deferred until then).
- Decide and add the project license.

## Cross-cutting, ongoing

- Keep dependencies minimal and justified (currently `crossterm` and
  `unicode-width`).
- Keep the game/render/terminal boundaries intact; see `AGENTS.md` and
  `docs/gameplay.md`.
- Keep documentation, the design documents, and the changelog current.
