# Changelog

All notable changes to Grimmoire are documented here. The project is in early
development and has **no tagged release yet**; everything is under `Unreleased`.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project intends to follow [Semantic Versioning](https://semver.org/) once
it starts releasing.

## [Unreleased]

### Added

- Cargo application project named `grimmoire` (binary crate `grimmoire`), with
  `Cargo.lock` committed.
- Terminal backend around `crossterm` that enters/leaves raw mode and the
  alternate screen, restores terminal state on normal exit, on the error path,
  and from a panic, and polls input events.
- Pure, cell-based rendering primitives: `Canvas`, `Cell`, `Style`, `Color`, and
  `Rect`, with ASCII border drawing and centred-text helpers.
- A title screen (`ui::title`) composing ASCII art, colour, and a small layout,
  with an interactive menu and a status line.
- `app::App` input → update → compose → present loop, with keyboard controls
  (`Up`/`Down`, `Enter`, `q`/`Esc`, `Ctrl`+`C`).
- Unit tests for the rendering primitives and the title screen.
- GitHub Actions CI running formatting, Clippy, tests, and a release build.
- Documentation: `README.md`, `AGENTS.md`, `ROADMAP.md`, and this changelog.

### Notes

- No license has been chosen yet, so none is included.
- The story, setting, characters, and detailed mechanics remain undecided.
