# Grimmoire

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust.

[![CI](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml/badge.svg)](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml)

> **Status: early development.** There is no stable release and no tagged
> version yet. Grimmoire is a small, playable **exploration prototype** — two
> connected areas, movement, interaction, one NPC, and fail-soft saving — on top
> of a custom cell renderer with an optional image backend. It is not a complete
> game.

## What Grimmoire is

A game about a wizard who grows stronger through knowledge rather than gear,
built for the terminal and taking ASCII art as its primary visual language. The
long-term vision includes exploration, character-focused dialogue with ASCII
portraits, strategic turn-based magic combat, skill trees, story-and-mastery
progression, and persistent saves.

The story, world, characters, and detailed mechanics are **deliberately
undecided**. Everything currently in the game is placeholder content. See
[`ROADMAP.md`](ROADMAP.md) for phasing.

## Play it

```sh
cargo run --release
```

You start in the Ruined Chapel. Walk to the stair in the south wall and press
`e` to head down to the Sunken Cellar.

To evaluate the visual direction instead, launch the **presentation study**:

```sh
cargo run --release -- --study
```

The study is a rendering prototype — an opaque themed surface, composed panels,
an animated ASCII scene, a portrait plate, a theme gallery, and live renderer
diagnostics including which graphics backend is in use. It is the subject of
[`docs/renderer-design.md`](docs/renderer-design.md).

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move (and face that way) |
| `e` | Interact with what you face |
| `h`, `?`, `F1` | Help and renderer diagnostics |
| `F3` | Presentation study (rendering prototype) |
| `F2` | Renderer showcase (development screen) |
| `Esc` | Quit |
| `q`, `Ctrl`+`C` | Quit |

In the study, `g` toggles the image-backed portrait, `space` pauses the scene,
and `r` replays the reveal.

Progress saves automatically on quit and whenever you move between areas; see
[`docs/gameplay.md`](docs/gameplay.md) for the save location and format.

Grimmoire is designed for a terminal of at least **80×24**. Below that it
suspends play, shows a short explanation, and resumes automatically when the
window grows — without losing your place.

## Implemented today

**The exploration prototype**

- A world of two connected, hand-authored areas with distinct layouts, walls,
  paths, water, and a stair between them.
- Tile-based movement with collision against terrain, area boundaries, and
  entities, plus a facing direction so you can face what you want to use.
- An interaction model resolved deterministically from the adjacent tiles.
- An NPC dialogue panel, pushed as a screen so input cannot leak through it.
- A camera that follows the player and centres small areas, with responsive
  layout down to a graceful "too small" fallback.
- Automatic, versioned, fail-soft persistence of area, position, and discovered
  areas.

**The renderer and framework underneath**

- A pure, terminal-agnostic cell renderer: `Canvas`, `Cell`, `Style`, `Color`,
  `Rect`, and a clipped local-coordinate `CanvasView`.
- A **fully opaque, application-owned background**: the theme paints every cell,
  and foreground-only draws inherit the surface, so the terminal's own theme (or
  a transparent desktop) never shows through.
- A theme expressed as roles: background, surface, raised surface, border, text,
  muted text, accents, and a magical-effect colour.
- `Frame` composition that separates **cell rendering** from **image placement**.
- An optional **sixel graphics backend** for image-backed art, with an ASCII
  fallback that is drawn whenever the terminal cannot support one.
- Composition primitives (`crop` / `blit`) with deterministic paint-order
  layering, and ASCII borders.
- An enforced text width policy, so the grid cannot desynchronise.
- **Diffed presentation**: only changed cells are written.
- Time-based animation with an explicitly advanced, testable clock.
- A screen stack with push/pop, input routed to the active screen, and layered
  composition, plus a curtain transition between screens.
- A **minimum-viewport policy** (80×24): below it, play suspends safely and
  resumes on its own.
- The presentation study (`F3` or `--study`) and the renderer showcase (`F2`).

## Not implemented yet

Combat, spells, inventory, items you can pick up, quests, skill trees, and
story. There is one NPC and no dialogue branching or scripting. Areas are tiny
and hand-authored.

## Prerequisites

- A stable Rust toolchain (pinned via `rust-toolchain.toml`). Install with
  [rustup](https://rustup.rs/).
- A terminal with ANSI escape support, at least 80×24, reporting 256 colours or
  truecolour. The richest palette is used where available.
- **For image-backed art, a sixel-capable terminal** (verified on `foot`;
  xterm with `-ti`, mlterm, Contour, WezTerm, iTerm2, and Windows Terminal 1.22+
  also implement sixel). Without one Grimmoire draws an ASCII fallback and
  everything else works unchanged. Kitty and iTerm2 graphics are detected but not
  implemented. See [`docs/renderer-design.md`](docs/renderer-design.md) for the
  evidence and the honest limits.

## Build and run

```sh
cargo run                # run a debug build
cargo run --release      # run an optimised build
cargo run --release -- --study   # the presentation study
cargo build --release    # produce target/release/grimmoire
```

Resize the terminal freely; the layout adapts, and below 80×24 play suspends to a
short explanation rather than drawing a mangled layout. The terminal is restored
on a normal exit, on a recoverable error, and from a panic.

## Development

Run the same checks that CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

## Architecture

| Module | Responsibility |
| --- | --- |
| `game` | The world model and its rules: areas, tiles, entities, movement, collision, interaction, travel. Pure — no rendering, no I/O. |
| `save` | A small versioned format and the only code that touches the save file. |
| `render` | Pure, terminal-agnostic drawing: cells, styles, colours, rectangles, canvases, views, `Frame` composition, and CPU-side `Image`s. |
| `terminal` | The **only** module that talks to the terminal: raw mode, alternate screen, input, capability detection, diffed presentation, image encoding, and protocol bytes. |
| `anim` | Time-based animation with an explicitly advanced clock. |
| `input` | Terminal-independent key and event types. |
| `ui` | Frame composition: theme, layout, art, screens, transitions, the screen stack, the presentation study, and the viewport policy. |
| `app` | Wires input, update, compose, and present together. |

Documentation:

- [`docs/renderer-design.md`](docs/renderer-design.md) — the rendering
  architecture, the audit that found a background-ownership defect, the options
  compared, the measured numbers, the graphics evidence, and the migration path.
- [`docs/gameplay.md`](docs/gameplay.md) — the world model, controls,
  interaction rules, and save format.

### Key decisions

- **`crossterm` is the sole terminal library.** It handles raw mode, the
  alternate screen, events with timeouts, and escape sequences, while Grimmoire
  owns its drawing model. `ratatui` was rejected because it would own that model.
- **Cells first, images second.** The cell renderer is the portable baseline and
  always works. A sixel image backend is layered on for terminals that support
  it, behind a `Frame`/`ImagePlacement` boundary so no protocol detail reaches
  the game.
- **Sixel, and only sixel.** It is the one graphics protocol that could be
  verified at runtime here, and it has the widest reach. Kitty and iTerm2 are
  detected but not implemented.
- **The viewport owns its background.** Foreground-only draws inherit the surface
  colour, so the terminal's theme can never leak into the game.
- **A hard minimum viewport (80×24).** Grimmoire is designed for a large
  terminal and suspends safely below it rather than degrading into noise.
- **No ECS, no scripting engine, no widget toolkit** until a phase needs one.
- **Game logic is pure.** Movement, collision, interaction, and travel are
  testable without a terminal; the UI decides how things look.
- **Persistence is narrow and fail-soft.** Only area, position, and discovery are
  stored; anything unrecognised means "start a new game".

## Known limitations

- Double-width and zero-width glyphs are not rendered; the cell model has no
  continuation cell. Block and box glyphs, which are one column wide, are usable.
- The sixel encoder uses a fixed 6×6×6 palette, so smooth gradients band.
- Image drawing requires the terminal to report its character cell size; when it
  does not, Grimmoire draws the ASCII fallback rather than guessing.
- Kitty and iTerm2 graphics are not implemented. Multiplexers may swallow sixel;
  this has not been tested.
- Presentation repaints per changed cell rather than tracking damage rectangles.
- Animation is capped at roughly 30 frames per second by the loop.
- Movement is throttled to a fixed step interval, which limits how fast a held
  key can walk.
- Only one area transition pair exists, and the world is tiny.
- The renderer, the game, and the terminal backend have been exercised on Linux;
  visual verification was done on `foot`.
- Terminal restoration cannot be guaranteed against `SIGKILL` or power loss.
- Grimmoire cannot change the terminal *window's* opacity; only the rendered
  viewport is opaque.

## License

No license has been chosen yet, so none is included. This is an open decision and
must be settled before the project is distributed or reused. See `AGENTS.md`.
