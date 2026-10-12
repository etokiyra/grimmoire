//! The title screen: the first thing the game says.
//!
//! A game should not drop the player into a room as though it were a command that
//! ran. This is the opening: a night sky over a cathedral, the name composed out
//! of glyphs, and a menu that knows whether there is a journey to continue.
//!
//! # How the scene is built
//!
//! From the bottom up, and each layer owns less of the cell than the one below:
//!
//! 1. [`crate::ui::atmosphere`] paints a procedural true-colour sky — the only
//!    layer that writes a cell's *background*, and the reason the sky can hold a
//!    gradient at all. It draws the moon, so there is no moon glyph art here.
//! 2. [`crate::ui::cathedral`] raises a silhouette on the horizon, then lights its
//!    windows.
//! 3. The wizard sprite and the light at its feet, in the doorway.
//! 4. Rising motes, in the sky's own palette.
//! 5. The wordmark, the menu, and whatever the menu has open.
//!
//! Nothing above the sky sets a background, so every glyph keeps the field behind
//! it. That is the whole of the integration, and it is why the menu is drawn as a
//! band along the bottom rather than a panel over the middle: a panel would have
//! to paint its own surface and would shut the sky out of the frame.
//!
//! # How it is animated
//!
//! Everything is a function of one number — how long the screen has been open —
//! so nothing drifts, and nothing needs to be stored. The stages are deliberate
//! and ordered rather than simultaneous: the sky is there, the cathedral rises and
//! wakes, the name is written, and the menu assembles. Then it all settles into a
//! calm idle and never plays again, so returning from Settings or the journal
//! costs nothing.
//!
//! The sky keeps its own, slower cadence: see [`Atmosphere::next_update`]. The
//! title never asks the loop for sixty frames a second once the opening is over,
//! because a still frame should cost nothing.
//!
//! # Destructive actions
//!
//! Starting again when a save exists does not silently replace it. The menu opens
//! a confirmation that says what will be lost, and `esc` backs out of it.

use std::time::Duration;

use super::FrameContext;
use super::atmosphere::{Atmosphere, Rgb, Sky};
use super::cathedral;
use super::journal::JournalScreen;
use super::ornament;
use super::screen::{Command, Message, Screen};
use super::settings::{Motion, Shared};
use super::sprite;
use crate::game::{Discovery, Figure, Progress};
use crate::input::{Key, KeyCode};
use crate::render::{CanvasView, Frame, Rect, Style, glyph};
use crate::save::{SaveData, SaveStore};
use crate::terminal::{Capabilities, ColorSupport};

/// How long the whole opening takes.
const OPENING: Duration = Duration::from_millis(1900);
/// How long the fade into the world takes.
const DEPARTURE: Duration = Duration::from_millis(420);
/// How many drifting motes the title carries.
const MOTES: usize = 26;
/// The name, letter-spaced. One row, because a terminal cell is about twice as
/// tall as it is wide: any multi-row letterform is stretched into a smear by the
/// cell grid, however correct the art is.
const NAME: &str = "GRIMMOIRE";
/// Columns between the letters of the name.
const NAME_GAP: usize = 4;

/// The inks the title's cathedral is painted in: a silhouette against a lit sky,
/// with warm glass.
const STONE: cathedral::Inks = cathedral::Inks {
    stone: Rgb::new(19, 17, 29),
    stone_high: Rgb::new(26, 23, 37),
    edge: Rgb::new(74, 68, 98),
    glass: Rgb::new(255, 186, 92),
    door: Rgb::new(150, 98, 48),
    dark_window: Rgb::new(10, 9, 18),
};

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
    portrait: Option<crate::render::AssetId>,
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
    /// The sky, and its cache.
    atmosphere: Atmosphere,
    /// What the terminal can show. The field is generated in it, so it has to be
    /// known before the first resolve rather than at compose time.
    support: ColorSupport,
    /// The area the field was resolved for; kept so `compose` can check it.
    area: Rect,
}

impl TitleScreen {
    /// Creates the title screen over an optional save.
    #[must_use]
    pub fn new(
        save: Option<SaveStore>,
        portrait: Option<crate::render::AssetId>,
        settings: Shared,
    ) -> Self {
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
        let motion = settings.get().motion;
        let support = ColorSupport::Ansi16;
        let mut atmosphere = Atmosphere::new(title_sky(support), support);
        atmosphere.set_moving(motion.ambience());
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
            atmosphere,
            support,
            area: Rect::new(0, 0, 0, 0),
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

    /// Opens what the player asked for, now that the departure has played.
    ///
    /// **New Game and Continue are different paths.** Continue restores the file;
    /// New Game opens the opening cinematic, which hands back through
    /// `Message::Finished` and is answered with a *fresh* journey. Pushing
    /// `from_save` for both — which is what this did — made New Game silently
    /// continue the old game, and no amount of correct menu wording hides that.
    fn arrive(&mut self) -> Command {
        match self.departure {
            Some(Action::Quit) | None => Command::Quit,
            // The journal and the settings panel never leave this screen.
            Some(Action::Journal) | Some(Action::Settings) => Command::None,
            Some(Action::NewGame) => Command::Push(Box::new(super::opening::OpeningScreen::new())),
            Some(Action::Continue) => Command::Push(Box::new(
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

    /// Where the horizon is, given the frame.
    fn horizon(&self, area: Rect) -> u16 {
        let ground = (area.height / 5).clamp(3, 6);
        area.bottom().saturating_sub(ground)
    }

    /// The rows the wordmark block occupies: the name, its rule, its tagline.
    fn logo_rows(&self, area: Rect) -> (u16, u16) {
        let name_row = area.y.saturating_add((area.height / 12).clamp(1, 3));
        (name_row, name_row.saturating_add(3))
    }
}

/// A smooth 0-to-1 ramp, so a stage starts and ends without a jolt.
fn stage(progress: f32, from: f32, to: f32) -> f32 {
    if to <= from {
        return f32::from(progress >= to);
    }
    let t = ((progress - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The title's own sky: a night with a larger moon and the horizon light swung
/// left of centre, so the composition is not symmetrical about it.
///
/// A terminal that cannot show true colour gets [`Sky::night_basic`] instead,
/// which is a different sky rather than a reduced one.
fn title_sky(support: ColorSupport) -> Sky {
    let mut sky = if support == ColorSupport::Ansi16 {
        Sky::night_basic()
    } else {
        Sky::night()
    };
    sky.moon = Some(super::atmosphere::Moon {
        u: 0.80,
        v: 0.22,
        radius: 0.042,
        // A tight halo on the small palette: a wide one is a grey smear, because
        // the only colour between white and the sky is not a colour at all.
        halo: if support == ColorSupport::Ansi16 {
            0.09
        } else {
            0.22
        },
    });
    // A softer aurora than the default: at the top of a title it competes with
    // the name, and a bright band behind a wordmark costs the wordmark its edge.
    if support != ColorSupport::Ansi16 {
        sky.aurora = Some(Rgb::new(84, 100, 176));
    }
    sky.glow_u = 0.34;
    sky
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
            KeyCode::Up | KeyCode::Char('w') | KeyCode::Left | KeyCode::Char('a') => {
                self.move_selection(-1);
                self.note.clear();
                Command::None
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Right | KeyCode::Char('d') => {
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
        // Reduced motion slows the sky down rather than stopping it; motion off
        // holds it where it stands, without taking the sky away.
        let motion = self.settings.get().motion;
        self.atmosphere.set_moving(motion.ambience());
        self.atmosphere.advance(delta);
        self.atmosphere.resolve(self.area);
    }

    fn after_update(&mut self) -> Command {
        match self.leaving {
            Some(age) if age >= DEPARTURE => self.arrive(),
            _ => Command::None,
        }
    }

    /// Answers the opening cinematic when it is done.
    ///
    /// A dialogue screen would be answered by pushing another dialogue; the
    /// opening is answered by starting the game it was announcing, which only
    /// this screen can do because only this screen holds the store.
    fn receive(&mut self, message: Message) -> Command {
        match message {
            Message::Finished if self.departure == Some(Action::NewGame) => {
                Command::Push(Box::new(super::exploration::ExplorationScreen::fresh(
                    self.save.clone(),
                    self.portrait,
                )))
            }
            // A screen that finished costing nothing to answer.
            Message::Finished | Message::Talk(_) => Command::None,
        }
    }

    fn on_resize(&mut self, area: Rect) {
        self.area = area;
        self.atmosphere.resolve(area);
    }

    fn on_capabilities(&mut self, capabilities: Capabilities) {
        if capabilities.colors == self.support {
            return;
        }
        self.support = capabilities.colors;
        // A field resolved for the wrong terminal is recoloured everywhere, which
        // only matters for a terminal that cannot show true colour at all — but
        // that is exactly the case this exists for.
        self.atmosphere = Atmosphere::new(title_sky(self.support), self.support);
        self.atmosphere
            .set_moving(self.settings.get().motion.ambience());
        self.atmosphere.resolve(self.area);
    }

    fn curtain(&self) -> bool {
        // The title brings the world in itself; a curtain on top would double it.
        false
    }

    fn is_animating(&self) -> bool {
        // The opening is the only thing that needs the animation rate. Once it has
        // settled the sky drifts on its own slow clock, and the loop is free.
        self.leaving.is_some() || !self.settled(self.settings.get().motion)
    }

    fn next_frame(&self) -> Option<Duration> {
        let motion = self.settings.get().motion;
        if self.leaving.is_some() || !self.settled(motion) {
            return Some(Duration::ZERO);
        }
        if !motion.ambience() {
            return None;
        }
        // The sky's cadence and the ambient detail's, whichever comes first.
        let ambient = super::exploration::ambient_period();
        Some(match self.atmosphere.next_update() {
            Some(sky) => sky.min(ambient),
            None => ambient,
        })
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let motion = context.motion;
        let area = frame.bounds();
        if area.is_empty() {
            return;
        }
        // The field owns every cell's background; everything below is drawn on
        // top of it and inherits it.
        if self.atmosphere.area() == area {
            self.atmosphere.paint(frame);
        }
        let progress = self.opening(motion);
        self.compose_scene(frame, area, context, progress);
        self.compose_logo(frame, area, progress);
        self.compose_menu(frame, area, context, progress);
        if let Some(overlay) = self.overlay {
            self.compose_overlay(frame, area, context, overlay);
        }
        if let Some(age) = self.leaving {
            self.compose_departure(frame, area, context, age);
        }
    }
}

impl TitleScreen {
    /// The real sky plus the horizon: the cathedral, the wizard, the light at its
    /// feet and the motes rising off the ground.
    fn compose_scene(&self, frame: &mut Frame, area: Rect, context: &FrameContext, progress: f32) {
        let support = self.support;
        let horizon = self.horizon(area);
        let mut view = frame.view(area);

        // A rule along the horizon, so the ground reads as ground and the building
        // has something to stand on.
        for x in 0..area.width {
            view.put_char(
                x,
                horizon,
                ornament::RULE_LIGHT,
                Style::new().with_fg(Rgb::new(60, 54, 84).to_color(support)),
            );
        }

        // The building rises first and lights second, so the two stages read as
        // one and not as a switch.
        let risen = stage(progress, 0.08, 0.46);
        let lit = stage(progress, 0.26, 0.78);
        let space = horizon
            .saturating_sub(area.y)
            .saturating_sub(self.logo_bottom(area, progress));
        if space >= 10 {
            cathedral::draw(
                &mut view,
                horizon,
                area.width,
                space,
                &cathedral::Look {
                    inks: STONE,
                    support,
                    risen,
                    lit,
                },
            );
        }

        // The wizard, standing in the doorway. It is drawn after the building so
        // its silhouette is not eaten by the doorway's light.
        let hero = sprite::for_figure(Figure::Mage);
        // To the left of the doorway, not in it: a figure standing in the middle of
        // the west front covers the rose window above it, which is the one thing in
        // the building worth seeing.
        let hero_x = area.centered_x(hero.width).saturating_sub(6);
        let hero_y = horizon.saturating_sub(hero.height.saturating_sub(1));
        let breath = (self.age.as_secs_f32() * 0.9).sin() > 0.3;
        let pose = sprite::Pose::Idle {
            breathing: context.motion.ambience() && breath,
        };
        // A pool of light at the feet, so the figure is standing somewhere.
        for dx in 0..8u16 {
            for dy in 0..2u16 {
                let x = hero_x.saturating_sub(1).saturating_add(dx);
                let y = hero_y.saturating_add(hero.height).saturating_add(dy);
                if x < area.width && y < area.height {
                    let glow =
                        Rgb::new(255, 236, 190).mix(Rgb::new(10, 9, 18), 0.35 + dy as f32 * 0.3);
                    view.put_char(x, y, '▁', Style::new().with_fg(glow.to_color(support)));
                }
            }
        }
        sprite::draw(&mut view, hero_x, hero_y, hero, pose, hero.inks, false);

        // Motes rising off the ground, warm against the cold sky.
        if context.motion.ambience() {
            self.compose_motes(&mut view, area, horizon, support);
        }
    }

    /// The lowest row the logo block uses, so the building knows what is above it.
    fn logo_bottom(&self, area: Rect, progress: f32) -> u16 {
        let (_, bottom) = self.logo_rows(area);
        if progress > 0.2 {
            bottom.saturating_sub(area.y)
        } else {
            area.height / 6
        }
    }

    /// Motes drifting up from the ground, in true colour.
    fn compose_motes(
        &self,
        view: &mut CanvasView,
        area: Rect,
        horizon: u16,
        support: ColorSupport,
    ) {
        let time = self.age.as_secs_f32();
        for index in 0..MOTES {
            let seed = index as u32;
            let rand = |n: u32| {
                let value = seed
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(n.wrapping_mul(40_503));
                (value >> 8) as f32 / 16_777_216.0
            };
            let rise = 0.05 + rand(1) * 0.10;
            let life = (time * rise + rand(2)).fract();
            let fade = (life * std::f32::consts::PI).sin().max(0.0);
            if fade <= 0.05 {
                continue;
            }
            let drift = (rand(3) - 0.5) * 0.05;
            let u = (rand(4) + time * drift).rem_euclid(1.0);
            let x = (u * f32::from(area.width)) as u16;
            let travel = (life * f32::from(horizon.saturating_sub(area.y) + 3)) as u16;
            let y = horizon.saturating_sub(travel);
            if x >= area.width || y >= area.height {
                continue;
            }
            let warm = Rgb::new(255, 214, 150).mix(Rgb::new(120, 90, 160), 1.0 - fade);
            let ch = if fade > 0.72 {
                '✧'
            } else if fade > 0.35 {
                '·'
            } else {
                '.'
            };
            view.put_char(x, y, ch, Style::new().with_fg(warm.to_color(support)));
        }
    }

    /// The name, its rule and its tagline.
    fn compose_logo(&self, frame: &mut Frame, area: Rect, progress: f32) {
        let support = self.support;
        let (name_row, _) = self.logo_rows(area);
        let mut view = frame.view(area);

        // The name is written, one column at a time, from the left.
        let letters: Vec<char> = NAME.chars().collect();
        let total = letters.len() * (1 + NAME_GAP);
        let reveal = (stage(progress, 0.32, 0.62) * total as f32).round() as usize;
        let start = area.centered_x(total as u16);
        let last = letters.len().saturating_sub(1).max(1) as f32;
        for (index, letter) in letters.iter().enumerate() {
            let column = start as usize + index * (1 + NAME_GAP);
            if column > start as usize + reveal && reveal < total {
                break;
            }
            // Gold at the head of the word, ember at the tail.
            let t = index as f32 / last;
            let ink = Rgb::new(255, 240, 196).mix(Rgb::new(236, 116, 96), t);
            // The leading edge of the reveal is the brightest part of the word.
            let at_edge = reveal < total && column + 2 >= start as usize + reveal;
            if at_edge {
                view.put_char(
                    column as u16,
                    name_row,
                    *letter,
                    Style::new()
                        .with_fg(Rgb::new(255, 250, 236).to_color(support))
                        .bold(),
                );
            } else {
                view.put_str(
                    column as u16,
                    name_row,
                    &letter.to_string(),
                    Style::new().with_fg(ink.to_color(support)).bold(),
                );
            }
        }

        // A rule under the name, with a diamond in the middle of it.
        if progress > 0.50 {
            let rule_width = area.width.saturating_sub(8).min(46);
            let rule_row = name_row.saturating_add(1);
            ornament::rule(
                &mut view,
                area.centered_x(rule_width),
                rule_row,
                rule_width,
                Style::new().with_fg(Rgb::new(120, 96, 128).to_color(support)),
            );
            let rule_left = area.centered_x(rule_width);
            for x in [
                rule_left,
                rule_left.saturating_add(rule_width / 2),
                rule_left.saturating_add(rule_width.saturating_sub(1)),
            ] {
                view.put_char(
                    x,
                    rule_row,
                    '◆',
                    Style::new().with_fg(Rgb::new(206, 154, 122).to_color(support)),
                );
            }
        }

        // The tagline.
        if progress > 0.62 {
            let tagline = "a dark-fantasy tale, drawn entirely in glyphs";
            view.put_str(
                area.centered_x(glyph::display_width(tagline)),
                name_row.saturating_add(2),
                tagline,
                Style::new().with_fg(Rgb::new(186, 176, 210).to_color(support)),
            );
        }
    }

    /// The menu, along the bottom, on the dark ground where it can be read.
    fn compose_menu(&self, frame: &mut Frame, area: Rect, context: &FrameContext, progress: f32) {
        let palette = &context.palette;
        let mut view = frame.view(area);
        let row = area.bottom().saturating_sub(3);
        if row <= area.y {
            return;
        }

        // The labels, laid end to end and centred.
        let gap = 5u16;
        let mut labels: Vec<(usize, u16)> = Vec::new();
        let mut width = 0u16;
        for (index, item) in self.items.iter().enumerate() {
            let shown = stage(
                progress,
                0.58 + index as f32 * 0.05,
                0.74 + index as f32 * 0.05,
            );
            if shown <= 0.0 {
                continue;
            }
            labels.push((index, width));
            width = width
                .saturating_add(glyph::display_width(item.label))
                .saturating_add(gap);
        }
        if labels.is_empty() {
            // Nothing has arrived yet: no band, or the opening shows an empty
            // dark strip sitting on the ground for no reason.
            return;
        }
        let width = width.saturating_sub(gap);
        let left = area.centered_x(width);

        // A dark band behind the menu. Over the ground it is almost invisible; it
        // is here so the labels are readable whatever the sky behind them is doing.
        view.fill_style(
            Rect::new(left.saturating_sub(4), row, width.saturating_add(8), 1),
            Style::new()
                .with_bg(palette.background)
                .with_fg(palette.background),
        );

        for (index, offset) in labels {
            let item = &self.items[index];
            let x = left.saturating_add(offset);
            let selected = index == self.selected;
            let available = item.enabled;
            if selected {
                view.fill_style(
                    Rect::new(
                        x.saturating_sub(2),
                        row,
                        glyph::display_width(item.label) + 4,
                        1,
                    ),
                    Style::new().with_bg(palette.surface_raised),
                );
                view.put_char(
                    x.saturating_sub(1),
                    row,
                    ornament::SELECTOR,
                    Style::new().with_fg(palette.focus).bold(),
                );
            }
            let style = match (selected, available) {
                (true, _) => Style::new().with_fg(palette.focus).bold(),
                (false, true) => Style::new().with_fg(palette.text),
                (false, false) => palette.muted_style(),
            };
            view.put_str(x, row, item.label, style);
        }

        // The bottom line: what is saved, the note, and the keys. It arrives with
        // the menu, not before it.
        if progress < 0.74 {
            return;
        }
        let hint_row = area.bottom().saturating_sub(1);
        let selected_note = self
            .items
            .get(self.selected)
            .filter(|item| item.enabled)
            .map(|item| item.note)
            .unwrap_or("");
        let hint = if self.note.is_empty() {
            if selected_note.is_empty() {
                "w/s choose · enter select · esc quit".to_string()
            } else {
                format!("{selected_note}  ·  w/s choose · enter select · esc quit")
            }
        } else {
            self.note.clone()
        };
        view.put_str(
            area.centered_x(glyph::display_width(&hint)),
            hint_row,
            &hint,
            if self.note.is_empty() {
                palette.muted_style()
            } else {
                Style::new().with_fg(palette.effect)
            },
        );

        // What is saved, when there is room for it beside the keys rather than
        // through them.
        if progress > 0.8 {
            let summary = match &self.saved {
                Some(data) => format!(
                    "saved in {} · {} found · {} places seen",
                    data.area.replace('-', " "),
                    data.journal.len(),
                    data.visited.len()
                ),
                None => "no journey yet".to_string(),
            };
            // Its own row, directly above the keys. Sharing one row with a
            // centred hint ran the two together as "1 places seew/s choose" as
            // soon as the save's summary was long enough to reach the middle.
            let summary_row = hint_row.saturating_sub(1);
            if summary_row > row {
                view.put_str(
                    area.centered_x(glyph::display_width(&summary)),
                    summary_row,
                    &summary,
                    palette.muted_style(),
                );
            }
        }
    }

    /// The fade into the world: the ground rises up the screen with a lit edge.
    fn compose_departure(
        &self,
        frame: &mut Frame,
        area: Rect,
        context: &FrameContext,
        age: Duration,
    ) {
        let t = (age.as_secs_f32() / DEPARTURE.as_secs_f32()).clamp(0.0, 1.0);
        let covered = (f32::from(area.height) * t).round() as u16;
        if covered == 0 {
            return;
        }
        let support = self.support;
        let mut view = frame.view(area);
        // A thin lit lip, then the dark behind it, so the sweep reads as a wave
        // rather than as a curtain being pulled down.
        for step in 0..covered {
            let y = area.bottom().saturating_sub(1 + step);
            let lip = Rgb::new(255, 196, 120).mix(Rgb::new(12, 10, 20), step as f32 / 3.0);
            view.fill_style(
                Rect::new(area.x, y, area.width, 1),
                Style::new()
                    .with_fg(if step < 2 { lip } else { Rgb::new(12, 10, 20) }.to_color(support))
                    .with_bg(context.palette.background),
            );
        }
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
        let mut screen = TitleScreen::new(save, None, Shared::new(crate::ui::Settings::new()));
        screen.on_capabilities(Capabilities {
            colors: crate::terminal::ColorSupport::TrueColor,
        });
        screen
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

    /// Composes a screen exactly as the application does: the size is announced
    /// first, so the field is resolved for the frame it is about to be drawn in.
    fn compose(screen: &mut TitleScreen, width: u16, height: u16, motion: Motion) -> String {
        screen.on_resize(crate::render::Rect::from_size(width, height));
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
        let mut screen = screen(None);
        let text = compose(&mut screen, 100, 34, Motion::Off);
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
        let mut screen = screen(Some(store));
        assert!(screen.is_enabled(Action::Continue));
        assert!(screen.is_enabled(Action::Journal));
        let text = compose(&mut screen, 120, 34, Motion::Off);
        assert!(
            text.contains("hollow village"),
            "the save is not summarised:\n{text}"
        );
        assert!(text.contains("1 found"), "{text}");
        std::fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn navigation_skips_entries_that_cannot_be_taken() {
        let mut screen = screen(None);
        // New Game is first. Down must land on Settings, skipping Continue and
        // Journal, because walking onto a dead entry and having it do nothing is
        // worse than not offering it.
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

        let text = compose(&mut screen, 100, 34, Motion::Off);
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
        screen.on_capabilities(Capabilities {
            colors: crate::terminal::ColorSupport::TrueColor,
        });
        // With nothing saved, Continue and Journal are skipped, so one step down
        // from New Game reaches Settings.
        screen.handle_key(Key::new(KeyCode::Down));
        assert_eq!(Some(Action::Settings), screen.selected_action());
        screen.handle_key(Key::new(KeyCode::Enter));
        assert_eq!(Some(Overlay::Settings), screen.overlay());
        assert_eq!(Motion::Full, settings.get().motion);
        screen.handle_key(Key::new(KeyCode::Enter));
        assert_eq!(Motion::Reduced, settings.get().motion);
        let text = compose(&mut screen, 100, 34, Motion::Reduced);
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
    fn the_opening_settles_and_then_stops_asking_for_sixty_frames() {
        let mut screen = screen(None);
        assert!(screen.is_animating(), "the opening is playing");
        // The sky keeps its own, slower cadence while the opening plays.
        assert_eq!(Some(Duration::ZERO), screen.next_frame());
        for _ in 0..60 {
            screen.update(Duration::from_millis(100));
        }
        assert!(!screen.is_animating(), "the opening is over");
        // And afterwards it breathes on the sky's clock, not the frame clock.
        let wait = screen.next_frame().expect("the sky still drifts");
        assert!(
            wait > Duration::ZERO,
            "the idle must not ask for every frame"
        );
        assert!(wait <= super::super::exploration::ambient_period());
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
        let mut still = TitleScreen::new(None, None, settings);
        assert!(!still.is_animating(), "motion off means motion off");
        assert_eq!(None, still.next_frame(), "and no ambient frames either");
        let text = compose(&mut still, 100, 34, Motion::Off);
        assert!(
            text.contains("New Game"),
            "the menu is still usable:\n{text}"
        );
        // The sky is drawn once and then held, but it is still *there*: the
        // field is atmosphere, not an effect that motion switches off.
        assert!(!still.atmosphere.is_moving());
        assert!(!still.atmosphere.field().bounds().is_empty());
        let _ = screen.age;
    }

    #[test]
    fn composition_is_safe_at_any_size() {
        let mut screen = screen(None);
        for width in [0, 1, 20, 40, 60, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40, 60] {
                let _ = compose(&mut screen, width, height, Motion::Full);
            }
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let mut screen = screen(None);
        for motion in [Motion::Full, Motion::Off] {
            screen.on_resize(crate::render::Rect::from_size(100, 34));
            let mut canvas = Canvas::new(100, 34);
            canvas.clear_with(Palette::EXTENDED.backdrop());
            let mut images = Vec::new();
            {
                let mut frame = Frame::begin(&mut canvas, &mut images);
                screen.compose(&mut frame, &context(motion));
            }
            for y in 0..canvas.height() {
                for x in 0..canvas.width() {
                    assert_ne!(
                        Some(crate::render::Color::Default),
                        canvas.get(x, y).map(|cell| cell.style.bg),
                        "{motion:?}: cell ({x}, {y}) would show the terminal background"
                    );
                }
            }
        }
    }

    #[test]
    fn the_title_is_composed_of_one_column_glyphs_only() {
        let mut screen = screen(None);
        for motion in [Motion::Full, Motion::Off] {
            let text = compose(&mut screen, 120, 34, motion);
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

    #[test]
    fn the_field_survives_beneath_the_scenery() {
        // The integration's whole claim: a glyph drawn over the sky keeps the sky
        // as its background, and the sky is not the terminal's own colour.
        let mut screen = screen(None);
        screen.on_resize(crate::render::Rect::from_size(120, 34));
        for _ in 0..40 {
            screen.update(Duration::from_millis(100));
        }
        let mut canvas = Canvas::new(120, 34);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context(Motion::Full));
        }
        // Somewhere up in the sky there is a cell whose glyph is the half block the
        // field draws and whose two colours differ.
        let mut two_sample = 0;
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let cell = canvas.get(x, y).expect("cell");
                if cell.ch == crate::ui::atmosphere::HALF_BLOCK {
                    if let (Some(fg), Some(bg)) = (cell.style.fg.as_rgb(), cell.style.bg.as_rgb()) {
                        if fg != bg {
                            two_sample += 1;
                        }
                    }
                }
            }
        }
        assert!(
            two_sample > 20,
            "the field is not under the title: only {two_sample} two-sample cells"
        );
    }

    #[test]
    fn the_cathedral_is_drawn_and_its_windows_light() {
        let mut screen = screen(None);
        // Early in the opening the building has not risen.
        screen.on_resize(crate::render::Rect::from_size(120, 34));
        let early = compose(&mut screen, 120, 34, Motion::Full);
        for _ in 0..40 {
            screen.update(Duration::from_millis(100));
        }
        let settled = compose(&mut screen, 120, 34, Motion::Full);
        assert!(
            settled.matches('█').count() > early.matches('█').count(),
            "the building did not rise: {} then {}",
            early.matches('█').count(),
            settled.matches('█').count()
        );
        assert!(settled.contains('▲'), "the roof finial is missing");
    }
}
