# Grimmoire

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust.

[![CI](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml/badge.svg)](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml)

> **Status: early development.** There is no stable release and no tagged
> version yet. Grimmoire currently provides a working terminal application: a
> cell-based renderer with animation, layered composition and diffed output, plus
> an interactive renderer showcase. It is **not** a playable game.

## What Grimmoire is

A game about a wizard who grows stronger through knowledge rather than gear,
built for the terminal and taking ASCII art as its primary visual language. The
long-term vision includes exploration, character-focused dialogue with ASCII
portraits, strategic turn-based magic combat, skill trees, story-and-mastery
progression, and persistent saves.

The story, world, characters, and detailed mechanics are **deliberately
undecided**. See [`ROADMAP.md`](ROADMAP.md) for phasing.

## Implemented today

**Renderer**

- A pure, terminal-agnostic cell renderer: `Canvas`, `Cell`, `Style`, `Color`,
  `Rect`, and a clipped local-coordinate `CanvasView` for independent regions.
- Composition primitives (`crop` / `blit`) with deterministic paint-order
  layering, and ASCII borders.
- An enforced ASCII-first text width policy: glyphs that are not exactly one
  column wide are dropped, so the grid cannot desynchronise.
- **Diffed presentation**: only changed cells are written, grouped into runs.
  An unchanged frame writes 0 cells.

**Animation**

- Time-based (not frame-count) animations with durations, delays, easing,
  concurrent tracks, completion, and cancellation; the clock is advanced
  explicitly, so animation is deterministic and testable.

**Application**

- A screen stack with push/pop, input routed to the active screen, and layered
  composition so overlays draw over the screen beneath them.
- A full-screen curtain transition on screen changes.
- Runtime capability detection (colour depth, and a best-effort graphics hint)
  that selects the palette.
- Responsive layouts: two-column, stacked, and a minimal fallback for terminals
  that are too small.
- Idle-aware loop: continuous frames only while something is animating.

**The showcase**

- A distinctive Grimmoire banner with a left-to-right reveal animation.
- Three animated ASCII scenes (Emberfall, Sigil, Starfield) drawn from a
  deterministic function of time.
- A pulsing highlight, a scene-change wipe, an information panel with live
  renderer diagnostics, and a status bar.
- A help & diagnostics overlay.

## Not implemented yet

Gameplay is absent: no exploration, dialogue, combat, quests, skill trees,
items, inventory, or save files. There is no world or story content.

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

## Controls

| Key | Action |
| --- | --- |
| `Left` / `Right` / `Up` / `Down`, `1`–`3` | Choose a scene |
| `Space` | Pause or resume motion |
| `+` / `-` | Increase or decrease animation speed |
| `r` | Replay the title reveal |
| `h`, `?`, `F1` | Toggle the help & diagnostics overlay |
| `Esc` | Close the overlay, or quit when no overlay is open |
| `q`, `Ctrl`+`C` | Quit (always available) |

Resize the terminal freely; the layout adapts. Below 44x13 columns the interface
falls back to a small notice rather than drawing a broken layout. The terminal is
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
| `render` | Pure, terminal-agnostic drawing: cells, styles, colours, rectangles, canvases, views, and the text width policy. |
| `terminal` | The **only** module that talks to the terminal: raw mode, alternate screen, input, capability detection, and diffed presentation. |
| `anim` | Time-based animation with an explicitly advanced clock. |
| `input` | Terminal-independent key and event types. |
| `ui` | Frame composition: theme, layout, art, screens, transitions, and the screen stack. |
| `game` | Where world state and rules will live (a documented placeholder today). |
| `app` | Wires input, update, compose, and present together and drives the loop. |

For the full design — including the terminal-library and graphics-protocol
evaluations, measured numbers, and extension points — see
[`docs/renderer-design.md`](docs/renderer-design.md).

### Key decisions

- **`crossterm` is the sole terminal library.** It handles raw mode, the
  alternate screen, events with timeouts, and escape sequences, while Grimmoire
  owns its drawing model. `ratatui` was rejected because it would own that model.
- **Cell-based rendering with diffed output.** Frames are pure and testable
  without a terminal; only changes are written.
- **ASCII-first, enforced.** Glyphs that are not exactly one column wide are
  dropped, so the grid always stays aligned.
- **No graphics protocol (yet).** Detection exists for diagnostics only; see the
  design document for the rationale and the extension point.

## Known limitations

- Double-width and zero-width glyphs are not rendered; the cell model has no
  continuation cell. This is deliberate and documented.
- Presentation repaints per changed cell rather than tracking damage rectangles.
- Animation is capped at roughly 30 frames per second by the loop.
- The renderer and terminal backend have been exercised on Linux.
- Restoration cannot be guaranteed against `SIGKILL` or power loss.

## License

No license has been chosen yet, so none is included. This is an open decision and
must be settled before the project is distributed or reused. See `AGENTS.md`.
