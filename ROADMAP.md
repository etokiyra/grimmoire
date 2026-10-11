# Roadmap

This roadmap is organised into practical phases. It is a direction, not a
promise, and it will change. **The story, setting, characters, and detailed
mechanics are explicitly undecided** and are not committed to by anything below.

Each phase should ship something small, correct, and verifiable.

## Phase 0 — Project foundation (done)

- Cargo application project named `grimmoire`, committing `Cargo.lock`.
- Terminal backend around `crossterm`: raw mode, alternate screen, input,
  guaranteed restore.
- A small, pure, cell-based renderer (`Canvas`, `Cell`, `Style`, `Color`, `Rect`).
- A title screen demonstrating ASCII art, colour, layout, and input.
- Unit tests, formatting/Clippy/test/release gates, and CI.
- `README.md`, `AGENTS.md`, `ROADMAP.md`, `CHANGELOG.md`.

## Phase 1 — Renderer growth

- Incremental/diffed screen updates instead of a full repaint each frame.
- An animation timeline the main loop drives (frame timing, eased transitions).
- Multiple named screens/layouts with explicit navigation between them.
- A small widget vocabulary that stays in `ui/` and never leaks into `render/`.
- Snapshots/tests for frame composition.

## Phase 2 — Interaction and shell

- A screen/mode stack (title, menu, settings, gameplay placeholder).
- Configurable key bindings loaded from a config file.
- Terminal capability detection (colour depth, size changes via resize events).
- Graceful handling of very small or resized terminals everywhere, not just the
  title screen.

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
- Combat presentation layered on the renderer's animation support.

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

- Keep dependencies minimal and justified.
- Keep the render/game/terminal boundaries intact.
- Keep documentation and the changelog current.
