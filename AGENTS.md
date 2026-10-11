# AGENTS.md

Repository guidance for coding agents working on Grimmoire. Keep it concise and
current.

## What Grimmoire is

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust. The
project is in early development: it currently provides a terminal application with
a cell-based renderer, animation, a screen stack, and an interactive renderer
showcase — not a playable game.

## Vision and confirmed decisions

These are settled and should guide new work:

- **Terminal-first, ASCII-first.** ASCII art is the primary visual language.
- **Magic-only, turn-based combat** (planned; not implemented).
- **Knowledge-driven progression.** A hybrid of discoveries/story milestones and
  gameplay mastery. Items support exploration, puzzles, quests, and storytelling
  rather than equipment-based power scaling.
- **Custom renderer, borrowed plumbing.** Grimmoire owns its drawing model,
  layout, frame composition, and animation. It relies on libraries for low-level
  terminal operations and does **not** build a terminal emulator or a pixel
  renderer.
- **`crossterm` is the sole terminal library.** Do not add `ratatui`, `termion`,
  or a second backend without a strong, documented reason. `ratatui` in
  particular would own the drawing model.
- **No graphics protocol is implemented.** The capability probe is informational
  only. Do not wire it into rendering without a documented design in
  `docs/renderer-design.md`.
- **Minimal dependencies.** Two direct dependencies is the current bar:
  `crossterm` and `unicode-width`. Every new direct dependency needs a
  justification in the pull request or commit message and in `Cargo.toml`.

Explicitly **undecided** (do not treat as settled, and do not invent them): the
story, world, characters, detailed mechanics, art direction beyond "ASCII", and
licensing.

## Architecture invariants

Respect these; they are the point of the current structure:

- `src/render/` — pure, terminal-agnostic drawing (`Canvas`, `CanvasView`, `Cell`,
  `Style`, `Color`, `Rect`, `glyph`). **No** terminal or game code, and no
  dependency on a terminal library. Must stay testable without a terminal.
- `src/anim/` — time-based animation. **No** terminal or UI code. Progress is a
  function of elapsed time, advanced explicitly; never frame counts.
- `src/terminal/` — the **only** module allowed to talk to the terminal (raw
  mode, alternate screen, input, capabilities, escape sequences, presentation).
- `src/input.rs` — terminal-independent `Key`/`Event` types. The backend
  translates library events into these.
- `src/ui/` — composes frames from state and owns the screen stack. No I/O, no
  escape sequences, no game rules.
- `src/game.rs` — where world state and rules will live. Keep rendering out of it
  and keep it out of rendering.
- `src/app.rs` — wires input → update → compose → present and drives the loop.

Additional rules:

- Screens must never write to the terminal; they compose into a `Canvas` and
  return `Command`s from `handle_key`.
- Input goes to the active (top) screen only; composition runs bottom to top.
- The text width policy in `render::glyph` is the single source of truth for
  what may be drawn. Do not bypass it with raw `Cell` writes that put wide or
  control glyphs into the grid.

If a change crosses a boundary, call it out and explain why. Read
`docs/renderer-design.md` before changing rendering, presentation, or animation.

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
terminal: assert on composed `Canvas`es and on presenter output captured into a
`Vec<u8>`.

## Verification discipline

- **Verify claims before reporting completion.** Never report a command, build,
  test, push, or CI run as successful unless you observed the result.
  "Attempted" is not "succeeded".
- Do not claim performance improvements without a measurement. The presenter's
  own statistics and CPU-time measurements are the accepted forms of evidence
  here; record numbers, not adjectives.
- When you state that interactive or terminal behaviour works, say how it was
  verified (for example, driven through a pseudo-terminal) or state that it was
  not.
- Do not claim remote CI passed unless you have seen its result.

## Documentation

After substantial changes, update the documentation in the same change:

- `README.md` — user-facing status, controls, build/run, architecture.
- `docs/renderer-design.md` — rendering, presentation, animation, and backend
  decisions, plus constraints and extension points.
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
