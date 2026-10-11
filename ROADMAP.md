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
- An enforced text width policy; diffed presentation; a time-based animation
  system; a screen stack with push/pop and layered composition.
- `docs/renderer-design.md`.

## Phase 2 — Interaction and shell (partly done)

Done: the screen/mode stack; capability detection; resize handling; and
terminal-independent input including an explicit auto-repeat flag.

Remaining: configurable key bindings from a config file; a settings screen; a
screen history (back, not just pop); a title/menu screen.

## Phase 3 — World and exploration (started)

Delivered: a pure world model (tile areas, terrain, entities, doorways, facing);
two connected areas; tile movement with collision; a deterministic interaction
system; an NPC dialogue panel as a pushed screen; a following camera; a status
area; and minimal versioned persistence of area, position, and discovery.

Remaining: more areas and a reason to explore them; items that can be picked up
and used; richer environment art; a map or discovered-area listing; varied
doors (locked, one-way); field of view, if it earns its complexity.

## Phase 4 — Rendering architecture and visual freedom (done)

This phase settled the visual ceiling before more gameplay was built on top of it.

Delivered:

- An **audit** of the renderer that found and fixed a real defect: foreground-only
  draws were resetting the background, so the terminal's own theme could show
  through the game. Backgrounds are now inherited from the surface, making the
  viewport genuinely opaque (verified by pixel sampling).
- A theme expressed as **roles**: background, surface, raised surface, border,
  text, muted text, accents, and a magical-effect colour.
- A **`Frame`** composition boundary that separates cell rendering from image
  placement, so screens describe *intent* and never emit protocol bytes.
- An optional **sixel image backend** with an ASCII fallback, driven by a
  three-valued capability probe and requiring reported cell metrics.
- Presenter support for reserving image rectangles, re-emitting images on change,
  and repainting fully when an image disappears.
- A **minimum-viewport policy** (80×24) with a suspend screen, swallowed input,
  preserved state, and automatic resumption.
- The **presentation study** (`--study`, `F3`): an opaque themed prototype with a
  scene, a portrait plate, a detail column, a reveal transition, and ambient
  effects, plus live render diagnostics.
- Measured numbers and visual evidence recorded in `docs/renderer-design.md`.

Not delivered, deliberately: any second graphics protocol; a GPU/window renderer;
wide-glyph continuation cells; damage-rectangle tracking; real image decoding.

## Phase 5 — Dialogue and story

- A dialogue system with branching choices and ASCII portraits.
- An image-backed portrait path (the study already proves the rendering side).
- Story, setting, and characters defined here (currently undecided) with
  authoring-friendly data.
- Conversation state that persists, and quests built on top.

## Phase 6 — Magic combat

- Turn-based, magic-only combat: spell resolution, resources, and enemy behaviour.
- Combat presentation built on the animation system and the graphics backend for
  effects and enemy art.

## Phase 7 — Progression and items

- Hybrid progression: discoveries/story milestones plus gameplay mastery.
- A skill tree.
- Items focused on exploration, puzzles, quests, and storytelling.

## Phase 8 — Persistence and release

- A save format for the rest of the game (inventory, quests, story flags) with
  versioning and migration. The current format deliberately covers exploration
  state only.
- Manual save slots.
- A real release strategy: packaging, versioned tags, and a release pipeline
  (deferred until then).
- Decide and add the project license.

## Cross-cutting, ongoing

- Keep dependencies minimal and justified (currently `crossterm` and
  `unicode-width`, with its unused `cjk` feature disabled).
- Keep the game/render/terminal boundaries intact; see `AGENTS.md`,
  `docs/gameplay.md`, and `docs/renderer-design.md`.
- Keep documentation, the design documents, and the changelog current.
