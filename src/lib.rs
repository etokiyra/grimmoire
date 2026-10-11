//! Grimmoire — a terminal-native, ASCII-first dark-fantasy wizard RPG.
//!
//! This crate is an early-development exploration prototype: it provides a
//! terminal backend, a cell-based renderer with layering and diffed
//! presentation, a time-based animation system, a screen stack, a small playable
//! world with two connected areas, and fail-soft persistence of exploration
//! progress. Combat, dialogue scripting, quests, and progression are absent; see
//! `ROADMAP.md`.
//!
//! # Architecture
//!
//! * [`render`] is pure and terminal-agnostic: cells, styles, canvases, views,
//!   and the text width policy. It has no dependency on any terminal library.
//! * [`terminal`] is the only module that talks to the terminal: raw mode, the
//!   alternate screen, input, capability detection, and diffed presentation.
//! * [`anim`] provides time-based animation driven by an injectable clock.
//! * [`input`] defines terminal-independent input events.
//! * [`game`] owns the world model and its rules — movement, collision,
//!   interaction, and area travel — with no rendering or I/O.
//! * [`assets`] loads image artwork; only the terminal layer turns it into
//!   protocol output.
//! * [`save`] persists essential exploration state in a small versioned format.
//! * [`ui`] composes frames from state and owns the screen stack.
//! * [`app`] wires input, update, compose, and present together.

#![warn(missing_docs)]

pub mod anim;
pub mod app;
pub mod assets;
pub mod game;
pub mod input;
pub mod render;
pub mod save;
pub mod terminal;
pub mod ui;
