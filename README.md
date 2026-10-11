# Grimmoire

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust.

[![CI](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml/badge.svg)](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml)

> **Status: early development.** There is no stable release and no tagged
> version yet. Grimmoire is now a small, playable **exploration prototype**: you
> can walk around two connected areas, examine objects, speak to one NPC, and
> have your progress saved. It is not a complete game.

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

| Key | Action |
| --- | --- |
| `w` `a` `s` `d` or arrow keys | Move (and face that way) |
| `e` | Interact with what you face |
| `h`, `?`, `F1` | Help and renderer diagnostics |
| `F2` | Renderer showcase (development screen) |
| `Esc` | Quit |
| `q`, `Ctrl`+`C` | Quit |

Progress saves automatically on quit and whenever you move between areas; see
[`docs/gameplay.md`](docs/gameplay.md) for the save location and format.

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
- Composition primitives (`crop` / `blit`) with deterministic paint-order
  layering, and ASCII borders.
- An enforced ASCII-first text width policy, so the grid cannot desynchronise.
- **Diffed presentation**: only changed cells are written.
- Time-based animation with an explicitly advanced, testable clock.
- A screen stack with push/pop, input routed to the active screen, and layered
  composition, plus a curtain transition between screens.
- Runtime capability detection selecting the palette.
- An idle-aware loop: continuous frames only while something is animating.
- The renderer showcase (`F2`): animated ASCII scenes and live diagnostics.

## Not implemented yet

Combat, spells, inventory, items you can pick up, quests, skill trees, and
story. There is one NPC and no dialogue branching or scripting. Areas are tiny
and hand-authored.

## Prerequisites

- A stable Rust toolchain (pinned via `rust-toolchain.toml`). Install with
  [rustup](https://rustup.rs/).
- A terminal with ANSI escape support. 256-colour and truecolour terminals get a
  richer palette; 16-colour terminals get the portable palette.

## Build and run

```sh
cargo run              # run a debug build
cargo run --release    # run an optimised build
cargo build --release  # produce target/release/grimmoire
```

Resize the terminal freely; the layout adapts. Below 24x8 the interface falls
back to a small notice rather than drawing a broken layout. The terminal is
restored on a normal exit, on a recoverable error, and from a panic.

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
| `render` | Pure, terminal-agnostic drawing: cells, styles, colours, canvases, views, text width policy. |
| `terminal` | The **only** module that talks to the terminal: raw mode, alternate screen, input, capability detection, diffed presentation. |
| `anim` | Time-based animation with an explicitly advanced clock. |
| `input` | Terminal-independent key and event types. |
| `ui` | Frame composition: theme, layout, art, screens, transitions, and the screen stack. |
| `app` | Wires input, update, compose, and present together. |

Documentation:

- [`docs/gameplay.md`](docs/gameplay.md) — the world model, controls,
  interaction rules, and save format.
- [`docs/renderer-design.md`](docs/renderer-design.md) — the renderer
  architecture, backend and graphics-protocol evaluation, measured numbers, and
  extension points.

### Key decisions

- **`crossterm` is the sole terminal library.** It handles raw mode, the
  alternate screen, events with timeouts, and escape sequences, while Grimmoire
  owns its drawing model. `ratatui` was rejected because it would own that model.
- **No ECS.** Two small hand-authored areas need a grid, a list of entities, and
  a list of doorways. An entity-component system would be speculative.
- **Game logic is pure.** Movement, collision, interaction, and travel are
  testable without a terminal; the UI decides how things look.
- **Dialogue is a pushed screen.** Input routing then gives the "cannot move
  while talking" behaviour for free, instead of a mode flag inside the game.
- **Persistence is narrow and fail-soft.** Only area, position, and discovery
  are stored; anything unrecognised means "start a new game" rather than a
  half-loaded save.
- **ASCII-first, enforced.** Glyphs that are not exactly one column wide are
  dropped, so the grid always stays aligned.
- **No graphics protocol (yet).** Detection exists for diagnostics only.

## Known limitations

- Double-width and zero-width glyphs are not rendered; the cell model has no
  continuation cell. This is deliberate and documented.
- Presentation repaints per changed cell rather than tracking damage rectangles.
- Animation is capped at roughly 30 frames per second by the loop.
- Movement is throttled to a fixed step interval, which limits how fast a held
  key can walk.
- Only one area transition pair exists, and the world is tiny.
- The renderer, the game, and the terminal backend have been exercised on Linux.
- Terminal restoration cannot be guaranteed against `SIGKILL` or power loss.

## License

No license has been chosen yet, so none is included. This is an open decision and
must be settled before the project is distributed or reused. See `AGENTS.md`.
