# Changelog

All notable changes to Grimmoire are documented here. The project is in early
development and has **no tagged release yet**; everything is under `Unreleased`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project intends to follow [Semantic Versioning](https://semver.org/) once
it starts releasing.

## [Unreleased]

### Changed — the ruined chapel is lit by its own air

The first gameplay location to use the atmosphere layer, and the only one: the
other five places are untouched.

- **`Backdrop`, the one seam.** `ui::scenery` gave every tile a background of two
  tones — the light the player carries, and the dark outside it — which is a
  hard-edged *rectangle* around the player. It now takes a policy: `Material`
  (what every place does, unchanged) or `Atmosphere`, where the tiles leave their
  backgrounds alone so a field underneath shows through. The player's light is not
  lost with the tones; it still chooses a tile's ink.
- **`Sky::cloister`**, the chapel's air: cold light from the high windows falling
  away towards the floor, and one warm glow low and off to one side where the
  candles are. Resolved **once**, because a place does not move, with only the top
  of it drifting.
- **Cost: nothing measurable.** Standing idle 3.83% → 3.32% of one core, walking
  4.26% → 3.93%, at the same byte volume. Walking is *cheaper* than before,
  because the material pool's boundary used to be repainted on every step. The
  chapel's own diagnostics read 60 fps with 16 of 3480 cells painted while the
  ambience is live.
- **A latent bug fixed**: a screen pushed while the game was running was never
  told the viewport or the terminal's capabilities, because the application only
  reports a resize when the size *changes*. `ScreenStack::install` now tells every
  screen what is already known, whichever route added it.
- **The 256-colour reduction uses the palette's grey ramp**, not only the 6×6×6
  cube, whose dark end jumps from 0 straight to 95: a dark gradient through it was
  two flat tones with an edge. The sixteen-colour path keeps the material
  backdrop, because no gradient survives sixteen colours.
- **The chapel's glazing pitch is halved**, so its bays read as a lit building.

### Changed — the title screen, rebuilt around the atmosphere layer
### Changed — the title screen, rebuilt around the atmosphere layer

The title is the first screen to use `ui::atmosphere`, and it has been recomposed
around it rather than having a gradient put behind the old layout.

- **One layered scene**: the field is composed first and owns every cell's
  background; a cathedral silhouette, the wizard, rising motes, the wordmark and
  the menu are drawn above it and inherit the sky. The world, the presenter, the
  diff, the frame schedule and the terminal lifecycle are untouched.
- **`ui::cathedral`**: a cathedral drawn from a *plan* rather than a picture —
  towers with tapering spires, a cornice, a steep gable, a rose window, a rank of
  nave windows, tower lancets and a lit doorway, all scaled from the space
  available and symmetric about a spine.
- **A wordmark that reads.** A terminal cell is about twice as tall as it is
  wide, so the five-row figlet banner was a smear however it was drawn. The name
  is now one row, letter-spaced, in a gold-to-ember ramp with a rule beneath it.
- **A staged opening**, coordinated rather than simultaneous: the sky is there,
  the cathedral rises and its windows light in order, the name is written
  left-to-right, then the menu assembles. It settles into a calm idle and never
  replays, so returning from the journal or Settings costs nothing.
- **A bottom menu band**, on the dark ground where it can be read, with a
  highlighted selection and a note line.
- **The small palettes get their own sky.** A smooth gradient reduced into
  sixteen colours collapses to one flat rectangle, so `Sky::night_basic` chooses
  its stops *from* the classic colours. The moon's body is pure white so that it
  reduces to white rather than to bright yellow.
- **`Screen::on_capabilities`**, a small addition: a screen that builds something
  from the terminal's colour support now knows it before the first frame. Every
  screen is told, including ones pushed later.
- **Measured**: the atmosphere adds about **1.7 points of one core** during normal
  idle animation at 150×37 (budget: 3), and the opening holds **60 fps**. The idle
  runs at 11 fps by design, on the ambient cadence the game already uses.
- **Fixed**: `tools/smoke.sh`'s "composed the ASCII world" check tested for `#`,
  which came from the old title's tower rather than the world. It now checks the
  world's own status line.

### Added — a production-ready procedural atmosphere layer
### Added — a production-ready procedural atmosphere layer

- **`ui::atmosphere`**: a procedural true-colour field drawn underneath the
  glyphs as upper-half-block cells, whose foreground is the colour of the top half
  of the cell and whose background is the colour of the bottom half. A `Ramp`, a
  glow that falls off vertically *and* horizontally, an optional moon, stars and
  an aurora; deterministic for a given tick. Nothing above it changed.
- **The composition rules are asserted, not assumed.** A cell the field keeps
  carries two samples; a glyph drawn over it keeps its own ink and the *lower*
  sample as its background; a glyph with its own background replaces both; a space
  flattens the cell. The layer does not claim two independent samples beneath a
  glyph, because it does not have them.
- **A cost budget of its own**: the field is cached, the clock is quantised to a
  tick index, and `Drift` bounds which rows may change. Measured headlessly, the
  atmosphere adds **57–103 µs per frame** — **0.3–0.6% of one core** — during
  normal idle animation at 150×37, and the game holds 58 of its 60 frames a
  second end to end. The worst case (the whole field at 30 Hz) is measured and
  reported separately at 9–12%.
- **Smoothness enforced by test.** `a_night_sky_shows_no_bands` holds the shipped
  sky to seven units of colour per vertical sample at three sizes. The first
  capture was banded by 9–20; a seam at the horizon and a glow that was a
  function of the row alone were both designed out.
- **Degradation**: true colour, the 6×6×6 cube, or the nearest classic colour —
  never `Color::Default`, so a covered cell can never show the user's background.
- **`examples/skyline.rs`** rewritten around the module, with `--bench` for
  headless measurement and `GRIMMOIRE_SKY_MODE` for load levels; resizes verified
  through a pty from 150×37 down to 1×1 and back.

### Investigated — a true-colour atmosphere layer (prototype)
### Investigated — a true-colour atmosphere layer (prototype only)

- **`docs/renderer-investigation.md`**: why the renderer looks the way it does
  (one glyph, one colour and one flat background per cell; a palette of roles
  rather than ramps; no layer under the terrain; 256-colour output), what VinZ
  actually implements and what of it is worth taking, and a five-stage migration
  plan with rollback points.
- **`examples/skyline.rs`**: a feasibility prototype — a smooth true-colour sky,
  a moon, stars, a glyph cathedral, rising motes and the supplied wizard,
  composed over one new half-block atmosphere layer that every existing layer
  inherits its background from. It is a `Screen`, so it runs through the real
  `App`, `Terminal`, `Presenter` and `Canvas`.
- **Measured**: the prototype runs at about 35 fps of full-screen true-colour
  redraws and costs **10% of one core**, against the game's 2% while walking.
  The animated field changes almost every cell every frame, so the presenter's
  diffing cannot skip anything — the finding that governs the migration plan.
- **No game code changed.** The renderer, the world, the saves and the terminal
  lifecycle are untouched.

### Changed — every place is built from its own stone, and the light is in the glyphs

- **A material per location** (`theme::Arte`): the lit edge of masonry, its face,
  its shadow, the floor, the grit on it, the glazing light comes through, and the
  pool the player carries. Six places sharing one grey is what made the world look
  like a spreadsheet; a chapel of warm stone and gold glass, a shrine of pale
  marble and lavender, a catacomb of bone and spectral green, a village of timber
  and lamp-light, a cellar of steel blue and a wood of green now each read as
  somewhere.
- **Light is drawn into the glyphs, not behind them.** The same floor tile is
  bright stone inside the pool the player carries and dull stone outside it, which
  is a *pool*; the first attempt used a bright background, which is a lit
  rectangle. A test asserts the pool is dark enough to draw on, and another that
  the floor is not lit the same everywhere.
- **Architecture instead of texture**: walls one cell thick with floor on both
  sides read as lintels, solitary walls as columns, faces catch the light, and a
  wall in a run carries glazing — stained glass in the chapel, a lit rune in the
  shrine, a lamp behind a shutter in the village, a grave-lamp in the catacombs,
  and nothing at all underground or in a treeline.
- **Still air is the canonical art.** A character at rest now shows its base
  sprite and breathes a few times a minute, instead of holding its breath pose
  permanently.
- **The wizard's walk is visible**: the two step frames swing the robe across the
  body rather than moving one cell, and the sprint leans with the hem trailing. A
  test asserts the two steps differ from each other and from the base pose.
- **A rune circle turns at the wizard's feet** on the title screen, pulsing with
  the clock and static when motion is off.

### Added — a sprite library, and characters worth looking at

- **A central character sprite library** (`ui::sprite`). The supplied art — the
  wizard, the necromancer, the hag, the plague doctor, the assassin, the knight,
  the skeleton, the wraith, the imp, the bat, the ooze, the drake and the Lich
  King — is stored row for row, spaces and all, and drawn exactly as authored.
  The source is never trimmed, reflowed or recoloured: colour is chosen by what
  each *glyph* is, so a character can be re-inked without a cell of art changing.
- **Five more characters in the same idiom**, for the cast the supplied set had no
  counterpart for: Rin the lantern keeper, Cassen the scavenger, Ioreth the
  priestess, Nim the apprentice and Wren the wood spirit.
- **Companion animation frames** for the wizard (two steps, a breath and a
  sprint) and for the wraith, the bat and the ooze, chosen **once per step** so a
  pose can never drift from the movement it illustrates. The canonical base art is
  never one of its own frames, and a test asserts it.
- **West-facing is the same art reflected**, so the staff changes hands and the
  character visibly turns without the source being redrawn.

### Changed — the palette has life in it

- Every character now carries **eight inks** — outline, mass, shade, light,
  highlight, metal, eye and accent — instead of one silhouette colour. The result
  is characters with faces, glints and readable shapes rather than coloured blobs.
- **Colours reduce to the sixteen a basic terminal has** by nearest-match rather
  than by a second hand-maintained table, so the rich and poor palettes cannot
  drift apart.
- Every ink set was chosen against the floor it stands on: the first pass made
  Ysolde invisible, because a hood drawn in the colour of the stone is not a
  character.

### Changed — the dialogue portrait is the character's sprite

- The portrait plate shows the character's own sprite on a darker inset with a
  ring of marks, lit for the mood they are speaking in — the brightest cells
  warm, cool or go out. The art is never redrawn for an expression.
- The title screen now has the wizard standing on the horizon beside the chapel,
  breathing in its own companion poses.

### Fixed

- The pty smoke test looked for the old character's ink; it now checks the
  wizard's outline, which nothing else in the game draws in.

### Added — a title screen, branching conversations, and a cast of nine

- **An animated title screen.** The game opens on a night sky over the chapel:
  stars twinkling on a hash of their position, a moon and a horizon, runes rising
  off the ground, and the name composed out of glyphs in a warm ramp, revealed
  left to right. The menu assembles a row at a time. It is the game's first
  impression rather than a panel that disappears.
- **A menu that tells the truth.** New Game, Continue, Journal, Settings, Quit.
  Continue and Journal are disabled — and *skipped* while navigating — when there
  is no save, and the footer names where the save left off. Starting again over a
  save asks first ("Starting again will replace the journey you have saved"), and
  `esc` backs out of it safely. Settings change the motion level and are written
  to their own file, never into the save.
- **Conversations instead of lines.** Dialogue is now a graph of nodes with
  choices, conditions and effects (`game::conversation`): a node names its
  speaker, its mood, its lines, what it does and where it goes. Entry nodes are
  chosen by what the player knows, which is how the old discovery-reactive speech
  was generalised rather than replaced. Every existing conversation still reacts
  to discoveries; several now ask questions too.
- **Choices with consequences.** Accusing Cassen of cutting the rope is
  remembered, changes how he greets you afterwards, and is what makes Maren admit
  she cut it; promising to return the letters changes what Ysolde says about them;
  telling Nim about the moonwell, Wren about the lantern keeper, and Tamsin the
  scribe's name are all story facts that persist. Some choices teach a discovery
  outright, exactly as examining a relic would.
- **Four new characters**, each with their own silhouette, inks, portrait and
  voice: **Nim**, the shrine's apprentice (ribbons and far too much energy),
  **Maren**, a wandering mage under an enormous hat who has been "passing
  through" for nine years, **Wren**, a moss-and-mushroom spirit who keeps the
  glade and speaks in halves, and **Tamsin**, a warden in steel with a notebook
  and a gentler manner than the armour suggests.
- **A visual-novel conversation panel.** A speaker's face in its own darker
  inset, the name set into the plate's edge, lines that arrive a character at a
  time and can be completed with one key, and the options listed at the bottom
  with a selector that moves between them. The portrait changes expression with
  the speaker's mood.
- **Discoveries announce themselves**: a plate over the top of the world naming
  what was found, with a line from the journal, folding away after a few seconds.
- **A records screen from the title**, so a returning player can read what they
  found before deciding to continue.

### Changed — characters are four rows tall, and drawn from a bigger vocabulary

- Figure art is now **two head rows, a body and a hem**, with the body and hem
  each in a still and a swaying pair, so a step moves the garment. Faces, hoods,
  hats, plumes and ribbons are all distinct enough to name a character at a
  glance.
- Dialogue portraits are 12 × 8 with **four eye rows** per character, swapped for
  the speaker's expression.

### Changed — save version 3

- Adds `flags`: the story facts conversations establish. Version 1 and version 2
  files are still read, with no flags, so nobody loses a place or a discovery to
  this. A flag this build does not know costs only itself.

### Fixed — a conversation's effects are actually applied

- The conversation screen resolves its own graph and hands the world a list of
  effects when it closes; without the wiring that delivers them, every choice that
  did something would have silently done nothing. Found and fixed while driving
  the game in a real terminal.

### Changed — the smoke test waits for the game instead of guessing

- The pty smoke test now polls for the title and then for the world rather than
  sleeping a fixed time, because the keyboard handshake on a pseudo-terminal
  takes an unpredictable amount of time. It checks the title as well as the world.

### Added — a region worth exploring, and something to find in it

- **Four new places, and the world is now a graph rather than two rooms.**
  *Whispering Woods* (a trodden trail, a hidden glade, a keeper with a lantern),
  *Hollow Village* (a crossroad, six fallen houses, a dry well), *Moonwell Shrine*
  (a pale colonnade around a pool, three braziers, a dial, and a sealed door), and
  *Forgotten Catacombs* (burial galleries and a scribe's tomb). Every one is
  reachable from the chapel without solving anything.
- **Every area has a material and an air.** The wood is floored in grass and
  breathes pollen and blossom; the village is ash and cinders; the shrine is pale
  marble under cold silver motes; the catacombs are bone dust. Trees are now drawn
  neighbour-aware, so a wood reads as a canopy with edges rather than a field of
  identical icons.
- **Five characters, each with a silhouette and an opinion.** Ysolde is joined by
  Rin the lantern keeper, Cassen the scavenger, Ioreth the priestess, and Vesper
  the revenant. Each has four authored facings and a bust that appears when they
  speak.
- **Discoveries and a journal.** Seven discoveries, each with a title and a piece
  of the setting, recorded when they are *found* rather than when an area loads.
  The journal (`j`) lists them and how many are still out there.
- **Dialogue that remembers.** A character's speech is a default plus variants
  unlocked by discoveries, so what you have learned changes what people will tell
  you.
- **The moonwell.** Three braziers marked with phases of the moon and a dial cut
  into the floor naming the order. Light them in order and the water answers;
  light one out of turn and all three go out. Solved state and lit braziers
  persist, and answering it opens a shortcut between the shrine and the
  catacombs — both ways, so it is a shortcut rather than a toll.
- **A cast table** (`ui::cast`): every figure's art, inks and dialogue bust in one
  place, so the renderer draws a role the model named rather than a hardcoded
  character. Props work the same way, replacing the old "tell objects apart by
  their single glyph" hack.

### Fixed — a panel dismissed itself on the key's own release

- The journal (and, latently, the help overlay) treated the *release* of the key
  that opened them as an activation, so `j` opened the panel and its release closed
  it again in the same instant. Both now require a genuine press, like the dialogue
  panel already did. Found by driving the game in a real terminal, which is the
  only place it shows up.

### Changed — the save format grew a version, and migrates

- **Save version 2** adds the journal and the moonwell's braziers. **Version 1
  files are still read**, widened into the new fields rather than converted: a
  player from before progression existed keeps their place and simply has nothing
  discovered. Nothing is reinterpreted and no valid save is discarded because the
  schema grew.
- Progression ids this build does not recognise are dropped individually, so a
  discovery that was renamed or removed costs only itself.
- A version 1 file carrying a version 2 field is rejected rather than guessed at.
- The generator now **carves a tree out of a passage** exactly as it carves a
  wall, which is how the wood's trails are thinned to the character's width.

### Changed — the generator catches a whole class of map mistake

- `tools/apply-areas.py` writes the generated maps into `src/game/areas.rs`, so
  the committed layout is always the one the generator produces.
- The generator resolves every named landmark against the *finished* map and
  refuses to continue if one landed more than a few cells from where it was asked
  for. That is what catches the failure this phase actually hit: a glade drawn
  with the trail stopping three cells short of it, which was then quietly sealed
  off as an unreachable island.

### Changed — passages the character can actually walk through, and GPLv3

- **Every passage now fits the character.** The sprite is five cells wide, and the
  maps had one- and two-cell corridors and doorways, so she walked through the
  walls with her shoulders out. The chapel nave is now nine cells across, its
  crossings five tall edge to edge, both stair doorways five, the cellar's
  connecting corridors three tall, and the bridges over the flooded chamber three
  wide. **No tile the player can stand in is narrower than three cells.**
- **The rule is written down and enforced twice.** `tools/generate-areas.py` widens
  a map after drawing it — filling in a nook that cannot be widened rather than
  leaving a gap nothing fits in — and refuses to emit one that is still too narrow.
  `areas::MIN_PASSAGE` and `areas::DOORWAY` state the rule, three tests in
  `src/game/areas.rs` enforce it on every run, and
  [`docs/map-design.md`](docs/map-design.md) records it as an instruction for
  future map design.
- **Grimmoire is now GPL-3.0-or-later.** `LICENSE` holds the GPLv3 text and
  `Cargo.toml` carries the SPDX id, so licensing is settled rather than an open
  question.
- **The README is a project front page now**: badges, screenshots of the chapel,
  the dialogue, the cellar and the cast, the controls table, the architecture and
  the dependency rationale.

### Changed — a kawaii mage bunny, with readable profiles

- **The character reads as a bunny now, so she is one.** The pointed head was
  already being read as ears; the ears are now two separated points, the face
  outline and the ears are fur rather than cloth, and the ears are the loudest
  pink on screen. She carries a star wand and wears the same violet robe.
- **The left and right profiles were reworked.** They were two near-identical
  single-eye faces whose eye sat in the same place whichever way she turned, which
  read as a missing eye rather than a turned head. Each profile is now authored
  rather than mirrored, and **the eye sits toward the direction of travel**, with
  the wand held out in front.

### Changed — a kawaii witch, anime air, and figures at the scale of the world

- **The player character is now a kawaii witch, and she is a third of the size she
  was.** The old figure was six cells wide and five tall — five tiles high against
  one-cell walls — which made her tower over the map. She is now **five cells wide
  and three tall**, exactly the height of a doorway, a tree or the sigil, with
  chibi proportions: a tall pointed hat and a face with two big bright eyes take
  the whole figure, and the dress underneath is one row that swishes as she walks.
  She carries a star wand whose star twinkles.
- **She is drawn in pink and violet.** The character ink set moved from silver,
  indigo and gold to pink hair, a violet hat and dress, white lace and big cyan
  eyes — deliberately the most saturated thing on screen, so she pops off a world
  made almost entirely of cold stone.
- **Ysolde was rescaled to match**, and her face is now a void with two violet
  eyes rather than a rim.
- **Anime air.** A few of the drifting motes are now blossom rather than dust;
  interactions and arrivals throw off sparkles that fly outward as the pulse
  fades.
- **Cuter interface marks**: a heart marks the prompt, a star marks sprinting, and
  the dialogue bust is chibi — a big cowl and two large eyes.

### Added — anime-inspired identity, animation, and game feel

- **Characters with identities, drawn in several inks at once.** A figure is ASCII
  art plus a *legend*: each character in its rows names an ink, and each cast
  member carries its own `Inks` value. The mage now has silver hair, a face with
  two eyes, an indigo robe with a gold clasp and a crystal staff; the other
  character has a cowl, a trim rim and two violet points of light, and no skin at
  all. Both are four body rows plus a stride row, so a walk cycle costs two hem
  rows per direction rather than a second figure, and facing has four variants.
- **A named character.** The label "the Hooded Figure" is replaced by a name, and
  a test pins that names live in the model (`Entity::name`, `Dialogue::speaker`)
  rather than in the renderer.
- **Life in the world.** Candles gutter along wall faces that open onto floor,
  motes drift over bare ground, water rolls with two wave trains and occasional
  crests, and the sigil turns. Each area also carries an inlaid motif — a rose cut
  around the chapel crossing, rings around the cellar sigil — drawn only on plain,
  unoccupied floor.
- **Feedback for what just happened.** A refused step pulses *and underlines* the
  tile that refused it, an interaction flashes whatever answered, and arriving in
  an area marks the spot. Pulses are style changes, never glyph changes, so
  feedback cannot hide a character or an exit.
- **An animation model** (`ui::animation`). The walk stride advances once per
  *step*, taken from the movement model, so the legs keep pace with the world at
  any cadence and stop dead when movement stops; a tap keeps its stride for one
  cadence rather than snapping back. Sprint is its own stage with a lean and a
  wake. Idle figures breathe and blink on a slow clock without asking for frames.
- **A dialogue that opens and closes.** The panel rises from the bottom edge,
  brightens as it settles, and slides away on dismissal — and finishing its own
  exit is what motivated `Screen::after_update`, a way for a screen to request a
  command without waiting for a key that will never come.
- **A 60 Hz animation loop while anything is moving.** Frames are scheduled from a
  *deadline* rather than by sleeping a fixed interval after composing, so
  composition time cannot accumulate into drift, and the wait is clamped to one
  frame interval so a deadline set while idle cannot be inherited by the first
  frame of a step.
- `Screen::curtain`, so a screen that animates itself into place can decline the
  stack's full-screen curtain.
- Measured frame rate and cost are reported by `GRIMMOIRE_FRAME_STATS=1`, and the
  help overlay now shows a live frames-per-second figure.

### Changed

- The HUD gained a hierarchy: a prompt mark on a raised row, the movement state on
  the right, and the location and controls on the plain row beneath.
- A fresh game opens with a line of place rather than an empty message row.

### Fixed

- **A frame-loop pacing defect.** The animation deadline could be inherited from
  an idle wait, so a loop asked for sixty frames a second could quietly deliver
  thirty. The wait is now clamped to one frame interval.
- The frame-rate report counted the startup keyboard handshake as idle time, which
  understated it on terminals that do not answer the query; it now times the loop.

### Notes — measured

- Frame rate in a pseudo-terminal at 200×50: **64 frames/s walking east, 60
  walking north, 8.7 idle.**
- CPU: **0.5% of one core idle, about 2% walking.**
- Composition did not get slower: 0.27 ms a frame at 120×40, with an unchanged
  frame still costing sixteen bytes.

### Added — a custom ASCII renderer, and movement paced by screen speed

- **A custom ASCII scenery renderer** (`ui::scenery`). The world is drawn as
  glyphs, and *how* a tile is drawn depends on what surrounds it: a wall with open
  floor above shows a lit cap, one with floor below shows its shadowed base, one
  with open floor beside it shows a dressed face, and one standing alone in a room
  is drawn as a pillar. That is what turns a tile grid into architecture.
- **Variation that does not shimmer.** Floors, paths, water, rubble and moss pick
  their glyph from a hash of their *world* position, so the same tile looks the
  same every time it is drawn and neighbouring tiles never match.
- **Characters drawn as ASCII art.** The player and the hooded figure are small
  figures three cells square, with four facing directions and a two-frame walk
  cycle, instead of a single coloured letter. Water ripples, and the player's
  light falls away with distance.
- **Sprint.** `Shift` (or an upper-case movement key) doubles the pace. A sprint
  steps, collides and stops exactly like a walk.
- **Per-axis movement pacing, which fixes a real complaint.** A step moves the view
  one cell, and cells are about twice as tall as they are wide, so at one cadence a
  step north crossed more than twice the screen distance of a step east — vertical
  movement felt fast and horizontal felt slow. Both axes are now paced from one
  speed in *pixels per second* (`ui::motion::WALK_SPEED`, doubled for
  `SPRINT_SPEED`), with the cell size taken from the terminal.
- **`cargo run --example preview`**, which prints a rendered area as text, so art
  changes can be read and diffed without a screenshot.
- A test asserting that the default path emits **no graphics sequence at all**.

### Changed

- The world is no longer rendered as a software pixel-art image. That work was
  reverted on instruction: it looked like a different game. The pixel renderer
  survives as an off-by-default interface extra (see below).
- The three-step ambient shade ramp now drives the scenery's cell backgrounds, and
  the far distance is dimmed with the `dim` attribute rather than a second colour
  per role.
- `assets` no longer reads files or validates PNG signatures; artwork is source
  text, and the optional interface pieces are rendered to PNG in memory.

### Removed

- The arrival plates and their generator (`tools/generate-assets.sh`, `assets/`).
  Entering an area still plays the travel curtain; there is no illustrated plate.
- The scene image layer and its protocol plumbing (`SceneFrame`,
  `Frame::place_scene`, `Presenter::present_layered`, `Graphics::sync_scene`, the
  z-index placement form, and the pixel-art scene, layout and tile modules).

### Notes — the optional pixel interface

- `--pixel-ui` (or `GRIMMOIRE_PIXEL_UI=1`) draws interface pieces as pixel art —
  the dialogue portrait, and the study's portrait panel. It is off by default, the
  game never depends on it, and `src/pixel` is not reachable from the default
  rendering path. `fdeflate` is a dependency solely for this.
- Verified live in Kitty 0.49.2: the chapel and cellar render as ASCII, the player
  and the hooded figure are distinct, one press is one step, a ten-press burst
  inside one cadence moves exactly one tile, sprint paces and indicates, travel
  works both ways, and the dialogue draws its panel, ASCII bust and lines over the
  world.

### Added — Kitty-first rendering, a full-screen world, and responsive movement

- **Kitty graphics backend, replacing sixel.** Images are PNG transmitted inline
  (`a=t,f=100,t=d`), placed by cell (`c`/`r`) with a stable image and placement id
  so a moved image is replaced without flicker, and deleted on every exit path.
  The PNG bytes are copied through untouched, so the project has no image codec.
  The old sixel encoder, the CPU-side `Image` type, and the cell-metrics
  requirement were removed rather than kept as a second path.
- **A strict startup contract.** Grimmoire now checks for Kitty before entering the
  alternate screen and, if it cannot establish support, prints an actionable error
  and exits — there is no silent fallback to a broken render mode. Capability is
  classified as `Confirmed`, `Forced`, `Unverified`, or `Unsupported`, `$TERM` is
  never treated as proof, and `GRIMMOIRE_ASSUME_KITTY=1` is a documented
  development override.
- **Kitty keyboard protocol.** `Terminal::enter` requests event-type reporting when
  the terminal answers for it, so press, repeat, and release are all visible, and
  the protocol is released on the normal path, the error path, and from the panic
  hook.
- **An explicit movement model** (`ui::motion`, `REPEAT_INTERVAL` 125 ms). A fresh
  press steps immediately, holding walks at a tuned cadence, releasing stops at
  once, and repeats that do not continue a hold are ignored. Motion is
  **event-driven**: time alone never moves the player, so a lost release or a
  pasted key cannot start a runaway, and a tap is exactly one step. `Key` now
  carries a `KeyKind` (`Press`/`Repeat`/`Release`).
- **The world now fills the viewport.** Both areas were rebuilt larger than the
  reference size (140×52 and 136×44) with recognisable architecture, and the camera
  scrolls. The HUD became a two-row overlay painted over the world instead of four
  chrome rows framing it, so the environment dominates the screen.
- **Ambient lighting.** Floors are tinted by squared distance from the player,
  which reads as a pool of light and gives the space depth, with new `shade_*`
  theme roles.
- **A role-based theme extension**: `separator`, `text_secondary`, `focus`,
  `interactive`, `player`, `character`, `warning`, and the ambient ramp, alongside
  the existing roles.
- **Original artwork.** A portrait was produced procedurally, so the repository
  carries no third-party imagery. (The area plates this added were later removed in
  the ASCII phase; see above.)
- `tools/smoke.sh`, a quoted pty smoke test asserting terminal lifecycle and image
  cleanup, and `examples/bench`, which measures the exploration path.
- Content tests that every walkable tile is reachable, that entities can be walked
  up to, that doorways sit on doors, that arrival tiles are clear, and that each
  area is larger than the reference viewport.

### Changed

- `Screen::compose` takes a `&mut Frame`; screens record image placements as
  `AssetId` handles rather than image data.
- `Screen` gained `on_terminal_ready`, so a screen can learn whether the terminal
  reports key releases.
- `PresentStats` reports images transmitted, placed, and removed separately.
- `FrameContext` carries graphics support, the artwork, and whether key releases
  are reported, replacing the old image-backend field.
- The render loop no longer runs continuously while a direction is held: movement
  is event-driven, so an idle game asks for no frames at all.
- `input::Key::repeat` became `Key::kind`, with `is_repeat`/`is_release` helpers
  that make releases non-activations everywhere.

### Removed

- The sixel encoder, the CPU-side `Image`/`Rgb` types, the `ImageBackend`
  abstraction, and the cell-metrics requirement.

### Fixed

- Kitty draws an image at the **cursor**, not at an arbitrary rectangle. Placements
  now move the cursor to the rectangle first and then hold it there (`C=1`), which
  was found by live verification: without it, images were reserved but never
  painted.
- Holding a direction could walk forever if a release was missed, because
  self-driven cadence had no upper bound; movement is now event-driven.
- Auto-repeat could re-trigger discrete actions, because the `repeat` flag was
  never set on terminals without the enhanced keyboard protocol; the flag is now
  real, and releases are non-activations.
- The cellar's stair landed in a disconnected pocket, making the way back a long
  detour; the generator now seals unreachable pockets and short-circuits the stair
  into the chamber.

### Notes

- Verified live in Kitty 0.49.2 at the time: launching into the world, moving in
  every direction, holding and releasing, travelling both ways, and the keyboard
  protocol and images being released on exit.
- No license has been chosen yet, so none is included.
- The story, setting, characters, and detailed mechanics remain undecided. All
  current area names, entities, dialogue, and art are placeholder content.
- The Kitty decision, the image and input models, the measured numbers, and what
  was and was not verified live are recorded in `docs/renderer-design.md`. The
  audit that started the phase is `docs/phase5-audit.md`.

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
