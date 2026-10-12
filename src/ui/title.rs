//! The title screen: the first thing the game says.
//!
//! A game should not drop the player into a room as though it were a command that
//! ran. This is the opening: a night sky over the chapel, the name composed out of
//! glyphs, and a menu that knows whether there is a journey to continue.
//!
//! # What is animated
//!
//! Everything here is a function of one number — how long the screen has been
//! open — so nothing drifts, and nothing needs to be stored: stars twinkle on a
//! hash of their position and the clock, motes rise, the logo reveals left to
//! right, and the menu items arrive one after another. With motion turned off the
//! sky is still, the logo is whole, and the menu is simply there.
//!
//! # Destructive actions
//!
//! Starting again when a save exists does not silently replace it. The menu opens
//! a confirmation that says what will be lost, and `esc` backs out of it.

use std::time::Duration;

use super::FrameContext;
use super::art::{self, Banner};
use super::journal::JournalScreen;
use super::ornament;
use super::screen::{Command, Screen};
use super::settings::{Motion, Shared};
use super::theme::Palette;
use crate::game::{Discovery, Progress};
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, CanvasView, Frame, Rect, Style, glyph};
use crate::save::{SaveData, SaveStore};

/// How long the whole opening takes.
const OPENING: Duration = Duration::from_millis(1400);
/// How long the fade into the world takes.
const DEPARTURE: Duration = Duration::from_millis(420);

/// What a menu entry does.
///
/// Public so the entries can be named from outside the screen: tests drive the
/// menu by asking for an action rather than by counting rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Begin again, asking first if there is anything to lose.
    NewGame,
    /// Pick up the saved journey.
    Continue,
    /// Read the saved journal.
    Journal,
    /// Change how the game presents itself.
    Settings,
    /// Leave.
    Quit,
}

/// One row of the menu.
#[derive(Debug, Clone, Copy)]
struct Item {
    action: Action,
    label: &'static str,
    note: &'static str,
    enabled: bool,
}

/// A panel open over the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    /// Confirming that starting again will replace the save.
    ConfirmNewGame,
    /// Choosing how much motion the game should use.
    Settings,
}

/// The title screen.
#[derive(Debug)]
pub struct TitleScreen {
    save: Option<SaveStore>,
    /// What the save holds, if there is a readable one.
    saved: Option<SaveData>,
    portrait: Option<AssetId>,
    settings: Shared,
    items: Vec<Item>,
    selected: usize,
    overlay: Option<Overlay>,
    /// Time since the screen opened.
    age: Duration,
    /// Time since the departure began.
    leaving: Option<Duration>,
    /// What to do once the departure has played.
    departure: Option<Action>,
    /// A line of feedback, for a disabled entry the player insisted on.
    note: String,
}

impl TitleScreen {
    /// Creates the title screen over an optional save.
    #[must_use]
    pub fn new(save: Option<SaveStore>, portrait: Option<AssetId>, settings: Shared) -> Self {
        let saved = save.as_ref().and_then(|store| store.load());
        let has_save = saved.is_some();
        let has_journal = saved.as_ref().is_some_and(|data| !data.journal.is_empty());
        let items = vec![
            Item {
                action: Action::NewGame,
                label: "New Game",
                note: if has_save { "begin again" } else { "begin" },
                enabled: true,
            },
            Item {
                action: Action::Continue,
                label: "Continue",
                note: if has_save { "resume" } else { "no journey yet" },
                enabled: has_save,
            },
            Item {
                action: Action::Journal,
                label: "Journal",
                note: if has_journal {
                    "what you found"
                } else {
                    "nothing found yet"
                },
                enabled: has_journal,
            },
            Item {
                action: Action::Settings,
                label: "Settings",
                note: "how the game moves",
                enabled: true,
            },
            Item {
                action: Action::Quit,
                label: "Quit",
                note: "leave",
                enabled: true,
            },
        ];
        let mut screen = TitleScreen {
            save,
            saved,
            portrait,
            settings,
            items,
            selected: 0,
            overlay: None,
            age: Duration::ZERO,
            leaving: None,
            departure: None,
            note: String::new(),
        };
        screen.select_first_enabled();
        screen
    }

    /// The action the highlighted entry would take.
    #[must_use]
    pub fn selected_action(&self) -> Option<Action> {
        self.items.get(self.selected).map(|item| item.action)
    }

    /// Whether the screen is on its way into the world.
    #[must_use]
    pub const fn is_leaving(&self) -> bool {
        self.leaving.is_some()
    }

    /// The panel open over the menu, for tests.
    #[must_use]
    pub const fn overlay(&self) -> Option<Overlay> {
        self.overlay
    }

    /// Whether an entry is currently selectable, for tests.
    #[must_use]
    pub fn is_enabled(&self, action: Action) -> bool {
        self.items
            .iter()
            .find(|item| item.action == action)
            .is_some_and(|item| item.enabled)
    }

    /// Moves the selection to the first entry that can be taken.
    fn select_first_enabled(&mut self) {
        if let Some(index) = self.items.iter().position(|item| item.enabled) {
            self.selected = index;
        }
    }

    /// Moves the selection by `delta`, skipping anything unselectable.
    fn move_selection(&mut self, delta: i32) {
        let count = self.items.len();
        if count == 0 {
            return;
        }
        let mut index = self.selected as i32;
        for _ in 0..count {
            index = (index + delta).rem_euclid(count as i32);
            if self.items[index as usize].enabled {
                self.selected = index as usize;
                return;
            }
        }
    }

    /// Whether the opening has played out.
    #[must_use]
    fn settled(&self, motion: Motion) -> bool {
        !motion.animates() || self.age >= OPENING
    }

    /// Starts the fade into the world.
    fn depart(&mut self, action: Action) {
        self.departure = Some(action);
        self.leaving = Some(Duration::ZERO);
    }

    /// How far the opening has come, from 0 to 1.
    #[must_use]
    fn opening(&self, motion: Motion) -> f32 {
        if !motion.animates() {
            return 1.0;
        }
        (self.age.as_secs_f32() / OPENING.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// Takes the highlighted entry, opening a panel or leaving.
    fn activate(&mut self) {
        let Some(item) = self.items.get(self.selected).copied() else {
            return;
        };
        if !item.enabled {
            self.note = format!("{} is not available.", item.label);
            return;
        }
        match item.action {
            Action::NewGame if self.saved.is_some() => {
                self.overlay = Some(Overlay::ConfirmNewGame);
            }
            Action::NewGame => self.depart(Action::NewGame),
            Action::Continue => self.depart(Action::Continue),
            // The journal is a screen of its own, pushed from `handle_key`.
            Action::Journal => {}
            Action::Settings => self.overlay = Some(Overlay::Settings),
            Action::Quit => {
                self.depart(Action::Quit);
            }
        }
    }

    /// Builds the world, or quits, now that the departure has played.
    fn arrive(&mut self) -> Command {
        match self.departure {
            Some(Action::Quit) | None => Command::Quit,
            // The journal and the settings panel never leave this screen.
            Some(Action::Journal) | Some(Action::Settings) => Command::None,
            Some(Action::NewGame) | Some(Action::Continue) => Command::Push(Box::new(
                super::exploration::ExplorationScreen::from_save(self.save.clone(), self.portrait),
            )),
        }
    }

    /// The journal of the saved journey, if there is one.
    fn journal(&self) -> Vec<Discovery> {
        self.saved
            .as_ref()
            .map(|data| {
                data.journal
                    .iter()
                    .filter_map(|id| Discovery::from_id(id))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl Screen for TitleScreen {
    fn name(&self) -> &'static str {
        "title"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Only genuine presses act, and auto-repeat must not race down the menu.
        if !key.is_down() || key.is_repeat() || self.is_leaving() {
            return Command::None;
        }

        // A panel takes the keyboard while it is open.
        match self.overlay {
            Some(Overlay::ConfirmNewGame) => {
                return match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('y') => {
                        self.overlay = None;
                        self.depart(Action::NewGame);
                        Command::None
                    }
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
                        self.overlay = None;
                        self.note = "Nothing was lost.".to_string();
                        Command::None
                    }
                    _ => Command::None,
                };
            }
            Some(Overlay::Settings) => {
                return match key.code {
                    KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('d') => {
                        self.settings.cycle_motion();
                        Command::None
                    }
                    KeyCode::Left | KeyCode::Char('a') => {
                        // Cycling backwards is not worth a second direction of
                        // travel for three values; left advances too, so a player
                        // who presses the other arrow is not stuck.
                        self.settings.cycle_motion();
                        Command::None
                    }
                    KeyCode::Esc | KeyCode::Char('s') => {
                        self.overlay = None;
                        Command::None
                    }
                    _ => Command::None,
                };
            }
            None => {}
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('w') => {
                self.move_selection(-1);
                self.note.clear();
                Command::None
            }
            KeyCode::Down | KeyCode::Char('s') => {
                self.move_selection(1);
                self.note.clear();
                Command::None
            }
            KeyCode::Enter | KeyCode::Char(' ') => match self.selected_action() {
                Some(Action::Journal) if self.is_enabled(Action::Journal) => {
                    let entries = self.journal();
                    Command::Push(Box::new(JournalScreen::new(entries, Progress::total())))
                }
                _ => {
                    self.activate();
                    Command::None
                }
            },
            KeyCode::Esc | KeyCode::Char('q') => Command::Quit,
            _ => Command::None,
        }
    }

    fn update(&mut self, delta: Duration) {
        self.age = self.age.saturating_add(delta);
        if let Some(age) = self.leaving {
            self.leaving = Some(age.saturating_add(delta));
        }
    }

    fn after_update(&mut self) -> Command {
        match self.leaving {
            Some(age) if age >= DEPARTURE => self.arrive(),
            _ => Command::None,
        }
    }

    fn curtain(&self) -> bool {
        // The title brings the world in itself; a curtain on top would double it.
        false
    }

    fn is_animating(&self) -> bool {
        // The sky is alive until the player leaves, unless they asked for less.
        let motion = self.settings.get().motion;
        self.leaving.is_some() || !self.settled(motion)
    }

    fn next_frame(&self) -> Option<Duration> {
        let motion = self.settings.get().motion;
        if self.leaving.is_some() || !self.settled(motion) {
            Some(Duration::ZERO)
        } else if motion.ambient_frames() {
            Some(super::exploration::ambient_period())
        } else {
            None
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let motion = context.motion;
        let area = frame.bounds();
        if area.is_empty() {
            return;
        }
        self.compose_sky(frame, area, context, motion);
        self.compose_logo(frame, area, context, motion);
        self.compose_menu(frame, area, context, motion);
        if let Some(overlay) = self.overlay {
            self.compose_overlay(frame, area, context, overlay);
        }
        if let Some(age) = self.leaving {
            // A closing curtain from the middle outwards, so the world arrives
            // rather than simply replacing the title.
            let t = (age.as_secs_f32() / DEPARTURE.as_secs_f32()).clamp(0.0, 1.0);
            let covered = (f32::from(area.height) * t).round() as u16;
            if covered > 0 {
                let mut view = frame.view(area);
                let band = Rect::new(
                    area.x,
                    area.y + (area.height.saturating_sub(covered)) / 2,
                    area.width,
                    covered,
                );
                view.fill_style(
                    band,
                    Style::new()
                        .with_fg(context.palette.background)
                        .with_bg(context.palette.background),
                );
            }
        }
    }
}

impl TitleScreen {
    /// The sky: stars, a moon, a horizon, and the chapel on it.
    fn compose_sky(&self, frame: &mut Frame, area: Rect, context: &FrameContext, motion: Motion) {
        let palette = &context.palette;
        let mut view = frame.view(area);
        let bounds = view.bounds();
        view.fill_style(
            bounds,
            Style::new()
                .with_fg(palette.accent_alt_dim)
                .with_bg(palette.background),
        );

        let time = if motion.animates() {
            self.age.as_secs_f32()
        } else {
            0.0
        };

        // Stars: one per cell, chosen by a hash of the position, twinkling on the
        // clock. Deterministic, so the same sky comes back every time.
        for y in 0..bounds.height {
            for x in 0..bounds.width {
                let seed =
                    u32::from(x).wrapping_mul(73_856_093) ^ u32::from(y).wrapping_mul(19_349_663);
                if seed % 97 != 0 {
                    continue;
                }
                let twinkle = ((time * 1.6) + (seed % 100) as f32 * 0.13).sin();
                let (ch, style) = if twinkle > 0.7 {
                    (ornament::STAR, Style::new().with_fg(palette.focus).bold())
                } else if twinkle > 0.0 {
                    (
                        ornament::STAR_OPEN,
                        Style::new().with_fg(palette.text_secondary),
                    )
                } else {
                    (ornament::DOT, palette.muted_style())
                };
                view.put_char(x, y, ch, style);
            }
        }

        // The moon, high and to the far right, clear of the name.
        let moon_x = bounds.right().saturating_sub(16);
        let moon_y = bounds.y.saturating_add(1);
        draw_moon(&mut view, moon_x, moon_y, palette, time);

        // A horizon near the floor, with the chapel standing on it at the edge
        // of the frame, where it frames the menu instead of competing with it.
        let horizon = bounds.bottom().saturating_sub(3);
        for x in 0..bounds.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(palette.accent_alt_dim),
            );
        }
        let tower_left = bounds.right().saturating_sub(38);
        art::draw_stencil(
            &mut view,
            tower_left,
            horizon.saturating_sub(art::TOWER.len() as u16),
            &art::TOWER,
            Style::new().with_fg(palette.accent_alt_dim),
        );

        // Runes rising off the ground, so the sky is not the only thing moving.
        if motion.animates() {
            for index in 0..14 {
                let seed = index as u32;
                let start_x =
                    bounds.x + (seed.wrapping_mul(37) % u32::from(bounds.width.max(1))) as u16;
                let rise = (time * 0.28) + (seed % 7) as f32;
                let travel = (rise * 3.0) % f32::from(bounds.height.max(1));
                let y = horizon.saturating_sub(travel as u16);
                if y <= bounds.y {
                    continue;
                }
                view.put_char(
                    start_x,
                    y,
                    ornament::rune(index),
                    Style::new().with_fg(palette.accent_dim),
                );
            }
        }
    }

    /// The name, in glyphs, with a shadow and a rule beneath it.
    fn compose_logo(&self, frame: &mut Frame, area: Rect, context: &FrameContext, motion: Motion) {
        let palette = &context.palette;
        let banner = art::banner_for_width(area.width.saturating_sub(8));
        if matches!(banner, Banner::None) {
            return;
        }
        let progress = self.opening(motion);
        // The name arrives before the menu does.
        let reveal = (progress / 0.6).clamp(0.0, 1.0);
        let row = area.y + area.height / 6;
        let mut view = frame.view(area);
        let width = banner.width();
        let start = area.centered_x(width);
        let lines = banner.lines();
        let columns = (reveal * f32::from(width)).round() as usize;

        // A shadow one row down, so the name sits on the sky rather than in it.
        if reveal > 0.9 {
            art::draw_banner(
                &mut view,
                banner,
                row.saturating_add(1),
                1.0,
                Style::new().with_fg(palette.background),
                Style::new().with_fg(palette.background),
            );
        }
        // The name itself, in a warm ramp: gold at the top, ember at the base.
        for (offset, line) in lines.iter().enumerate() {
            let ink = if offset * 2 < lines.len() {
                palette.focus
            } else {
                palette.effect
            };
            let mut style = Style::new().with_fg(ink).bold();
            if reveal < 1.0 {
                // The leading edge of the reveal is the brightest part.
                let edge = columns.saturating_sub(offset * 3) as u16;
                if edge <= 2 {
                    style = Style::new().with_fg(palette.text).bold();
                }
            }
            let visible: String = line.chars().take(columns).collect();
            view.put_str(start, row.saturating_add(offset as u16), &visible, style);
        }

        if progress > 0.55 {
            let rule_row = row.saturating_add(lines.len() as u16).saturating_add(1);
            ornament::rule(
                &mut view,
                area.centered_x(width),
                rule_row,
                width,
                Style::new().with_fg(palette.accent_dim),
            );
            let tagline = "a dark-fantasy tale, drawn entirely in glyphs";
            view.put_str(
                area.centered_x(glyph::display_width(tagline)),
                rule_row.saturating_add(1),
                tagline,
                Style::new().with_fg(palette.text_muted),
            );
        }
    }

    /// The menu, and what the save holds.
    fn compose_menu(&self, frame: &mut Frame, area: Rect, context: &FrameContext, motion: Motion) {
        let palette = &context.palette;
        let mut view = frame.view(area);
        let progress = self.opening(motion);
        let banner = art::banner_for_width(area.width.saturating_sub(8));
        let logo_rows = if matches!(banner, Banner::None) {
            0
        } else {
            banner.height() + 3
        };
        let top = area
            .y
            .saturating_add(area.height / 6)
            .saturating_add(logo_rows)
            .saturating_add(1);
        let row_width = 44u16.min(area.width);
        let left = area.centered_x(row_width);

        for (index, item) in self.items.iter().enumerate() {
            // Items arrive one after another, so the menu assembles rather than
            // appearing.
            let order = 0.6 + index as f32 * 0.08;
            let shown = if !motion.animates() {
                1.0
            } else {
                ((progress - order) / 0.12).clamp(0.0, 1.0)
            };
            if shown <= 0.01 {
                continue;
            }
            let y = top.saturating_add(index as u16 * 2);
            if y >= area.bottom() {
                break;
            }
            let selected = index == self.selected;
            let available = item.enabled;

            if selected {
                view.fill_style(
                    Rect::new(left.saturating_sub(2), y, row_width, 1),
                    Style::new().with_bg(palette.surface_raised),
                );
                view.put_char(
                    left.saturating_sub(1),
                    y,
                    ornament::SELECTOR,
                    Style::new().with_fg(palette.focus).bold(),
                );
            }
            let (label_style, note_style) = match (selected, available) {
                (true, _) => (
                    Style::new().with_fg(palette.focus).bold(),
                    Style::new().with_fg(palette.text_secondary),
                ),
                (false, true) => (Style::new().with_fg(palette.text), palette.muted_style()),
                (false, false) => (palette.muted_style(), palette.muted_style()),
            };
            view.put_str(left, y, item.label, label_style);
            let note_x = left
                .saturating_add(row_width)
                .saturating_sub(glyph::display_width(item.note) + 2);
            view.put_str(note_x, y, item.note, note_style);
        }

        // The bottom of the screen: what is saved, and how to work the menu.
        let hint_row = area.bottom().saturating_sub(2);
        let summary = match &self.saved {
            Some(data) => format!(
                "saved in {} · {} found · {} places seen",
                data.area.replace('-', " "),
                data.journal.len(),
                data.visited.len()
            ),
            None => "no journey saved yet".to_string(),
        };
        view.put_str(
            area.centered_x(glyph::display_width(&summary)),
            hint_row,
            &summary,
            palette.muted_style(),
        );

        let hint = if self.note.is_empty() {
            "w/s choose · enter select · esc quit".to_string()
        } else {
            self.note.clone()
        };
        view.put_str(
            area.centered_x(glyph::display_width(&hint)),
            hint_row.saturating_add(1),
            &hint,
            if self.note.is_empty() {
                palette.muted_style()
            } else {
                Style::new().with_fg(palette.effect)
            },
        );
    }

    /// A panel over the menu: a confirmation, or the settings.
    fn compose_overlay(
        &self,
        frame: &mut Frame,
        area: Rect,
        context: &FrameContext,
        overlay: Overlay,
    ) {
        let palette = &context.palette;
        let width = area.width.saturating_sub(8).min(62);
        let height = 9u16.min(area.height.saturating_sub(2));
        if width < 30 || height < 6 {
            return;
        }
        let plate = Rect::new(
            area.centered_x(width),
            area.centered_y(height),
            width,
            height,
        );
        let mut view = frame.view(plate);
        view.fill_style(
            view.bounds(),
            Style::new()
                .with_fg(palette.text)
                .with_bg(palette.background),
        );
        let bounds = view.bounds();
        match overlay {
            Overlay::ConfirmNewGame => {
                ornament::plate(
                    &mut view,
                    bounds,
                    Some("begin again"),
                    Style::new().with_fg(palette.warning),
                    Style::new().with_fg(palette.warning).bold(),
                );
                let lines = [
                    "Starting again will replace the journey you have saved:",
                    "where you stand, what you found, and what it opened.",
                ];
                for (offset, line) in lines.iter().enumerate() {
                    view.put_str(
                        bounds.x.saturating_add(3),
                        bounds.y.saturating_add(2 + offset as u16),
                        line,
                        Style::new().with_fg(palette.text),
                    );
                }
                view.put_str(
                    bounds.x.saturating_add(3),
                    bounds.y.saturating_add(5),
                    "[enter] begin again      [esc] keep what I have",
                    Style::new().with_fg(palette.focus).bold(),
                );
            }
            Overlay::Settings => {
                ornament::plate(
                    &mut view,
                    bounds,
                    Some("settings"),
                    palette.border_style(),
                    Style::new().with_fg(palette.accent_alt).bold(),
                );
                let settings = self.settings.get();
                view.put_str(
                    bounds.x.saturating_add(3),
                    bounds.y.saturating_add(2),
                    "motion",
                    palette.muted_style(),
                );
                view.put_str(
                    bounds.x.saturating_add(14),
                    bounds.y.saturating_add(2),
                    settings.motion.label(),
                    Style::new().with_fg(palette.focus).bold(),
                );
                view.put_str(
                    bounds.x.saturating_add(14),
                    bounds.y.saturating_add(3),
                    settings.motion.note(),
                    palette.secondary_style(),
                );
                for (index, motion) in Motion::ALL.iter().enumerate() {
                    let chosen = *motion == settings.motion;
                    view.put_str(
                        bounds.x.saturating_add(3),
                        bounds.y.saturating_add(5 + index as u16),
                        &format!(
                            "{} {}",
                            if chosen {
                                ornament::SELECTOR
                            } else {
                                ornament::DOT
                            },
                            motion.label()
                        ),
                        if chosen {
                            Style::new().with_fg(palette.focus).bold()
                        } else {
                            palette.muted_style()
                        },
                    );
                }
                view.put_str(
                    bounds
                        .right()
                        .saturating_sub(glyph::display_width("[enter] change") + 3),
                    bounds.bottom().saturating_sub(2),
                    "[enter] change",
                    palette.muted_style(),
                );
            }
        }
    }
}

/// Draws a moon: a disc with a crescent of shade, and a halo.
fn draw_moon(view: &mut CanvasView, x: u16, y: u16, palette: &Palette, time: f32) {
    const MOON: [&str; 5] = [
        "   ▄██▄  ",
        "  ██████ ",
        " ██████▓ ",
        "  █████▓ ",
        "   ▀██▀  ",
    ];
    let pulse = if (time * 1.3).sin() > 0.6 {
        Style::new().with_fg(palette.text).bold()
    } else {
        Style::new().with_fg(palette.text_secondary).bold()
    };
    art::draw_stencil(view, x, y, &MOON, pulse);
    // A thin halo, drawn only where nothing else is.
    for (offset, halo) in [(0i32, '·'), (1, '✧')] {
        view.put_char(
            x.saturating_sub(1),
            y.saturating_add(2 + offset as u16),
            halo,
            Style::new().with_fg(palette.accent_alt_dim),
        );
        view.put_char(
            x.saturating_add(9),
            y.saturating_add(2 - offset as u16),
            halo,
            Style::new().with_fg(palette.accent_alt_dim),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use super::{Action, Overlay, TitleScreen};
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement};
    use crate::save::{SaveData, SaveStore};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::screen::{Command, Screen};
    use crate::ui::settings::{Motion, Shared};
    use crate::ui::{FrameContext, Palette};

    fn scratch(tag: &str) -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("grimmoire-title-{tag}-{unique}"))
    }

    fn context(motion: Motion) -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Unsupported,
            art: Art::none(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
            key_releases: true,
            motion,
        }
    }

    fn screen(save: Option<SaveStore>) -> TitleScreen {
        TitleScreen::new(save, None, Shared::new(crate::ui::Settings::new()))
    }

    fn saved_store(tag: &str) -> (SaveStore, PathBuf) {
        let directory = scratch(tag);
        let store = SaveStore::at(directory.join("save.txt"));
        let data = SaveData::new("hollow-village", 62, 30, vec!["ruined-chapel".to_string()])
            .with_progress(
                vec!["village-well".to_string()],
                vec![],
                vec!["cassen-accused".to_string()],
            );
        store.save(&data).expect("save");
        (store, directory)
    }

    fn compose(screen: &TitleScreen, width: u16, height: u16, motion: Motion) -> String {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images: Vec<ImagePlacement> = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context(motion));
        }
        let mut text = String::new();
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                text.push(canvas.get(x, y).map_or(' ', |cell| cell.ch));
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn the_menu_offers_every_entry_and_greys_out_what_cannot_be_taken() {
        let screen = screen(None);
        let text = compose(&screen, 100, 34, Motion::Off);
        for label in ["New Game", "Continue", "Journal", "Settings", "Quit"] {
            assert!(text.contains(label), "{label} is missing:\n{text}");
        }
        assert!(!screen.is_enabled(Action::Continue), "no save, no continue");
        assert!(!screen.is_enabled(Action::Journal));
        assert!(screen.is_enabled(Action::NewGame));
        assert!(screen.is_enabled(Action::Quit));
        assert!(text.contains("no journey yet"), "{text}");
    }

    #[test]
    fn a_save_turns_continue_on_and_names_where_it_left_off() {
        let (store, directory) = saved_store("continue");
        let screen = screen(Some(store));
        assert!(screen.is_enabled(Action::Continue));
        assert!(screen.is_enabled(Action::Journal));
        let text = compose(&screen, 100, 34, Motion::Off);
        assert!(
            text.contains("hollow village"),
            "the save is not summarised:\n{text}"
        );
        assert!(text.contains("1 found"), "{text}");
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn navigation_skips_entries_that_cannot_be_taken() {
        let screen = screen(None);
        // New Game is first. Down must land on Settings, skipping Continue and
        // Journal, because walking onto a dead entry and having it do nothing is
        // worse than not offering it.
        let mut screen = screen;
        assert_eq!(Some(Action::NewGame), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Down));
        assert_eq!(Some(Action::Settings), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Down));
        assert_eq!(Some(Action::Quit), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Down));
        assert_eq!(Some(Action::NewGame), screen.selected_action(), "it wraps");
    }

    #[test]
    fn starting_again_over_a_save_asks_first() {
        let (store, directory) = saved_store("confirm");
        let mut screen = screen(Some(store));
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter)),
            Command::None
        ));
        assert_eq!(Some(Overlay::ConfirmNewGame), screen.overlay());
        assert!(!screen.is_leaving(), "nothing has happened yet");

        let text = compose(&screen, 100, 34, Motion::Off);
        assert!(text.contains("replace the journey"), "{text}");
        assert!(text.contains("keep what I have"), "{text}");

        // And backing out leaves everything alone.
        screen.handle_key(Key::new(KeyCode::Esc));
        assert_eq!(None, screen.overlay());
        assert!(!screen.is_leaving());
        assert!(screen.is_enabled(Action::Continue), "the save survived");
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn confirming_starts_a_new_game() {
        let (store, directory) = saved_store("confirmed");
        let mut screen = screen(Some(store));
        screen.handle_key(Key::new(KeyCode::Enter)); // New Game
        screen.handle_key(Key::new(KeyCode::Enter)); // yes, begin again
        assert!(screen.is_leaving());
        let mut command = Command::None;
        for _ in 0..30 {
            screen.update(Duration::from_millis(60));
            command = screen.after_update();
            if !matches!(command, Command::None) {
                break;
            }
        }
        assert!(matches!(command, Command::Push(_)), "the world should open");
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn continuing_opens_the_world_with_a_click_of_nothing() {
        let (store, directory) = saved_store("fresh");
        let mut screen = screen(Some(store));
        screen.handle_key(Key::new(KeyCode::Down)); // Continue
        assert_eq!(Some(Action::Continue), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Enter));
        assert!(screen.is_leaving());
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn without_a_save_new_game_goes_straight_in() {
        let mut screen = screen(None);
        screen.handle_key(Key::new(KeyCode::Enter));
        assert_eq!(None, screen.overlay(), "nothing to confirm");
        assert!(screen.is_leaving());
    }

    #[test]
    fn the_journal_opens_the_saved_journal() {
        let (store, directory) = saved_store("journal");
        let mut screen = screen(Some(store));
        screen.handle_key(Key::new(KeyCode::Down));
        screen.handle_key(Key::new(KeyCode::Down)); // Journal
        assert_eq!(Some(Action::Journal), screen.selected_action());
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Enter)),
            Command::Push(_)
        ));
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn settings_change_how_the_game_moves() {
        let settings = Shared::new(crate::ui::Settings::new());
        let mut screen = TitleScreen::new(None, None, settings.clone());
        // With nothing saved, Continue and Journal are skipped, so one step down
        // from New Game reaches Settings.
        screen.handle_key(Key::new(KeyCode::Down));
        assert_eq!(Some(Action::Settings), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Enter));
        assert_eq!(Some(Overlay::Settings), screen.overlay());
        assert_eq!(Motion::Full, settings.get().motion);
        screen.handle_key(Key::new(KeyCode::Enter));
        assert_eq!(Motion::Reduced, settings.get().motion);
        let text = compose(&screen, 100, 34, Motion::Reduced);
        assert!(text.contains("reduced"), "{text}");
    }

    #[test]
    fn a_release_or_a_repeat_never_acts() {
        let mut screen = screen(None);
        for key in [
            Key::new(KeyCode::Enter).released(),
            Key::new(KeyCode::Esc).released(),
            Key::new(KeyCode::Down).with_kind(KeyKind::Repeat),
            Key::new(KeyCode::Enter).with_kind(KeyKind::Repeat),
        ] {
            assert!(
                matches!(screen.handle_key(key), Command::None),
                "{key:?} acted"
            );
            assert!(!screen.is_leaving());
        }
    }

    #[test]
    fn escape_quits_and_is_not_a_release() {
        let mut screen = screen(None);
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Esc)),
            Command::Quit
        ));
    }

    #[test]
    fn the_opening_settles_and_then_stops_asking_for_frames() {
        let mut screen = screen(None);
        assert!(screen.is_animating(), "the opening is playing");
        for _ in 0..40 {
            screen.update(Duration::from_millis(100));
        }
        assert!(!screen.is_animating());
        // With motion on it still breathes, slowly.
        assert!(screen.next_frame().is_some());
    }

    #[test]
    fn with_motion_off_the_title_is_still_and_the_menu_is_there() {
        let mut screen = screen(None);
        for _ in 0..40 {
            screen.update(Duration::from_millis(100));
        }
        let settings = Shared::new(crate::ui::Settings {
            motion: Motion::Off,
        });
        let still = TitleScreen::new(None, None, settings);
        assert!(!still.is_animating(), "motion off means motion off");
        assert_eq!(None, still.next_frame(), "and no ambient frames either");
        let text = compose(&still, 100, 34, Motion::Off);
        assert!(
            text.contains("New Game"),
            "the menu is still usable:\n{text}"
        );
        let _ = screen.age;
    }

    #[test]
    fn composition_is_safe_at_any_size() {
        let screen = screen(None);
        for width in [0, 1, 20, 40, 60, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40, 60] {
                let _ = compose(&screen, width, height, Motion::Full);
            }
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let screen = screen(None);
        let mut canvas = Canvas::new(100, 34);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context(Motion::Full));
        }
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                assert_ne!(
                    Some(crate::render::Color::Default),
                    canvas.get(x, y).map(|cell| cell.style.bg),
                    "cell ({x}, {y}) would show the terminal background"
                );
            }
        }
    }

    #[test]
    fn the_title_is_composed_of_one_column_glyphs_only() {
        let screen = screen(None);
        let text = compose(&screen, 100, 34, Motion::Off);
        for ch in text.chars() {
            if ch == '\n' || ch == ' ' {
                continue;
            }
            assert!(
                crate::render::glyph::is_drawable(ch),
                "{ch:?} would desynchronise the frame"
            );
        }
    }
}
