# Grimmoire

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust.

[![CI](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml/badge.svg)](https://github.com/etokiyra/grimmoire/actions/workflows/ci.yml)

> **Status: early development.** There is no stable release and no tagged
> version yet. Grimmoire currently ships a working terminal foundation and a
> title screen — not a playable game. Everything below reflects the state of the
> repository, not the long-term vision.

## What Grimmoire is

Grimmoire is a game about a wizard who grows stronger through knowledge rather
than gear. It is being built for the terminal and takes ASCII art as its primary
visual language. The long-term vision includes:

- A custom terminal renderer with flexible layouts, colour, and animation.
- ASCII-art environments, characters, enemies, and magical effects.
- Exploration of towns, forests, dungeons, and other locations.
- Character-focused dialogue with ASCII portraits and meaningful choices.
- Strategic, turn-based combat based exclusively on magic.
- Hybrid progression: discoveries and story milestones alongside gameplay mastery.
- Items that serve exploration, puzzles, quests, and storytelling, not raw power.
- Persistent save files.

The story, world, characters, and detailed mechanics are **deliberately
undecided**. See [`ROADMAP.md`](ROADMAP.md) for the phasing that will settle them.

## Implemented today

- A Cargo application project named `grimmoire` that builds a `grimmoire` binary.
- A terminal backend that safely enters and leaves raw mode and the alternate
  screen, restores state on normal exit, on the error path, and from a panic.
- A small, pure, cell-based renderer (`Canvas`, `Cell`, `Style`, `Color`, `Rect`).
- A title screen composed from ASCII art, colour, and a simple layout.
- Keyboard input: menu navigation, quit, and a status message for entries that
  have no behaviour yet.
- Unit tests covering the rendering primitives and the title screen.

## Not implemented yet

Combat, quests, dialogue, exploration, skill trees, items, save files, and a full
game engine are all absent. So is any notion of a playable world. Do not expect
gameplay.

## Prerequisites

- A stable Rust toolchain (the repository pins the `stable` channel via
  `rust-toolchain.toml`). Install it with [rustup](https://rustup.rs/).
- A terminal that supports ANSI escape sequences and at least a 16-colour
  palette. 256-colour and truecolour terminals are used when available.

## Build and run

```sh
cargo run              # run a debug build
cargo run --release    # run an optimised build
cargo build --release  # produce target/release/grimmoire
```

## Controls

| Key                | Action                          |
| ------------------ | ------------------------------- |
| `Up`               | Move the menu highlight up      |
| `Down`             | Move the menu highlight down    |
| `Enter`            | Activate the highlighted entry  |
| `q` / `Esc`        | Quit                            |
| `Ctrl`+`C`         | Quit                            |

The terminal is restored when you quit.

## Development

Run the same checks that CI runs:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

## Architecture

The crate is split along three boundaries so that each concern can evolve — and
be tested — independently.

| Module      | Responsibility                                                                 |
| ----------- | ------------------------------------------------------------------------------ |
| `render`    | Pure, terminal-agnostic drawing: cells, styles, colours, rectangles, the canvas. |
| `terminal`  | The **only** module that talks to the terminal: raw mode, alt screen, input, presentation. |
| `ui`        | Composes frames from state. Knows nothing about escape sequences or rules.       |
| `game`      | Where world state and rules will live. A documented placeholder today.           |
| `app`       | Wires input, update, compose, and present together and drives the loop.          |

### Decisions and reasoning

- **Terminal library: `crossterm`.** It handles the fiddly, platform-specific
  parts — raw mode, the alternate screen, input events, escape sequences — while
  leaving the drawing model to us. `termion` is Unix-only and less actively
  maintained; `ratatui` is a full widget/TUI framework that would own the
  drawing model Grimmoire intends to control. Only one direct dependency is used.
- **Cell-based rendering model.** Frames are composed into a rectangular grid of
  `Cell`s (`char` + `Style`). This keeps composition deterministic and pure, which
  makes it testable without a terminal and gives future work a natural place to add
  damage/redraw diffing, animation layering, and multiple layouts. The foundation
  is intentionally small: no widget toolkit, no retained scene graph, no hidden
  layout engine.
- **ASCII-first borders.** The built-in border primitive uses `+`, `-`, and `|`
  rather than Unicode box-drawing glyphs, so visuals survive limited terminals and
  font setups. The cell model still holds one `char`, so richer glyphs can be
  adopted later without changing the model.
- **Full repaint per frame.** The backend currently repaints the whole canvas each
  frame with per-cell colour diffing. This is more than sufficient at the current
  scale and keeps the backend simple; incremental redraw is a deliberate future
  optimisation, not a missing feature.
- **Terminal restore is layered.** `TerminalBackend::leave` is idempotent and is
  called on the normal exit path; `Drop` restores the terminal if the backend is
  dropped early; a panic hook restores it before the panic message is printed.

## Known limitations

- Wide (double-width) glyphs and combining marks are not supported; `Cell` holds
  exactly one `char`.
- Rendering repaints the whole frame each tick; there is no diffed/incremental
  update path yet.
- The bottom-right cell is intentionally left unpainted to avoid an implicit
  terminal scroll.
- No animation timeline yet. The main loop has a tick but nothing consumes it.
- Mouse input is not handled.
- The renderer and terminal backend have only been exercised on Linux.

## License

No license has been chosen yet, so none is included. This is an open decision and
must be settled before the project is distributed or reused. See `AGENTS.md`.
