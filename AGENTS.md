# AGENTS.md

Repository guidance for coding agents working on Grimmoire. Keep it concise and
current.

## What Grimmoire is

A terminal-native, ASCII-first dark-fantasy wizard RPG written in Rust. The
project is in early development: it currently provides a terminal foundation and
a title screen, not a playable game.

## Vision and confirmed decisions

These are settled and should guide new work:

- **Terminal-first, ASCII-first.** ASCII art is the primary visual language.
- **Magic-only, turn-based combat** (planned; not implemented).
- **Knowledge-driven progression.** A hybrid of discoveries/story milestones and
  gameplay mastery. Items support exploration, puzzles, quests, and storytelling
  rather than equipment-based power scaling.
- **Custom renderer, borrowed plumbing.** Grimmoire owns its drawing model,
  layout, frame composition, and animation. It relies on libraries for low-level
  terminal operations and does **not** build a terminal emulator.
- **`crossterm` is the sole terminal library.** Do not add `ratatui`, `termion`,
  or a second backend without a strong, documented reason.
- **Minimal dependencies.** Every new direct dependency needs a justification in
  the pull request or commit message and, where practical, in `Cargo.toml`.

Explicitly **undecided** (do not treat as settled, and do not invent them):
the story, world, characters, detailed mechanics, art direction beyond "ASCII",
and licensing.

## Architectural boundaries

Respect these; they are the point of the current structure:

- `src/render/` — pure, terminal-agnostic drawing primitives (`Canvas`, `Cell`,
  `Style`, `Color`, `Rect`). No terminal or game code here. Must stay testable
  without a terminal.
- `src/terminal/` — the **only** module allowed to talk to the terminal (raw
  mode, alternate screen, input, escape sequences, presentation).
- `src/ui/` — composes frames from state into a `Canvas`. No rules, no escape
  sequences.
- `src/game.rs` — where world state and rules will live. Keep rendering out of
  it and keep it out of rendering.
- `src/app.rs` — wires input → update → compose → present and drives the loop.

If a change crosses a boundary, call it out and explain why.

## Code quality

Run these before reporting work complete; CI runs the same commands:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build --release
```

Clippy warnings are errors. Keep the code formatted. Prefer small, focused units
with focused tests over broad, shallow coverage.

## Verification discipline

- **Verify claims before reporting completion.** Never report a command, a build,
  a test, a push, or a CI run as successful unless you actually observed the
  result. "Attempted" is not "succeeded".
- When you state that interactive or terminal behaviour works, say how it was
  verified (for example, run under a pseudo-terminal) or state that it was not.
- Do not claim remote CI passed unless you have seen its result.

## Documentation

After substantial changes, update the documentation in the same change:

- `README.md` — user-facing status, controls, build/run, architecture.
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
