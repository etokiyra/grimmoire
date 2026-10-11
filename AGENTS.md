# AGENTS.md

Repository guidance for coding agents working on Grimmoire. Keep it concise and
current.

## What Grimmoire is

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust. The
project is in early development: it currently provides a playable exploration
prototype — two connected areas, tile movement, interaction, one NPC, and
fail-soft saving — on top of a cell-based renderer, animation system, and screen
stack. It is not a complete game.

## Vision and confirmed decisions

These are settled and should guide new work:

- **Terminal-first, ASCII-first.** ASCII art is the primary visual language.
  Single-column block and box glyphs (`█`, `░`) are legitimate shading and
  swatches; wide glyphs are not used.
- **Cells are the baseline; images are optional.** The cell renderer must always
  produce a complete, correct screen. Any graphics capability is an additive
  layer behind the `Frame`/`ImagePlacement` boundary, never a requirement.
- **Sixel is the only graphics protocol implemented.** Kitty and iTerm2 are
  detected and reported but select no backend. Do not add a second protocol
  without evidence that it is needed and can be verified.
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
- **No ECS, no scripting engine, no widget toolkit** until a phase concretely
  needs one. The current world is a grid plus lists.
- **No graphics protocol is implemented beyond sixel.** The capability probe is
  three-valued and must stay that way: treat `Unknown` as "no", never as support.
  A detected-but-unimplemented protocol must select no backend rather than a
  half-working one.
- **Minimal dependencies.** Two direct dependencies is the current bar:
  `crossterm` and `unicode-width`. Every new direct dependency needs a
  justification in the pull request or commit message and in `Cargo.toml`.

Explicitly **undecided** (do not treat as settled, and do not invent them): the
story, world, characters, detailed mechanics, art direction beyond "ASCII", and
licensing. All current area names, entities, and dialogue lines are placeholders.

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
  `Frame`, `ImagePlacement`, `Cell`, `Style`, `Color`, `Rect`, `Image`, `glyph`).
  **No** terminal or game code, and no dependency on a terminal library.
- `src/anim/` — time-based animation. **No** terminal or UI code. Progress is a
  function of elapsed time, advanced explicitly; never frame counts.
- `src/terminal/` — the **only** module allowed to talk to the terminal, and the
  **only** place that emits graphics-protocol bytes (`sixel::encode`).
- `src/ui/viewport.rs` — owns the minimum-viewport policy. Screens must not grow
  their own "too small" behaviour; the application suspends for them.
- `src/input.rs` — terminal-independent `Key`/`Event` types. The backend
  translates library events into these, including the auto-repeat flag.
- `src/ui/` — composes frames from state and owns the screen stack. No I/O, no
  escape sequences, no game rules.
- `src/app.rs` — wires input → update → compose → present and drives the loop.

Additional rules:

- Screens must never write to the terminal; they compose into a `Frame` and
  return `Command`s from `handle_key`.
- Screens never encode or position an image. They record an `ImagePlacement` and
  draw a fallback in the same rectangle; the presenter reserves the rectangle and
  decides whether to paint pixels there.
- Input goes to the active (top) screen only; composition runs bottom to top.
  This is what stops movement while a dialogue panel is open — do not replace it
  with a mode flag inside the game.
- Discrete actions (`e`, help, screen changes) must ignore auto-repeat. Movement
  is paced by a fixed step interval instead of the repeat flag, because classic
  terminals report repeats as ordinary presses.
- The text width policy in `render::glyph` is the single source of truth for what
  may be drawn.
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
graphics.

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

## Documentation

After substantial changes, update the documentation in the same change:

- `README.md` — user-facing status, controls, build/run, architecture.
- `docs/gameplay.md` — the world model, controls, interaction rules, save format,
  and where the code lives.
- `docs/renderer-design.md` — rendering, presentation, animation, and backend
  decisions.
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
