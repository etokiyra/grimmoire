# AGENTS.md

Repository guidance for coding agents working on Grimmoire. Keep it concise and
current.

## What Grimmoire is

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust, targeting
the Kitty terminal. The project is in early development: it currently provides a
playable exploration prototype — two large connected areas, responsive movement
with sprinting, an overlay HUD, interaction, one NPC, and fail-soft saving — on
top of a custom ASCII scenery renderer, an animation system, and a screen stack.
It is not a complete game.

## Vision and confirmed decisions

These are settled and should guide new work:

- **ASCII is the identity, not a fallback.** The world is a grid of glyphs drawn
  by a renderer Grimmoire owns (`ui::scenery`). Single-column block and box glyphs
  (`█`, `▓`, `▀`, `·`) are legitimate drawing elements; wide glyphs are not used.
  Do not move the world back into an image, and do not treat a glyph grid as a
  limitation to be escaped: invest in the character renderer instead.
- **Pixels are an optional interface extra, off by default.** The software pixel
  renderer (`src/pixel`) and the PNG encoder exist only for interface pieces such
  as the dialogue portrait, enabled by `--pixel-ui` /
  `GRIMMOIRE_PIXEL_UI=1`. The default path must emit no graphics sequence at all,
  and a test asserts it.
- **Kitty is the supported terminal.** The reason is the keyboard protocol — key
  releases are what make "hold to walk, release to stop" work — and colour depth.
  Other Kitty-protocol terminals are untested and tmux/zellij may intercept the
  protocol. When the terminal cannot be established, the application prints an
  actionable error and exits; it never silently degrades.
- **Magic-only, turn-based combat** (planned; not implemented).
- **Knowledge-driven progression.** A hybrid of discoveries/story milestones and
  gameplay mastery. Items support exploration, puzzles, quests, and storytelling
  rather than equipment-based power scaling.
- **Custom renderer, borrowed plumbing.** Grimmoire owns its drawing model,
  layout, frame composition, scenery, and animation. It relies on libraries for
  low-level terminal operations and does **not** build a terminal emulator.
- **`crossterm` is the sole terminal library.** Do not add `ratatui`, `termion`,
  or a second backend without a strong, documented reason. `ratatui` in
  particular would own the drawing model.
- **No ECS, no scripting engine, no widget toolkit** until a phase concretely
  needs one. The current world is a grid plus lists.
- **Graphics capability is never assumed.** Classification is `Confirmed`,
  `Forced` (the documented development override), `Unverified`, or `Unsupported`.
  `Unsupported` is fatal, `Unverified` warns, and `$TERM` alone is never proof.
  Do not treat a successful build, or a detection heuristic, as evidence that
  rendering works.
- **Minimal dependencies.** The bar is `crossterm` and `unicode-width`, with
  `fdeflate` added solely for the optional pixel interface art. Every new direct
  dependency needs a justification in the pull request or commit message and in
  `Cargo.toml`. Do not add an image decoder: Kitty decodes.

Explicitly **undecided** (do not treat as settled, and do not invent them): the
story, world, characters, detailed mechanics, and art direction beyond "ASCII".
All current area names, entities, and dialogue lines are placeholders.

**Licensing is settled: GPL-3.0-or-later.** `LICENSE` holds the GPLv3 text and
`Cargo.toml` carries the SPDX id. Do not relicense, and keep the two in step.

## Map design

- **Every passage fits the character.** No tile the player can stand in may be
  narrower than `areas::MIN_PASSAGE` (three cells; the sprite is five wide). It
  covers anything passed *through*: corridors, chamber mouths, the gaps between
  pillars and pews, and the approach to a doorway. Routes walked constantly are
  drawn wider — the chapel nave is nine cells, its crossings and both stair
  doorways five. Doorways in outer walls are `areas::DOORWAY` (five) wide.
- The rule is enforced twice, so a hand edit cannot quietly regress it:
  `tools/generate-areas.py` (`widen`) widens a narrow map and refuses to emit
  one, and `src/game/areas.rs` asserts it in `every_passage_fits_the_character`.
- Maps are **generated**: `tools/generate-areas.py` draws them, `tools/apply-areas.py`
  writes the grids into `src/game/areas.rs`. Everything around the grid — the
  cast, the props, the doorways, the spawns, the landmarks — is authored by hand
  there, from the coordinates the generator prints.
- Read [`docs/map-design.md`](docs/map-design.md) before drawing or editing an
  area. It also lists the rules a map already had to satisfy (reachability,
  doorway placement, viewport size) and the material/air/motif identity each area
  should carry.

## Conversation and presentation

- **A conversation is data, not code.** Nodes, choices, conditions and effects
  live in `game::conversation`; the world applies the effects and nothing else
  mutates state. Adding a conversation must not add a branch to the renderer, and
  must not add a special case to a screen.
- **A screen cannot reach the world.** A pushed screen hands its outcome back
  through `Command::Return` and `Screen::receive`; that is the only route by which
  a conversation changes anything, and it is what keeps input routing — not a mode
  flag — responsible for stopping movement during one.
- **The graph is checked, not trusted.** `every_conversation_in_the_game_is_a_sound_graph`
  walks every conversation the game can put a player inside: no dangling node, no
  unreachable node, no empty speech, no line too long to read.
- **Effects are idempotent and by stable id.** A flag or a discovery this build
  does not know costs only itself in a save, and applying an effect twice changes
  nothing.
- **The ornament vocabulary is shared.** Frames, rules, selectors and markers come
  from `ui::ornament`, so the title, a conversation, the journal and the HUD are
  visibly one game. Do not draw a new border inline.
- **Everything animates from elapsed time, and everything can be turned off.**
  `Motion::Off` must leave every screen fully usable and still — the title, the
  conversation, the world and the transitions included.
- **Nothing the player must act on may be hidden.** A panel may announce, but it
  must not cover an exit, a landmark, or the player.

## Light and material

- **Every place has a material** (`theme::Arte`): cap, wall, base, mass, floor,
  grit, glazing, pool and dark. A new area without one is drawn in the fallback
  stone, which is a bug rather than a default.
- **Light is a property of the glyphs, not of the background.** The pool the
  player carries brightens the ink of what it falls on; the background only steps
  between two dark tones. A bright background is a lit rectangle, not a pool, and
  a test asserts the pool stays dark enough to draw on.
- **Do not use `dim` to express distance.** On an indexed colour a terminal is
  free to ignore it, which is how the first lighting pass ended up with no falloff
  at all. Step the colour instead.
- **A gradient steeper than roughly six units of colour per vertical sample
  bands.** At terminal resolution a steep ramp reads as a stack of horizontal
  lines, however "correct" the maths is. Spread a ramp over the height, fall a
  light off in both axes, and keep the whole field one continuous ramp: a sky
  ramp meeting a ground ramp at the horizon is a hard edge by construction.
  `a_night_sky_shows_no_bands` is what holds the shipped sky to that.
- **Architecture comes from neighbours, not from a texture.** A wall with floor
  above and below is a span; a lone wall is a column; a glazed run is a wall with
  stone above and below it. Draw the building, not the material.

## Character art

- **The sprite library is the authority.** `ui::sprite` holds the character art
  row for row. It is never trimmed, reflowed, padded or rewritten: a row's
  leading and trailing spaces are part of the silhouette, and
  `preserves_every_declared_row_exactly` compares the library against the
  supplied text so a careless edit fails the build.
- **Colour is chosen by glyph, never stored in the art.** `part_of` classifies a
  cell as outline, mass, shade, light, highlight, metal, eye or accent, and the
  character's `ArtInks` says what each is. A sprite can be recoloured, re-posed or
  drawn over anything without a glyph changing.
- **Every sprite declares its logical size, and every glyph is one column wide.**
  `every_sprite_glyph_is_one_cell_wide` enforces it. If a future sprite needs a
  glyph that is not one column, the renderer must grow an explicit
  logical-versus-physical distinction rather than let a row shear.
- **Companion frames are companions.** A pose is a *separate* row set; the base
  art is never one of its own frames, and a sprite without a pose for the moment
  holds its base rather than inventing one.
- **New characters follow the library's idiom**: a strong silhouette at 7×5 (11×7
  for a boss), a mass made mostly of shading rather than solid blocks, a readable
  face, and deliberate headwear. Do not add a character whose mass is one solid
  colour — it reads as a blob, not a person.
- **Readability is checked against the floor.** An ink set whose mass is the same
  value as the stone it stands on is a bug, not a mood.

## Content and progression

- **A discovery is the reward.** Recording one is what "finding something" means,
  and it happens when the player examines a relic or answers the moonwell — never
  because an area was loaded. `game::discovery` is the whole list.
- **A character is a role, not a picture.** The model names a `Figure` or a `Prop`;
  `ui::cast` and one table in `ui::scenery` decide what it looks like. Adding a
  character or an artefact must not add a branch to the renderer.
- **Names and words live in the model.** `Entity::name`, `Dialogue::speaker` and
  every journal line are content. The renderer has no idea what any of them say.
- **Progression stays small and principled.** What exists is discoveries, the
  journal order, and the moonwell's braziers. Do not add a level, a skill tree, an
  economy or an inventory until a phase concretely needs one; the state is shaped
  so those can be added beside it rather than replacing it.
- **Unlocking is a shortcut, not a toll.** Every area must be reachable without
  solving anything; a puzzle may open a faster way between two places.
- **Saving migrates, it does not discard.** Files are written at the current
  version and older ones are read, widened into the new fields. A progression id
  this build does not know costs only itself. Never reset a valid save because the
  schema grew.

## Architecture invariants

Respect these; they are the point of the current structure:

- `src/game/` — the world model and its rules (`area`, `entity`, `dialogue`,
  `world`, `areas`). Pure: no rendering, no canvas, no terminal, no file I/O.
  Movement, collision, interaction, and travel must stay testable without a
  terminal. The game never decides how anything looks; it exposes tiles and
  entity kinds and lets the UI map them to glyphs and colours.
- `src/save.rs` — the versioned save format and the only code that touches the
  save file. Saving must stay fail-soft: anything unrecognised means "start a new
  game", never a half-loaded state. Only essential exploration state is stored.
- `src/render/` — pure, terminal-agnostic drawing (`Canvas`, `CanvasView`,
  `Frame`, `ImagePlacement`, `AssetId`, `Cell`, `Style`, `Color`, `Rect`, `glyph`).
  **No** terminal or game code, and no dependency on a terminal library.
- `src/anim/` — time-based animation. **No** terminal or UI code. Progress is a
  function of elapsed time, advanced explicitly; never frame counts.
- `src/terminal/` — the **only** module allowed to talk to the terminal, and the
  **only** place that emits graphics-protocol bytes (`terminal::kitty`). It owns
  the image lifecycle for the optional pixel UI: transmitting an asset once,
  placing it by cell, replacing a placement in place, and deleting images on every
  exit path.
- `src/ui/atmosphere.rs` — the procedural true-colour background layer. It draws
  one `▀` per cell, foreground above and background below, which is two vertical
  samples in a model that has one. It is **not** used by the game yet. A cell it
  keeps carries two samples; a glyph drawn over it keeps its own ink and the
  *lower* sample as its background; a glyph with its own background replaces
  both; a space flattens the cell. Do not claim two samples beneath a glyph — the
  tests assert the table in `docs/renderer-design.md` instead. Keep it cached,
  quantise its clock, and bound its drift with `Drift`: an animated full-screen
  field defeats the presenter's diff, which is what the first prototype did.
- `src/ui/scenery.rs` — the ASCII world renderer. Pure: a world, a palette, a
  time and a pose in; cells out. Structure comes from a tile's neighbours,
  variation from a hash of its world position (never of the frame, or the world
  would shimmer), and light from distance. Decor — inlay, candles, motes — is
  drawn only where it cannot cover a character, a prop or a doorway, and the draw
  order is terrain → inlay → props → ambience → figures. Keep it total: it must
  draw every visible cell without gaps.
- `src/ui/animation.rs` — the character animation model. The walk stride advances
  on *steps*, not on a clock, so it cannot drift away from the movement it is
  illustrating; idle breathing and blinking run on their own slow clock.
- `src/ui/effects.rs` — transient feedback pulses. They are pure, self-expiring,
  capped, and rendered as a *style* change so they can never hide a glyph.
- `src/pixel/` — software pixel rendering, used **only** by optional interface
  art. It must never be reached from the default rendering path.
- `src/ui/viewport.rs` — owns the minimum-viewport policy. Screens must not grow
  their own "too small" behaviour; the application suspends for them.
- `src/input.rs` — terminal-independent `Key`/`Event` types. The backend
  translates library events into these, including the event kind
  (press/repeat/release).
- `src/ui/` — composes frames from state and owns the screen stack. No I/O, no
  escape sequences, no game rules.
- `src/app.rs` — wires input → update → compose → present and drives the loop.

Additional rules:

- Screens must never write to the terminal; they compose into a `Frame` and
  return `Command`s from `handle_key`.
- Screens never encode or position an image, and never see image bytes. They
  record an `ImagePlacement` naming an `AssetId` and a rectangle, and draw a
  fallback in the same rectangle; the presenter reserves the rectangle and the
  graphics layer decides whether to paint pixels there. Kitty draws an image at
  the cursor, so a placement must move the cursor to the rectangle first.
- Input goes to the active (top) screen only; composition runs bottom to top.
  This is what stops movement while a dialogue panel is open — do not replace it
  with a mode flag inside the game.
- Discrete actions (`e`, help, screen changes) fire only on a non-repeat press:
  releases are not activations, and auto-repeat must never re-trigger them.
- **Movement is event-driven; time never moves the player.** A press steps at
  once, auto-repeats step again at the cadence, and a release stops the walk. Do
  not reintroduce timer-driven stepping: it is what lets a lost release walk the
  player onward. A stall guard may clear the reported held state, but must never
  take a step.
- **Animation is a function of elapsed time, never of frame count**, and the
  walk cycle is locked to steps rather than to a free-running clock. Do not
  animate from a frame counter: a slow machine must not slow the world down.
- **A frame is composed when something changed, and only changed cells are
  written.** The animation target is sixty updates a second while something is
  moving and a slower ambient tick when nothing is; do not make the idle game
  run at sixty, and do not emit a full redraw to look busy. The loop schedules
  from a deadline, not by sleeping a fixed interval after composing.
- **The two axes share a screen speed, not a cadence.** Terminal cells are taller
  than they are wide, so `ui::motion` derives the north/south cadence from the
  cell height and the east/west cadence from the cell width, both from one speed
  in pixels per second (`WALK_SPEED`, doubled for `SPRINT_SPEED`). Do not
  "simplify" this to one interval: that is exactly the bug that made vertical
  movement feel twice as fast as horizontal.
- The text width policy in `render::glyph` is the single source of truth for what
  may be drawn.
- **Artwork is source text, not files.** The world is glyph tables, figure
  legends and neighbour rules in `ui::scenery`; the optional interface art is
  ASCII sprite tables in `src/pixel/art.rs` rendered to PNG in memory. Characters
  are multi-ink: a figure's rows name inks from an `Inks` value carried per cast
  member, so hair, skin, cloth, trim and eyes are separate colours. Nothing is
  read from disk, so there is no third-party imagery, no provenance to track, and
  no missing file to break the game.
- **Names and display text live in the model.** `Entity::name` and
  `Dialogue::speaker` are the only places character names appear; the renderer has
  no idea what any of them say. Keep it that way so the cast can be rewritten
  without touching drawing code.
- **Backgrounds are inherited, not defaulted.** A draw that leaves `Style::bg` at
  `Color::Default` keeps whatever background the cell already had. That is what
  makes the viewport opaque. Only paint an explicit background when a component
  genuinely wants its own surface, and never replace `Canvas::set`'s inheritance
  with a raw store.
- Screens persist their own state in `Screen::on_exit`; the stack decides when
  the application is finished.

If a change crosses a boundary, call it out and explain why. Read
`docs/gameplay.md` before changing the world model, and
`docs/renderer-design.md` before changing rendering, presentation, animation, or
the input model. `tools/smoke.sh`, `examples/bench` and `examples/preview` exist so
claims about terminal lifecycle, performance and appearance can be re-checked
instead of asserted.

## Code quality

Run these before reporting work complete; CI runs the same commands:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Clippy warnings are errors. Keep the code formatted. Prefer small, focused units
with focused tests over broad, shallow coverage. Tests must run without a real
terminal: assert on composed `Canvas`es, on presenter output captured into a
`Vec<u8>`, and on game state directly. Use a temporary directory for any test
that exercises the save store — never the real user data directory.

## Verification discipline

- **Verify claims before reporting completion.** Never report a command, build,
  test, push, or CI run as successful unless you observed the result.
  "Attempted" is not "succeeded".
- Do not claim performance improvements without a measurement.
- Visual claims need visual evidence. A graphical terminal, a screenshot, and a
  sampled pixel are stronger than an argument: `foot` plus `grim` are available
  here for exactly that. Sample pixel colours to prove background ownership, and
  diff two frames to prove that graphics survive live cell traffic.
- When you state that interactive or terminal behaviour works, say how it was
  verified (for example, driven through a pseudo-terminal) or state that it was
  not. Distinguish "the protocol is implemented and tested" from "it renders
  correctly on a real terminal".
- Do not claim remote CI passed unless you have seen its result.
- A rendering change is not verified until it has been seen in a real Kitty
  session: launch it, drive it, and capture it. `tools/smoke.sh` covers terminal
  lifecycle through a pty; `examples/bench` covers the numbers; `examples/preview`
  prints the world so art can be read directly.

## Documentation

After substantial changes, update the documentation in the same change:

- `README.md` — user-facing status, requirements, controls, build/run,
  architecture.
- `docs/gameplay.md` — the world model, controls, interaction rules, save format,
  and where the code lives.
- `docs/renderer-design.md` — rendering, the ASCII scenery renderer, the movement
  model, the optional pixel interface, measurements, and verification.
- `docs/map-design.md` — the rules an area must satisfy, above all that every
  passage fits the character, and how the generator and the tests enforce them.
- `docs/renderer-investigation.md` — the renderer's structural limits, the VinZ
  investigation, the proposed atmosphere layer, the prototype's measurements, and
  stage 1's outcome (the `ui::atmosphere` module, its corrections and its budget).
- `ROADMAP.md` — phasing for future work.
- `CHANGELOG.md` — notable changes under `[Unreleased]`.
- This file — if vision, boundaries, or workflow change.

Keep documentation honest about what is and is not implemented. Do not describe
planned features as if they exist.

## Git and commits

- Use clear [Conventional Commit](https://www.conventionalcommits.org/) messages
  (for example `feat:`, `fix:`, `docs:`, `chore:`).
- Commit `Cargo.lock`; Grimmoire is an application.
- Never commit secrets, credentials, API keys, or machine-specific paths.
- Never rewrite or force-push shared history.
