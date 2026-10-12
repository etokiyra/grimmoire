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
and used; a map or discovered-area listing; varied doors (locked, one-way); field
of view, if it earns its complexity. Phase 5 rebuilt the two areas larger and
richer and made movement event-driven, so the "tiny world" and "throttled
movement" items on this list are done.

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

## Phase 5 — Kitty, a full-screen world, and movement that feels right (done)

This phase stopped being a renderer experiment and started being a game: the
playable world now dominates the screen and responds to input properly.

Delivered:

- An **audit** (`docs/phase5-audit.md`) that found why the world was small (tiny
  areas plus a centring camera) and why movement felt poor (input loss, one flat
  rate, no key releases, and a world that did not move).
- **Kitty is the supported terminal**, and the only graphics protocol implemented.
  Sixel was removed rather than kept as a second path. PNG is transmitted inline
  once per image, placed by cell, replaced without flicker when it moves, and
  deleted on every exit path.
- **A strict startup contract**: an actionable error and a clean exit when Kitty
  cannot be established, with `$TERM` never treated as proof and a three-valued
  capability classification.
- **Responsive, event-driven movement**: immediate on press, a tuned cadence while
  held, immediate on release, and impossible to run away with when a release is
  lost, using Kitty's keyboard protocol for press/repeat/release.
- **The world fills the viewport.** Both areas were rebuilt larger than a normal
  terminal, the camera scrolls, the HUD became a two-row overlay, and floors are
  lit by the player's distance.
- **Original generated artwork** for two arrival plates and a dialogue portrait,
  with provenance and no third-party rights.
- Live verification in a real Kitty session, plus `tools/smoke.sh` and
  `examples/bench`.

## Phase 6 — A custom ASCII game (done)

This phase was a direction correction as much as a build. The world had been
drawn as a software pixel-art image; the instruction was that the game is ASCII,
so that was reverted and the effort went into a character renderer instead.

Delivered:

- **A custom ASCII scenery renderer** (`ui::scenery`). A wall's glyph depends on
  what surrounds it — a lit cap where floor is above, a shadowed base where floor
  is below, a side face, or a pillar when it stands alone — which is what turns a
  tile grid into architecture rather than a field of `#`.
- **Variation that never shimmers**: floors, paths, water, rubble and moss pick
  their glyph from a hash of their *world* position, so the same tile always looks
  the same and neighbouring tiles never match.
- **Characters as ASCII art**: a staff-bearing wizard and a dark, still, two-eyed
  hooded figure, each three cells square, with four facing directions and a walk
  cycle.
- **Sprint**, and a fix for the axis asymmetry: the two directions are paced from
  one speed in *pixels per second* rather than one step rate, so north and east
  cross the screen at the same rate despite cells being taller than they are wide.
- **The pixel renderer kept, but demoted**: it now draws optional interface pieces
  only, behind `--pixel-ui`, off by default. The default path emits no graphics.
- `examples/preview`, which prints a rendered area as text so art can be read and
  diffed without a screenshot.

## Phase 7 — Anime-inspired identity, animation, and game feel (done)

This phase made Grimmoire look and behave like a designed game rather than a
renderer demo. It began with an audit (`docs/phase7-audit.md`) of how frames were
scheduled, what actually animated, and what the input path guaranteed.

Delivered:

- **Characters with identities.** Figure art plus, for the first time, *inks*: a
  figure's rows name hair, skin, cloth, trim and eyes, and each cast member
  carries its own set. The player is a kawaii mage bunny — pink ears, a pale face
  with two big bright eyes, a violet robe with white lace and a twinkling star
  wand — and Ysolde is a cowl with two violet points of light where a face should
  be. Figures were rescaled to **five cells wide and three tall**, the height of a
  doorway or a tree, so they stand in the world rather than over it.
- **Names in the model.** The NPC is a named individual, and `Entity::name` /
  `Dialogue::speaker` remain the only places character names appear.
- **Life in the world.** Candles gutter along wall faces, motes drift over bare
  floor, water rolls with two wave trains, and the sigil turns. Each area also
  carries an inlaid motif so its wide open floors are not blank paper.
- **Feedback.** A refused step pulses and underlines the tile that refused it, an
  interaction flashes what answered, and an arrival marks the spot — as style
  changes, so nothing is ever hidden.
- **Animation that keeps pace.** The stride advances per *step*, sprint is its own
  stage with a lean and a wake, idle figures breathe and blink, and the dialogue
  panel rises, settles and slides away.
- **A 60 Hz animation loop while something is moving**, deadline-scheduled so
  composition time cannot accumulate into drift, dropping to a slow ambient tick
  when idle. Measured at 60–64 frames a second while walking and 8.7 idle, at
  about 2% of a core moving and 0.5% idle.
- **The world brought to the character's scale.** The sprite is five cells wide,
  and the first maps had one- and two-cell passages, so she walked through the
  walls with her shoulders out. Both areas were redrawn so that no walkable tile is
  narrower than three cells and the routes walked constantly are wider still; the
  rule is enforced by the generator and by tests, and recorded as an instruction in
  [`docs/map-design.md`](docs/map-design.md).
- **An open-source face.** GPL-3.0-or-later, stated in `LICENSE` and `Cargo.toml`,
  and a README written as a project front page rather than a status note.

## Phase 8 — A living, explorable region (done)

The rendering foundation was in place; this phase built the *game* on top of it:
somewhere to go, someone to meet, and a reason to look closely.

Delivered:

- **Six connected places**, all reachable without solving anything: the Ruined
  Chapel, the Sunken Cellar, the Whispering Woods, the Hollow Village, the
  Moonwell Shrine, and the Forgotten Catacombs. Each has its own **material** and
  **air**, so it is recognisable before its name is read, and each has a landmark
  worth walking up to.
- **Five characters** with their own silhouettes, palettes and voices — a veiled
  keeper, a lantern child, a scavenger who lies, a priestess who answers
  obliquely, and something in the catacombs. Dialogue *reacts*: each character's
  speech has variants unlocked by what the player has discovered.
- **Discoveries and a journal.** Examining a relic or answering the moonwell
  records a discovery with a title and a piece of the setting. The journal (`j`)
  is the reward for looking closely, and the only one: no experience, no loot.
- **One puzzle, written on the floor.** Three braziers and a dial, an order to
  light them in, feedback that says when it was wrong, no way to soft-lock it, and
  a state that persists. Answering it opens a **shortcut** between the shrine and
  the catacombs — a reward for exploring, not a toll on it.
- **A save format that migrates.** Version 2 adds the journal and the puzzle
  state; version 1 files are still read, widened rather than converted, and an
  unknown progression id costs only itself.
- **The cast as a table.** `ui::cast` holds every figure's art, inks and dialogue
  bust, so the renderer draws a role rather than a hardcoded character. Props got
  the same treatment.

## Phase 9 — Identity, conversations, and an opening (done)

The game had a region and a loop. This phase gave it a face.

Delivered:

- **An opening.** A title screen with an animated glyph sky, a composed logo, a
  menu that reflects the save, and a safe confirmation before starting over. The
  game no longer begins as though it were a command that ran.
- **Conversations.** Dialogue became a graph — nodes, choices, conditions,
  effects — and the discovery-reactive speech became one case of it. Nine
  characters now have branching conversations, and what you do in one changes
  what another says.
- **Nine characters**, four of them new, each with a four-row figure, a portrait
  with four expressions, and their own inks.
- **Presentation throughout**: a visual-novel conversation plate, expression
  changes, a discovery announcement, and a shared ornament vocabulary so the
  title, the conversation, the journal and the HUD look like one game.
- **Settings**, in their own file: full, reduced, or no motion. Every animation
  can be turned off and the game stays entirely playable.

## Phase 10 — Expressive sprites and living colour (done)

The systems were in place and the presentation was not good enough. This phase
was about making the game *look* like one.

Delivered:

- **A sprite library.** Eighteen fixed sprites — the wizard, ten enemies and
  creatures, the Lich King, and five characters authored for the region — stored
  row for row and drawn exactly as supplied, with a test comparing them against
  the original art so nothing can quietly drift.
- **Eight inks per character.** Outline, mass, shade, light, highlight, metal,
  eye and accent, chosen by what each glyph *is*, so the art never changes to
  change a colour. Chosen against the floor a character stands on, which is how
  the first pass was caught making one of them invisible.
- **Companion poses** for the wizard and three creatures, cut once per step.
- **The wizard on the title screen**, standing on the horizon.
- **The dialogue portrait is the character's sprite**, lit for the mood they are
  speaking in.

## Phase 11 — Light, material, and the world as somewhere (done)

The sprites were in place and the *world* was still grey. This phase fixed the
world.

Delivered:

- **A material per location** and a rewritten lighting model: the pool the player
  carries brightens the glyphs it falls on, and everything outside it falls into
  the place's own dark. No bright rectangles, no `dim` attribute the terminal is
  free to ignore.
- **Architecture**: lintels, columns, glazing on long walls, and floors laid with
  seams, chosen from the tile's neighbours rather than applied as a texture.
- **Correct animation**: the canonical pose is what an idle character shows, the
  walk swings the robe instead of nudging a cell, and the sprint is its own pose.
- **A rune circle** at the wizard's feet on the title screen.

## Phase 12 — Renderer investigation: a true-colour atmosphere layer (prototype)

The renderer has accumulated features and still looks weak, so this phase
questioned the model rather than adding to it.

Delivered:

- **An honest statement of the limits**: one glyph, one colour and one flat
  background per cell means a gradient has nowhere to live; the palette is a
  table of roles rather than a ramp; a tile is either scenery or nothing; and the
  output is 256-colour indexed.
- **A VinZ investigation**, with what is worth taking (the half-block technique,
  independently implemented), what is not (its loop, cadence and terminal
  handling), and the licence position — which is straightforward, since
  Grimmoire is already GPL-3.0-or-later.
- **`examples/skyline.rs`**, a layered true-colour title-screen prototype built
  as a `Screen`: a smooth sky, a moon, stars, a glyph cathedral, motes and the
  supplied wizard, composed over one new atmosphere layer that every existing
  layer inherits its background from.
- **Measurements**, including the one that matters: an animated field defeats the
  presenter's diffing, and the prototype costs 10% of a core against the game's
  2%. Three fixes are proposed, all cheap.
- **Recommendation**: adopt the technique, not the program, and migrate in five
  reversible stages.

No game code changed. The report is `docs/renderer-investigation.md`.

## Phase 13 — The atmosphere layer, production-ready (done; not yet in the game)

Stage 1 of the migration the investigation proposed. The layer exists and is
verified; **no screen uses it yet**, so the game's own look has not changed. See
`docs/renderer-investigation.md` §9.

Delivered:

- **`ui::atmosphere`**: a procedural true-colour field drawn as half blocks. One
  continuous ramp, a glow that falls off in both axes, an optional moon, stars
  and an aurora. Deterministic for a given tick, with a `Ramp` and `Drift` a
  caller shapes rather than a picture baked in.
- **The composition rules, tested rather than assumed**: an open cell carries two
  samples, a glyph keeps its ink and the lower sample as its background, a glyph
  with its own background replaces both, and a space flattens the cell. The
  layer does not claim two samples under a glyph, because it does not have them.
- **A budget**: the field is cached, its clock is quantised, and `Drift` bounds
  which rows may change. The atmosphere adds **0.3–0.6% of one core** in normal
  idle animation at 150×37 and the game still holds 58 of its 60 frames a second.
  The worst case is measured separately: 9–12% end to end.
- **Smoothness as a constraint**: the first capture was banded, by 9–20 units of
  colour per half-row, because a sky ramp met a ground ramp at the horizon and
  the glow was a function of the row alone. One continuous ramp and a
  two-dimensional glow bring the worst step to 5 units, and
  `a_night_sky_shows_no_bands` holds it there at three sizes.
- **Degradation**: true colour, the 6×6×6 cube, or the nearest classic colour, and
  never `Color::Default`, so the field can never let the user's background show.
- **Resizes** driven through a pty from 150×37 down to 1×1 and back, repainting at
  each and exiting cleanly.

Next: stage 2 — the **title screen only**, behind `ColorSupport::TrueColor`.

## Phase 14 — The title screen, integrated (done)

The atmosphere layer's first host, and the title rebuilt around it rather than a
gradient put behind the old layout. `docs/renderer-investigation.md` §10 is the
full report; `docs/renderer-design.md` §7, §9 and §10 hold the design and the
numbers.

Delivered:

- **The scene**: a true-colour night with a moon, a cathedral silhouette (towers
  with spires, a cornice, a steep gable, a rose window, nave windows, lancets, a
  lit doorway), the wizard standing at the door's jamb, rising motes, a one-row
  letter-spaced wordmark in a gold-to-ember ramp, and a bottom menu band.
- **A staged opening**: sky, then the cathedral rises and its windows light in
  order, then the name is written, then the menu assembles. It settles into a calm
  idle and never replays.
- **The grid's own rules, learned and recorded**: a cell is about twice as tall
  as it is wide, so a multi-row wordmark smears, a wide roof becomes a staircase,
  and a trigonometric circle becomes a plus sign. The title answers each in the
  way the grid allows.
- **A deliberate fallback**: a sixteen-colour terminal gets a poster built *from*
  the classic palette rather than a collapsed gradient.
- **Measured**: the atmosphere adds ~1.7 points of one core at idle (budget 3);
  the opening holds 60 fps; the idle is 11 fps by design.
- **The world is untouched**, and the smoke test now actually tests it.

Next: one gameplay place, not all six. See the investigation's closing section.

## Phase 15 — Story

- Story, setting, and characters defined here (currently undecided) with
  authoring-friendly data.
- Conversation state that persists, and quests built on top.

## Phase 9 — Magic combat

- Turn-based, magic-only combat: spell resolution, resources, and enemy behaviour.
- Combat presentation built on the animation system, with ASCII effects and enemy
  art.

## Phase 10 — Progression and items

- Hybrid progression: discoveries/story milestones plus gameplay mastery.
- A skill tree.
- Items focused on exploration, puzzles, quests, and storytelling.

## Phase 11 — Persistence and release

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
