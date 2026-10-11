//! Grimmoire — a terminal-native, ASCII-first dark-fantasy wizard RPG.
//!
//! This crate is an early-development foundation. It currently provides the
//! terminal backend, a small cell-based renderer, and a title screen that
//! demonstrates both. Gameplay systems are intentionally absent; see
//! `ROADMAP.md` in the repository root for what comes next.
//!
//! # Architecture
//!
//! The crate is split along three boundaries:
//!
//! * [`render`] is pure and terminal-agnostic: cells, styles, rectangles, and
//!   the [`render::Canvas`] that frames are composed into.
//! * [`terminal`] is the only module that talks to the terminal. It owns raw
//!   mode, the alternate screen, input, and frame presentation.
//! * [`ui`] composes frames from state, while [`game`] will eventually own the
//!   world and its rules. [`app`] wires them together and drives the loop.

#![warn(missing_docs)]

pub mod app;
pub mod game;
pub mod render;
pub mod terminal;
pub mod ui;
