# Grimmoire

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust.

[![CI](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml/badge.svg)](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml)

> **Status: early development.** There is no stable release and no tagged version
> yet. Grimmoire is a small, playable **exploration prototype** — two connected
> areas, responsive movement with sprinting, interaction, one NPC, and fail-soft
> saving — on top of a custom ASCII renderer. It is not a complete game.

## What Grimmoire is

A game about a wizard who grows stronger through knowledge rather than gear, built
for the terminal and taking **ASCII art as its primary visual language**. The
long-term vision includes exploration, character-focused dialogue, strategic
turn-based magic combat, skill trees, story-and-mastery progression, and
persistent saves.

The story, world, characters, and detailed mechanics are **deliberately
undecided**. Everything currently in the game is placeholder content. See
[`ROADMAP.md`](ROADMAP.md) for phasing.

## Requirements

**Grimmoire runs in [Kitty](https://sw.kovidgoyal.net/kitty/).** It uses Kitty's
keyboard protocol so that it can see key *releases*, which is what makes "hold to
walk, release to stop" work properly. If it cannot establish that the terminal can
do this, it says so and exits rather than starting in a mode that would feel
broken.

Everything else it needs is a Rust toolchain and a terminal of at least **80×24**
reporting 256 colours.

## Play it

```sh
kitty --directory "$PWD" cargo run --release
```

You start in the **Ruined Chapel**. The stair down to the Sunken Cellar is at the
south end of the nave; **Ysolde** and a cracked basin are elsewhere in the chapel,
and a faded sigil waits at the far side of the cellar. Press `e` when the prompt
appears.

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move (and face that way). Hold to keep walking. |
| `Shift` + a direction | Sprint |
| `e` | Interact with what you face |
| `h`, `?`, `F1` | Help and diagnostics |
| `F3` | Presentation study (rendering prototype) |
| `F2` | Renderer showcase (development screen) |
| `Esc`, `q`, `Ctrl`+`C` | Quit |

To look at the interface pieces instead, launch the **presentation study**:

```sh
kitty --directory "$PWD" cargo run --release -- --study
```

Progress saves automatically on quit and whenever you move between areas; see
[`docs/gameplay.md`](docs/gameplay.md) for the save location and format.

Below **80×24** the game suspends, shows a short explanation, and swallows input
so you cannot act blind — then resumes exactly where you were when the window
grows. Grimmoire cannot maximise your window; that part is yours.

## Implemented today

**The exploration prototype**

- Two large connected areas that are **bigger than the viewport**, so the camera
  scrolls and the environment fills the screen.
- Tile-based movement with collision against terrain, boundaries, and entities,
  plus a facing direction so you can face what you want to use.
- **Event-driven movement**: a press steps at once, holding walks at a steady
  cadence, releasing stops immediately, and `Shift` sprints. A tap is always
  exactly one step, and no lost event can leave you walking on your own.
- **Visible feedback**: a refused step pulses the tile that refused it, an
  interaction flashes whatever answered, and arriving somewhere new marks the
  spot — all without ever hiding a glyph.
- **Animation that keeps pace**: characters stride once per step (never on a free
  running clock), sprint with a lean and a wake, breathe and blink when idle, and
  the dialogue panel rises and settles rather than appearing.
- **Both axes move at the same screen speed.** Terminal cells are taller than
  they are wide, so the two axes are paced from one speed in pixels per second
  rather than a single step rate — which is what stops vertical movement feeling
  twice as fast as horizontal.
- An interaction model resolved deterministically from the adjacent tiles, with a
  contextual prompt instead of permanent instructions.
- A dialogue overlay, pushed as a screen so input cannot leak through it.
- Autumn-tinted floors around the player, so the space reads as lit rather than
  flat.
- Automatic, versioned, fail-soft persistence of area, position, and discovered
  areas.

**The renderer and framework underneath**

- A **custom ASCII scenery renderer**: walls show lit caps, shadowed bases, side
  faces and pillars depending on what surrounds them; floors, paths, water and
  rubble vary by position without ever shimmering; water rolls; candles gutter
  along the walls; motes drift; and each area has an inlaid motif cut into its open
  floors.
- **Characters drawn as ASCII figures**, several cells tall and several *inks* at
  once — the mage has silver hair, a face with two eyes, an indigo robe with a gold
  clasp and a crystal staff; Ysolde is a cowl with a trim rim and two violet points
  of light. Four facing directions, a stride, a sprint, and an idle breath.
- A pure, terminal-agnostic cell renderer: `Canvas`, `Cell`, `Style`, `Color`,
  `Rect`, and a clipped local-coordinate `CanvasView`.
- A **fully opaque, application-owned background**: the theme paints every cell,
  and foreground-only draws inherit the surface, so the terminal's own theme never
  shows through.
- A role-based theme: background, surfaces, borders and separators, three weights
  of text, accents, focus, interactive objects, player, character, effects,
  warnings, and an ambient light ramp.
- `Frame` composition that separates **cell rendering** from **image placement**.
- **Diffed presentation**: only changed cells are written. An unchanged frame
  costs sixteen bytes.
- **A 60 Hz animation loop while anything is moving** (measured at 60–64 frames a
  second), dropping to a slow ambient tick when the world is merely breathing, so
  an untouched game costs about half a percent of a core.
- Time-based animation with an explicitly advanced, testable clock.
- A screen stack with push/pop, input routed to the active screen, and layered
  composition, plus a curtain transition between screens.
- A **minimum-viewport policy** (80×24): below it, play suspends safely and resumes
  on its own.
- Optional **pixel interface art** behind a development flag, off by default; the
  game itself never emits a graphics sequence.

## Not implemented yet

Combat, spells, inventory, items you can pick up, quests, skill trees, and story.
There is one NPC and no dialogue branching or scripting. The two areas are
hand-authored placeholders.

## Build and run

```sh
cargo run                        # run a debug build
cargo run --release              # run an optimised build
cargo run --release -- --study   # the presentation study
cargo build --release            # produce target/release/grimmoire
```

Resize the terminal freely: the layout and camera are recomputed, and below 80×24
play suspends to a short explanation rather than drawing a mangled layout. The
terminal — including the Kitty keyboard protocol — is restored on a normal exit,
on a recoverable error, and from a panic.

### Development overrides

| Variable | Effect |
| --- | --- |
| `GRIMMOIRE_ASSUME_KITTY=1` | Assume the terminal is Kitty even if it cannot be confirmed. For inspecting the game in another terminal, and for the pty smoke test. |
| `GRIMMOIRE_PIXEL_UI=1` | Draw interface pieces (the dialogue portrait) as pixel art. Off by default; equivalent to `--pixel-ui`. |

## Development

Run the same checks that CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Three tools are kept honest by use:

```sh
tools/smoke.sh                            # pty smoke test: terminal lifecycle and cleanup
cargo run --release --example preview     # print the ASCII world to the terminal
cargo run --release --example bench       # the performance numbers in the design doc
```

`examples/preview` is the quickest way to look at scenery: it renders a real area
as text, so art changes can be read and diffed without a screenshot.

## Architecture

| Module | Responsibility |
| --- | --- |
| `game` | The world model and its rules: areas, tiles, entities, movement, collision, interaction, travel. Pure — no rendering, no I/O. |
| `save` | A small versioned format and the only code that touches the save file. |
| `render` | Pure, terminal-agnostic drawing: cells, styles, colours, rectangles, canvases, views, `Frame` composition, image handles, and the optional PNG encoder. |
| `terminal` | The **only** module that talks to the terminal: raw mode, alternate screen, the Kitty keyboard and graphics protocols, input, capability detection, and diffed presentation. |
| `anim` | Time-based animation with an explicitly advanced clock. |
| `input` | Terminal-independent key and event types, including press/repeat/release. |
| `ui` | Frame composition: the ASCII scenery renderer, character animation, feedback effects, theme, layout, motion, screens, transitions, the screen stack, and the viewport policy. |
| `app` | Wires input, update, compose, and present together. |
| `pixel` | Software pixel rendering, used only by optional interface art. |

Documentation:

- [`docs/renderer-design.md`](docs/renderer-design.md) — the rendering
  architecture, the ASCII scenery renderer, the movement model and why the two
  axes are paced differently, the measured numbers, and what was verified live.
- [`docs/gameplay.md`](docs/gameplay.md) — the world model, controls, movement,
  interaction rules, and save format.
- [`docs/phase5-audit.md`](docs/phase5-audit.md) — the audit that started this
  work, before any change.

### Key decisions

- **ASCII is the identity, not a fallback.** The world is a glyph grid drawn by a
  renderer Grimmoire owns. Pixel rendering exists only as an off-by-default
  interface extra, and the default path sends no graphics at all.
- **Kitty is the supported terminal**, for its keyboard protocol and colour depth.
  `crossterm` is the sole terminal library; `ratatui` was rejected because it would
  own the drawing model.
- **Movement is event-driven, never timer-driven.** A press steps at once, holding
  walks at a cadence, releasing stops. No sequence of lost events can make the
  player walk on their own.
- **The two axes share a screen speed, not a cadence.** Cells are not square, and
  pretending otherwise is what made vertical movement feel fast.
- **The world is larger than the viewport**, and the camera scrolls; the HUD is an
  overlay of two rows rather than rows taken from the world.
- **The viewport owns its background.** Foreground-only draws inherit the surface
  colour, so the terminal's theme can never leak into the game.
- **Animation is time-based, and the walk cycle is locked to steps.** A slow
  machine must not slow the world down, and the legs must not drift away from the
  movement they are illustrating.
- **A hard minimum viewport (80×24).** Grimmoire is designed for a large terminal
  and suspends safely below it rather than degrading into noise.
- **No ECS, no scripting engine, no widget toolkit** until a phase needs one.
- **Game logic is pure.** Movement, collision, interaction, and travel are testable
  without a terminal; the UI decides how things look.
- **Persistence is narrow and fail-soft.** Only area, position, and discovery are
  stored; anything unrecognised means "start a new game".

## Known limitations

- **Kitty only.** Other Kitty-protocol terminals are untested, and tmux/zellij may
  intercept the keyboard protocol.
- **One glyph per cell.** Wide and zero-width glyphs are not rendered; the cell
  model has no continuation cell.
- Presentation repaints per changed cell rather than tracking damage rectangles. A
  step with a scrolled camera rewrites a few thousand cells.
- The world is two rooms with one NPC and no dialogue branching; the art is
  placeholder.
- Visual verification was done on Linux under Kitty 0.49.2; other platforms are
  untested.
- Terminal restoration cannot be guaranteed against `SIGKILL` or power loss.

## License

No license has been chosen yet, so none is included. This is an open decision and
must be settled before the project is distributed or reused. See `AGENTS.md`. All
artwork is generated from source text in this repository and carries no
third-party rights.
