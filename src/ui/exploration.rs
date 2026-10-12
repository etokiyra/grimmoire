//! The playable exploration screen.
//!
//! This is the game. It owns a [`World`], interprets keys through [`Movement`],
//! draws the world with the ASCII scenery renderer, and composes the interface
//! over it. It is the root screen of the application.
//!
//! # Composition
//!
//! The world is drawn across the **entire viewport** and the camera scrolls with
//! the player. The interface is a two-row bar at the bottom, painted over the
//! world rather than stealing rows from it, plus whatever screen is pushed on top
//! (dialogue, help). Screens compose from the bottom up, so an overlay draws over
//! the world without the world having to know it exists.
//!
//! # Frames
//!
//! The screen asks for a frame every [`FRAME_INTERVAL`] while anything is
//! animated - the player moving, a pulse fading, a curtain playing - and for one
//! every [`AMBIENT_PERIOD`] when the world is merely breathing. Nothing else
//! requests frames, so an untouched game costs almost nothing.

use std::time::Duration;

use super::animation::Performance;
use super::atmosphere::{Atmosphere, Sky};
use super::dialogue::DialogueScreen;
use super::effects::{Effects, Pulse};
use super::help::{Binding, HelpOverlay};
use super::journal::JournalScreen;
use super::motion::{Motion, Movement};
use super::ornament;
use super::scenery::{Backdrop, Scenery};
use super::screen::{Command, Message, Screen};
use super::showcase::Showcase;
use super::theme::Palette;
use super::transition;
use super::{FrameContext, viewport};
use crate::game::{Direction, Discovery, Figure, Interaction, Prompt, World, areas};
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Frame, Rect, Style, glyph};
use crate::save::SaveStore;
use crate::terminal::{Capabilities, ColorSupport};

/// How long the curtain takes to reveal a newly entered area.
const TRAVEL_DURATION: Duration = Duration::from_millis(300);
/// Rows the interface occupies at the bottom of the viewport.
const HUD_ROWS: u16 = 2;
/// How often the world's ambient detail advances while the player is still.
///
/// Slow enough that an untouched game costs almost nothing, fast enough that the
/// water, the motes and the candles are visibly alive.
const AMBIENT_PERIOD: Duration = Duration::from_millis(90);
/// How long a discovery announces itself before folding away.
const DISCOVERY_MOMENT: Duration = Duration::from_millis(2600);

/// How often the world's ambient detail advances, for screens that pace
/// themselves the same way.
#[must_use]
pub const fn ambient_period() -> Duration {
    AMBIENT_PERIOD
}

/// Bindings listed by the help overlay while exploring.
const CONTROLS: [Binding; 9] = [
    ("w a s d / arrows", "move (hold to keep walking)"),
    ("shift", "sprint while moving"),
    ("e", "interact with what you face"),
    ("j", "journal of what you have found"),
    ("h / ? / F1", "controls and diagnostics"),
    ("F3", "presentation study (rendering)"),
    ("F2", "renderer showcase (development)"),
    ("Esc", "quit"),
    ("q / Ctrl+C", "quit"),
];

/// The exploration screen.
#[derive(Debug)]
pub struct ExplorationScreen {
    world: World,
    save: Option<SaveStore>,
    /// The dialogue portrait, when optional interface art is in use.
    portrait: Option<AssetId>,
    movement: Movement,
    /// The player's animation state.
    performance: Performance,
    /// Live feedback pulses.
    effects: Effects,
    /// Remaining travel-curtain time.
    travel: Duration,
    /// Countdown to the next ambient frame.
    tick: Duration,
    /// Seconds of world time elapsed, for everything that moves.
    phase: f32,
    /// The discovery being announced, and how long ago it was made.
    discovery: Option<(Discovery, Duration)>,
    /// The air of the place, where the place has one. Resolved for the frame it
    /// will be drawn in, and painted under the world.
    atmosphere: Atmosphere,
    /// What the terminal can show, so the air is generated in the right colours.
    support: ColorSupport,
    /// The viewport the air was resolved for.
    area: Rect,
    /// The place whose air is loaded, when the place has one of its own.
    air_area: Option<crate::game::AreaId>,
}

/// The air of a place, if the place has one.
///
/// Only the ruined chapel so far, and deliberately so: this is the one place the
/// technique has been proved on. Every other place keeps its material backdrop.
fn air_of(area: crate::game::AreaId) -> Option<Sky> {
    if area == areas::CHAPEL {
        Some(Sky::cloister())
    } else {
        None
    }
}

impl ExplorationScreen {
    /// Creates an exploration screen over `world`, optionally saving to `save`.
    #[must_use]
    pub fn new(world: World, save: Option<SaveStore>, portrait: Option<AssetId>) -> Self {
        ExplorationScreen {
            world,
            save,
            portrait,
            movement: Movement::new(false),
            performance: Performance::new(),
            effects: Effects::new(),
            travel: Duration::ZERO,
            tick: AMBIENT_PERIOD,
            phase: 0.0,
            discovery: None,
            atmosphere: Atmosphere::new(Sky::cloister(), ColorSupport::Ansi16),
            support: ColorSupport::Ansi16,
            area: Rect::new(0, 0, 0, 0),
            air_area: None,
        }
    }

    /// Creates the screen with the prototype world, restoring a save if present.
    ///
    /// A save that is missing, unreadable, or inconsistent is ignored and a new
    /// game is started, so a stale file can never block play.
    #[must_use]
    pub fn from_save(save: Option<SaveStore>, portrait: Option<AssetId>) -> Self {
        let mut world = World::new(areas::all(), areas::CHAPEL);
        let restored = save
            .as_ref()
            .and_then(|store| store.load())
            .is_some_and(|data| world.restore(&data));
        if !restored {
            // A first line so the opening frame says where you are and what the
            // place is. Provisional, like everything else here.
            world.set_message("The Ruined Chapel. Cold air, and the smell of old stone.");
        }
        ExplorationScreen::new(world, save, portrait)
    }

    /// The world being explored.
    #[must_use]
    pub const fn world(&self) -> &World {
        &self.world
    }

    /// Whether an area transition is currently playing.
    #[must_use]
    pub fn is_travelling(&self) -> bool {
        self.travel > Duration::ZERO
    }

    /// The movement model, for diagnostics.
    #[must_use]
    pub const fn movement(&self) -> &Movement {
        &self.movement
    }

    /// The animation state, for diagnostics and tests.
    #[must_use]
    pub const fn performance(&self) -> &Performance {
        &self.performance
    }

    /// The live feedback pulses, for diagnostics and tests.
    #[must_use]
    pub const fn effects(&self) -> &Effects {
        &self.effects
    }

    /// Whether the player is sprinting right now.
    #[must_use]
    pub const fn is_sprinting(&self) -> bool {
        self.movement.is_sprinting()
    }

    /// Mutable world access, for tests that need a specific starting position.
    #[cfg(test)]
    pub(crate) fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Writes the current exploration state, if persistence is enabled.
    fn persist(&mut self) {
        let Some(store) = &self.save else {
            return;
        };
        let data = self.world.snapshot();
        if store.save(&data).is_err() {
            self.world
                .set_message("Your progress could not be written to disk.");
        }
    }

    /// Applies a movement decision to the world.
    ///
    /// A refused step is answered with a pulse on whatever refused it, so a wall
    /// looks solid rather than the key looking broken.
    fn step(&mut self, direction: Direction) {
        let before = self.world.player();
        let moved = self.world.move_player(direction);
        if moved {
            self.performance.stepped(self.movement.interval(direction));
            return;
        }
        if self.world.player() != before {
            // Turned without moving: no feedback, the facing was the point.
            return;
        }
        let area = self.world.area();
        if let Some(target) = before.checked_step(direction, area.width(), area.height()) {
            self.effects.spawn(target, Pulse::Blocked);
        }
    }

    /// Applies a movement decision.
    fn apply(&mut self, motion: Motion) {
        if let Some(direction) = motion.direction() {
            self.step(direction);
        }
    }

    /// Interacts with whatever is within reach.
    fn interact(&mut self) -> Command {
        // Point at what is about to answer, before the world changes underneath.
        if let Some(target) = self.world.interaction_target() {
            if let Some(at) = self.world.target_position(target) {
                self.effects.spawn(at, Pulse::Touched);
            }
        }
        match self.world.interact() {
            Interaction::Talk(start) => {
                // The optional image is a portrait of the veiled figure; every
                // other character is drawn from their own ASCII bust.
                let portrait = if start.figure == Figure::Veiled {
                    self.portrait
                } else {
                    None
                };
                Command::Push(Box::new(DialogueScreen::new(start, portrait)))
            }
            Interaction::Discovered(discovery) => {
                self.celebrate(discovery);
                self.persist();
                Command::None
            }
            Interaction::Travelled(_) => {
                self.travel = TRAVEL_DURATION;
                self.effects.spawn(self.world.player(), Pulse::Arrived);
                self.persist();
                Command::None
            }
            Interaction::Nothing | Interaction::Message => Command::None,
        }
    }

    /// Marks a discovery: a flourish on the spot, and a plate that says what was
    /// found. Finding something is the reward the game gives, so it is the one
    /// thing that interrupts.
    fn celebrate(&mut self, discovery: Discovery) {
        self.effects.spawn(self.world.player(), Pulse::Arrived);
        self.discovery = Some((discovery, Duration::ZERO));
    }

    /// Whether the place the player is standing in has an air of its own.
    fn aired(&self) -> bool {
        self.air_area.is_some()
    }

    /// Draws the world across the whole viewport.
    fn compose_world(&self, frame: &mut Frame, palette: &Palette) {
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        // The air goes down first and owns every cell's background; the tiles and
        // the figures then draw their glyphs over it. Nothing above sets a
        // background, which is the whole of the seam between the two.
        if self.aired() && self.atmosphere.area() == bounds {
            self.atmosphere.paint(frame);
        }
        let player = self.performance.pose(
            self.world.facing(),
            self.movement.is_held(),
            self.movement.is_sprinting(),
        );
        let mut view = frame.view(bounds);
        Scenery {
            world: &self.world,
            palette,
            phase: self.phase,
            player,
            step: self.performance.steps(),
            effects: &self.effects,
            backdrop: if self.aired() {
                Backdrop::Atmosphere
            } else {
                Backdrop::Material
            },
        }
        .draw(&mut view);
    }

    /// Draws the two-row interface bar over the bottom of the screen.
    ///
    /// The rows are a hierarchy rather than a block: what just happened sits on a
    /// raised surface, where you are sits on the plain one, and the controls stay
    /// out of the way on the right.
    fn compose_hud(&self, frame: &mut Frame, palette: &Palette) {
        let bounds = frame.bounds();
        if bounds.height < HUD_ROWS {
            return;
        }
        let hud = Rect::new(
            bounds.x,
            bounds.bottom().saturating_sub(HUD_ROWS),
            bounds.width,
            HUD_ROWS,
        );
        let mut view = frame.view(hud);

        // Upper row: what just happened, or what is within reach.
        view.fill_style(Rect::new(0, 0, view.width(), 1), palette.raised_style());
        let (mark, line, style) = if !self.world.message().is_empty() {
            ('·', self.world.message().to_string(), palette.text_style())
        } else if let Some(prompt) = self.world.prompt() {
            (
                '♥',
                prompt_line(prompt),
                Style::new().with_fg(palette.focus).bold(),
            )
        } else {
            (' ', String::new(), palette.muted_style())
        };
        view.put_char(1, 0, mark, Style::new().with_fg(palette.accent));
        view.put_str(3, 0, &line, style);

        // Upper row, right: the movement state, so sprinting is unmistakable.
        if self.movement.is_sprinting() {
            let label = "★ sprint »";
            view.put_str(
                view.width()
                    .saturating_sub(glyph::display_width(label))
                    .saturating_sub(1),
                0,
                label,
                Style::new().with_fg(palette.effect).bold(),
            );
        }

        // Lower row: where you are, and the controls.
        view.fill_style(Rect::new(0, 1, view.width(), 1), palette.surface_style());
        let player = self.world.player();
        let mut column = view.put_str(
            1,
            1,
            self.world.area_name(),
            Style::new().with_fg(palette.accent).bold(),
        );
        column = column.saturating_add(view.put_str(column + 1, 1, "·", palette.muted_style()));
        let where_ = format!(
            "{},{} · facing {}",
            player.x,
            player.y,
            self.world.facing().name()
        );
        view.put_str(column + 1, 1, &where_, palette.muted_style());

        let keys = "wasd move · shift sprint · e interact · h help";
        view.put_str(
            view.width()
                .saturating_sub(glyph::display_width(keys))
                .saturating_sub(1),
            1,
            keys,
            palette.muted_style(),
        );
    }
}

impl Screen for ExplorationScreen {
    fn name(&self) -> &'static str {
        "exploration"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Discrete actions fire once per press: releases are not activations and
        // auto-repeat must not interact over and over.
        if key.is_down() && !key.is_repeat() {
            match key.code {
                KeyCode::Esc => return Command::Quit,
                KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Function(1) => {
                    return Command::Push(Box::new(HelpOverlay::new(&CONTROLS)));
                }
                KeyCode::Function(3) => {
                    return Command::Push(Box::new(super::stage::StageScreen::new()));
                }
                KeyCode::Function(2) => return Command::Push(Box::new(Showcase::new())),
                KeyCode::Char('e') | KeyCode::Char('E') => {
                    return self.interact();
                }
                KeyCode::Char('j') | KeyCode::Char('J') => {
                    return Command::Push(Box::new(JournalScreen::new(
                        self.world.journal().to_vec(),
                        crate::game::Progress::total(),
                    )));
                }
                _ => {}
            }
        }

        let motion = self.movement.key(key);
        self.apply(motion);
        Command::None
    }

    fn update(&mut self, delta: Duration) {
        // Movement is event-driven; time only advances the world's own motion.
        self.movement.update(delta);
        self.performance.advance(delta);
        self.effects.advance(delta);
        self.phase += delta.as_secs_f32();
        self.tick = self.tick.saturating_sub(delta);
        if self.tick.is_zero() {
            self.tick = AMBIENT_PERIOD;
        }
        if self.travel > Duration::ZERO {
            self.travel = self.travel.saturating_sub(delta);
        }
        if let Some((discovery, age)) = self.discovery {
            let age = age.saturating_add(delta);
            self.discovery = (age < DISCOVERY_MOMENT).then_some((discovery, age));
        }

        // Load the air of wherever the player now is, and let it drift. A place
        // without one is never resolved at all, so it costs nothing.
        let here = self.world.area_id();
        if self.air_area != Some(here) {
            self.air_area = None;
            // A place's air is a *gradient*, and a gradient needs somewhere to
            // live. Sixteen colours cannot hold one: every dark stop snaps to the
            // same black and the field becomes two flat tones with an edge, which
            // looks worse than the material backdrop it replaced. So the small
            // palette keeps the stone it had.
            if self.support.has_extended_palette() && air_of(here).is_some() {
                self.air_area = Some(here);
                self.atmosphere = Atmosphere::new(Sky::cloister(), self.support);
                self.atmosphere.resolve(self.area);
            }
        }
        if self.aired() {
            self.atmosphere.advance(delta);
            self.atmosphere.resolve(self.area);
        }
    }

    fn on_capabilities(&mut self, capabilities: Capabilities) {
        if capabilities.colors == self.support {
            return;
        }
        self.support = capabilities.colors;
        if self.aired() {
            self.atmosphere = Atmosphere::new(Sky::cloister(), self.support);
            self.atmosphere.resolve(self.area);
        }
    }

    fn on_resize(&mut self, area: Rect) {
        self.area = area;
        if self.aired() {
            self.atmosphere.resolve(area);
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        self.compose_world(frame, palette);
        self.compose_hud(frame, palette);

        self.compose_discovery(frame, palette);

        if self.travel > Duration::ZERO {
            let progress = 1.0 - self.travel.as_secs_f32() / TRAVEL_DURATION.as_secs_f32();
            transition::compose_curtain(frame.canvas(), progress, palette);
        }
    }

    fn is_animating(&self) -> bool {
        // Continuous frames only while something is actually moving: a step, a
        // fading pulse, an announcement, or an area transition. A merely-idle
        // world is paced by `next_frame` instead.
        self.travel > Duration::ZERO
            || self.discovery.is_some()
            || self.effects.is_active()
            || self.performance.is_animated(self.movement.is_held())
    }

    fn next_frame(&self) -> Option<Duration> {
        if self.is_animating() {
            return Some(Duration::ZERO);
        }
        // The place's air has its own, slower clock; whichever comes first wins.
        Some(match self.atmosphere.next_update() {
            Some(air) if self.aired() => air.min(self.tick),
            _ => self.tick,
        })
    }

    fn receive(&mut self, message: Message) -> Command {
        match message {
            // A conversation walked its own graph; the world applies what it did.
            // This is the only way a conversation can change anything, and it is
            // why the effects are reported rather than written directly.
            Message::Talk(effects) => {
                let learned = self.world.apply(&effects);
                if let Some(discovery) = learned.first() {
                    self.celebrate(*discovery);
                }
                self.persist();
                Command::None
            }
        }
    }

    fn on_terminal_ready(&mut self, releases: bool) {
        // Preserve the held direction across the handshake; only the source of
        // release information changes.
        let held = self.movement.held();
        let sprinting = self.movement.is_sprinting();
        self.movement = Movement::new(releases);
        if let Some(direction) = held {
            let key = match direction {
                Direction::North => 'w',
                Direction::South => 's',
                Direction::West => 'a',
                Direction::East => 'd',
            };
            let key = if sprinting {
                Key::char(key).with_shift()
            } else {
                Key::char(key)
            };
            self.movement.key(key);
        }
    }

    fn on_cell_metrics(&mut self, cell_px: Option<(u16, u16)>) {
        if let Some(cell_px) = cell_px {
            self.movement.set_cell_px(cell_px);
        }
    }

    fn on_exit(&mut self) {
        self.persist();
    }
}

impl ExplorationScreen {
    /// Announces a discovery over the top of the world.
    ///
    /// It rises, holds, and folds away, and it never covers the player: the world
    /// stays the thing being looked at, and the plate is a note laid over it.
    fn compose_discovery(&self, frame: &mut Frame, palette: &Palette) {
        let Some((discovery, age)) = self.discovery else {
            return;
        };
        let bounds = frame.bounds();
        if bounds.width < 40 || bounds.height < 12 {
            return;
        }
        let total = DISCOVERY_MOMENT.as_secs_f32();
        let t = (age.as_secs_f32() / total).clamp(0.0, 1.0);
        // In quickly, hold, out slowly.
        let presence = if t < 0.12 {
            t / 0.12
        } else if t > 0.82 {
            (1.0 - t) / 0.18
        } else {
            1.0
        };
        if presence <= 0.02 {
            return;
        }
        let width = (bounds.width.saturating_sub(8)).min(64);
        let height = 5u16;
        let top = bounds.y.saturating_add(2);
        let plate = Rect::new(bounds.centered_x(width), top, width, height);
        let mut view = frame.view(plate);
        let plate_bounds = view.bounds();
        let ink = if presence > 0.6 {
            palette.focus
        } else {
            palette.effect_dim
        };
        view.fill_style(
            plate_bounds,
            Style::new()
                .with_fg(palette.text)
                .with_bg(palette.surface_raised),
        );
        ornament::plate(
            &mut view,
            plate_bounds,
            Some("you noticed something"),
            Style::new().with_fg(ink),
            Style::new().with_fg(ink).bold(),
        );
        view.put_str(
            plate_bounds.x.saturating_add(2),
            plate_bounds.y.saturating_add(1),
            discovery.title(),
            Style::new().with_fg(palette.text).bold(),
        );
        view.put_str(
            plate_bounds
                .right()
                .saturating_sub(glyph::display_width("[j] journal") + 2),
            plate_bounds.y.saturating_add(1),
            "[j] journal",
            palette.muted_style(),
        );
        for (offset, line) in glyph::wrap(discovery.journal(), plate_bounds.width.saturating_sub(4))
            .into_iter()
            .take(2)
            .enumerate()
        {
            view.put_str(
                plate_bounds.x.saturating_add(2),
                plate_bounds.y.saturating_add(2 + offset as u16),
                &line,
                palette.secondary_style(),
            );
        }
    }
}

/// Turns a prompt into a short line of text.
fn prompt_line(prompt: Prompt) -> String {
    match prompt {
        Prompt::Speak(name) => format!("e  speak with {name}"),
        Prompt::Examine(name) => format!("e  examine {name}"),
        Prompt::Enter(name) => format!("e  enter {name}"),
        // A sealed way is announced rather than hidden: the player should know it
        // is there and that something is missing, not discover it by being
        // refused.
        Prompt::Sealed(name) => format!("e  {name} — sealed"),
    }
}

/// Whether the viewport is too small to play in.
#[must_use]
pub fn usable(bounds: Rect) -> bool {
    viewport::is_usable(bounds)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::ExplorationScreen;
    use crate::game::{Direction, Position, World, areas};
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement, Rect};
    use crate::terminal::{Capabilities, ColorSupport, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
    use crate::ui::effects::Pulse;
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::EXTENDED,
            graphics: GraphicsSupport::Unsupported,
            art: Art::none(),
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
            key_releases: true,
            motion: crate::ui::settings::Motion::Full,
        }
    }

    fn screen() -> ExplorationScreen {
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
        screen.on_terminal_ready(true);
        screen.on_cell_metrics(Some((9, 20)));
        screen
    }

    fn compose_into(
        screen: &ExplorationScreen,
        width: u16,
        height: u16,
    ) -> (Canvas, Vec<ImagePlacement>) {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        (canvas, images)
    }

    fn frame_text(canvas: &Canvas) -> String {
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
    fn a_conversation_effect_reaches_the_world_and_is_persisted() {
        use crate::game::{Effect, Flag};
        use crate::ui::screen::Message;

        let mut screen = screen();
        // A conversation hands its effects back through the stack.
        assert!(matches!(
            screen.receive(Message::Talk(vec![
                Effect::Set(Flag::WrenToldOfRin),
                Effect::Learn(crate::game::Discovery::WoodsGlade),
            ])),
            Command::None
        ));
        assert!(screen.world().progress().has(Flag::WrenToldOfRin));
        assert!(screen.world().knows(crate::game::Discovery::WoodsGlade));
        assert!(
            screen.discovery.is_some(),
            "a discovery should announce itself"
        );
        assert!(screen.is_animating(), "and it needs frames to do it");
    }

    #[test]
    fn an_announcement_settles_and_stops_asking_for_frames() {
        use crate::game::{Effect, Flag};
        use crate::ui::screen::Message;

        let mut screen = screen();
        screen.receive(Message::Talk(vec![Effect::Set(Flag::CassenAccused)]));
        assert!(screen.discovery.is_none(), "a flag is not a discovery");
        screen.receive(Message::Talk(vec![Effect::Learn(
            crate::game::Discovery::VillageWell,
        )]));
        assert!(screen.discovery.is_some());
        for _ in 0..40 {
            screen.update(Duration::from_millis(120));
        }
        assert!(screen.discovery.is_none(), "it folds away");
    }

    #[test]
    fn the_chapel_is_lit_by_its_own_air() {
        // The integration's claim: the field goes down first and the tiles draw
        // over it, so a tile's background is the place's air and not the theme's
        // flat dark.
        let mut screen = screen();
        screen.on_capabilities(Capabilities {
            colors: ColorSupport::TrueColor,
        });
        let area = Rect::from_size(120, 40);
        screen.on_resize(area);
        screen.update(Duration::from_millis(20));
        assert!(screen.aired(), "the chapel has no air loaded");
        assert_eq!(
            area,
            screen.atmosphere.area(),
            "the air was not resolved for the frame"
        );

        let mut canvas = Canvas::new(120, 40);
        canvas.clear_with(Palette::EXTENDED.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, &context());
        }
        let truecolour = (0..40)
            .flat_map(|y| (0..120).map(move |x| (x, y)))
            .filter(|(x, y)| {
                canvas
                    .get(*x, *y)
                    .is_some_and(|cell| matches!(cell.style.bg, crate::render::Color::Rgb(..)))
            })
            .count();
        assert!(
            truecolour > 500,
            "only {truecolour} cells carry the air; the tiles overwrote it"
        );
    }

    #[test]
    fn the_game_starts_in_the_first_area_at_its_spawn() {
        let screen = screen();
        assert_eq!("Ruined Chapel", screen.world().area_name());
        assert_eq!(Position::new(70, 44), screen.world().player());
    }

    #[test]
    fn a_fresh_game_opens_with_a_line_and_an_old_save_does_not() {
        let fresh = ExplorationScreen::from_save(None, None);
        assert!(
            !fresh.world().message().is_empty(),
            "the opening frame should say where you are"
        );
    }

    #[test]
    fn a_fresh_press_moves_at_once() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(70, 43), screen.world().player());
    }

    #[test]
    fn holding_a_direction_walks_on_our_cadence_not_the_terminals() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(70, 43), screen.world().player());

        for _ in 0..40 {
            screen.handle_key(Key::char('w').with_kind(KeyKind::Repeat));
        }
        assert_eq!(Position::new(70, 43), screen.world().player());

        screen.update(screen.movement().interval(Direction::North));
        screen.handle_key(Key::char('w').with_kind(KeyKind::Repeat));
        assert_eq!(Position::new(70, 42), screen.world().player());
    }

    #[test]
    fn releasing_stops_the_walk_immediately() {
        let mut screen = screen();
        screen.handle_key(Key::char('d'));
        let after_press = screen.world().player();
        screen.handle_key(Key::char('d').released());

        for _ in 0..20 {
            screen.handle_key(Key::char('d').with_kind(KeyKind::Repeat));
        }
        screen.update(Duration::from_millis(600));
        assert_eq!(after_press, screen.world().player());
        assert!(!screen.movement().is_held());
    }

    #[test]
    fn time_alone_never_moves_the_player() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        let after_press = screen.world().player();
        for _ in 0..50 {
            screen.update(Duration::from_millis(200));
        }
        assert_eq!(
            after_press,
            screen.world().player(),
            "movement must be event-driven"
        );
    }

    #[test]
    fn changing_direction_is_immediate() {
        let mut screen = screen();
        screen.handle_key(Key::char('d'));
        let start = screen.world().player();
        screen.handle_key(Key::char('a'));
        assert_eq!(Position::new(start.x - 1, start.y), screen.world().player());
    }

    #[test]
    fn a_release_is_not_an_activation_for_discrete_actions() {
        let mut screen = screen();
        assert!(matches!(
            screen.handle_key(Key::char('e').released()),
            Command::None
        ));
        assert!(screen.world().message().is_empty());
    }

    #[test]
    fn auto_repeat_does_not_trigger_discrete_actions() {
        let mut screen = screen();
        assert!(matches!(
            screen.handle_key(Key::char('h').with_kind(KeyKind::Repeat)),
            Command::None
        ));
        assert!(matches!(
            screen.handle_key(Key::char('e').with_kind(KeyKind::Repeat)),
            Command::None
        ));
    }

    #[test]
    fn collision_still_wins_over_repeated_movement() {
        let mut screen = screen();
        for _ in 0..400 {
            screen.handle_key(Key::char('a'));
            screen.update(Duration::from_millis(60));
        }
        let stuck = screen.world().player();
        for _ in 0..50 {
            screen.handle_key(Key::char('a'));
            screen.update(Duration::from_millis(60));
        }
        assert_eq!(stuck, screen.world().player(), "walls are authoritative");
        assert!(screen.world().area().tile_walkable(stuck));
    }

    #[test]
    fn sprinting_still_collides() {
        let mut screen = screen();
        for _ in 0..400 {
            screen.handle_key(Key::char('a').with_shift());
            screen.update(Duration::from_millis(30));
        }
        let stuck = screen.world().player();
        for _ in 0..80 {
            screen.handle_key(Key::char('a').with_shift());
            screen.update(Duration::from_millis(30));
        }
        assert_eq!(stuck, screen.world().player(), "a sprint is not a noclip");
        assert!(screen.world().area().tile_walkable(stuck));
    }

    #[test]
    fn moving_with_shift_covers_more_ground_in_the_same_time() {
        let mut walking = screen();
        walking.handle_key(Key::char('w'));
        let mut sprinting = screen();
        sprinting.handle_key(Key::char('w').with_shift());
        assert!(sprinting.is_sprinting());
        assert!(!walking.is_sprinting());
        assert_eq!(43, walking.world().player().y);
        assert_eq!(43, sprinting.world().player().y);

        let sprint_gap = sprinting.movement().interval(Direction::North);
        walking.update(sprint_gap);
        sprinting.update(sprint_gap);
        walking.handle_key(Key::char('w').with_kind(KeyKind::Repeat));
        sprinting.handle_key(Key::char('w').with_shift().with_kind(KeyKind::Repeat));

        assert_eq!(43, walking.world().player().y, "the walk cadence holds");
        assert_eq!(42, sprinting.world().player().y, "the sprint keeps pace");
    }

    #[test]
    fn help_is_pushed_and_escape_quits() {
        let mut screen = screen();
        assert!(matches!(
            screen.handle_key(Key::char('h')),
            Command::Push(_)
        ));
        assert!(matches!(
            screen.handle_key(Key::new(KeyCode::Esc)),
            Command::Quit
        ));
    }

    // --- feedback -----------------------------------------------------------

    #[test]
    fn a_refused_step_pulses_whatever_refused_it() {
        let mut screen = screen();
        // Walk into the south wall until the step is refused.
        for _ in 0..40 {
            screen.handle_key(Key::char('s'));
            screen.update(screen.movement().interval(Direction::South));
        }
        let blocked = screen.world().player();
        let ahead = blocked
            .checked_step(Direction::South, 140, 52)
            .expect("a tile to walk into");
        screen.handle_key(Key::char('s'));
        assert_eq!(
            Some(Pulse::Blocked),
            screen.effects().sample(ahead).map(|(kind, _)| kind),
            "a blocked step should be visible, not just silent"
        );
    }

    #[test]
    fn a_pulse_keeps_the_frame_loop_alive_then_lets_it_settle() {
        let mut screen = screen();
        for _ in 0..40 {
            screen.handle_key(Key::char('s'));
            screen.update(screen.movement().interval(Direction::South));
        }
        screen.handle_key(Key::char('s'));
        assert!(screen.is_animating(), "a fade needs frames");
        screen.update(Duration::from_millis(400));
        assert!(!screen.is_animating(), "the fade has finished");
        assert!(!screen.effects().is_active());
    }

    #[test]
    fn interacting_pulses_the_thing_that_answered() {
        let mut screen = screen();
        screen
            .world_mut()
            .set_player_for_test(Position::new(57, 33));
        let figure = Position::new(56, 33);
        screen.handle_key(Key::char('e'));
        assert_eq!(
            Some(Pulse::Touched),
            screen.effects().sample(figure).map(|(kind, _)| kind)
        );
    }

    #[test]
    fn travelling_pulses_the_arrival() {
        let mut screen = screen();
        screen
            .world_mut()
            .set_player_for_test(Position::new(70, 50));
        screen.handle_key(Key::char('e'));
        assert!(screen.is_travelling());
        assert_eq!(
            Some(Pulse::Arrived),
            screen
                .effects()
                .sample(screen.world().player())
                .map(|(kind, _)| kind)
        );
        assert!(screen.is_animating(), "the curtain needs frames");
    }

    // --- animation ----------------------------------------------------------

    #[test]
    fn the_stride_advances_once_per_step_and_stops_with_movement() {
        let mut screen = screen();
        let start = screen.performance().frame();
        screen.handle_key(Key::char('d'));
        assert_ne!(start, screen.performance().frame(), "the stride advanced");

        // Standing still for a long time must not advance it further.
        let after = screen.performance().frame();
        for _ in 0..20 {
            screen.update(Duration::from_millis(100));
        }
        assert_eq!(after, screen.performance().frame());
    }

    #[test]
    fn a_step_keeps_asking_for_frames_until_it_has_played_out() {
        let mut screen = screen();
        screen.handle_key(Key::char('d'));
        assert!(screen.is_animating(), "the stride is playing");
        screen.update(Duration::from_millis(500));
        assert!(!screen.is_animating(), "and then it settles");
    }

    #[test]
    fn sprinting_reaches_the_renderer_as_a_different_stage() {
        use crate::ui::animation::Stage;
        let mut screen = screen();
        screen.handle_key(Key::char('d').with_shift());
        let pose = screen.performance().pose(
            screen.world().facing(),
            screen.movement().is_held(),
            screen.movement().is_sprinting(),
        );
        assert_eq!(Stage::Sprint, pose.stage);
    }

    // --- composition --------------------------------------------------------

    #[test]
    fn the_world_fills_the_viewport_and_is_ascii() {
        let screen = screen();
        let (canvas, images) = compose_into(&screen, 120, 40);
        assert!(
            images.is_empty(),
            "the world is ASCII: no images are placed"
        );
        for y in 0..38 {
            let drawn = (0..120)
                .filter(|x| canvas.get(*x, y).is_some_and(|cell| cell.ch != ' '))
                .count();
            assert_eq!(120, drawn, "row {y} of the world is not fully drawn");
        }
        let text = frame_text(&canvas);
        assert!(text.contains("Ruined Chapel"));
        assert!(text.contains("interact"));
    }

    #[test]
    fn the_world_has_a_visible_character() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40);
        let text = frame_text(&canvas);
        // The wizard's hat and staff: the sprite is drawn from the library, so
        // these are the supplied glyphs, not a stand-in.
        assert!(text.contains("▟▖"), "the wizard's hat is missing:\n{text}");
        assert!(text.contains("▓▓▓"), "the wizard's robe is missing");
    }

    #[test]
    fn the_hud_reports_position_facing_and_the_controls() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40);
        let text = frame_text(&canvas);
        assert!(text.contains("70,44"), "coordinates missing");
        assert!(text.contains("facing"), "facing missing");
        assert!(
            text.contains("sprint"),
            "the sprint control is undiscoverable"
        );
    }

    #[test]
    fn the_interface_marks_are_single_column_glyphs() {
        for ch in ['♥', '★', '·', '»', '✿', '❀', '✧'] {
            assert!(
                crate::render::glyph::is_drawable(ch),
                "{ch:?} is not one column wide"
            );
        }
    }

    #[test]
    fn the_prompt_appears_when_something_is_in_reach() {
        let mut screen = screen();
        screen
            .world_mut()
            .set_player_for_test(Position::new(57, 33));
        let (canvas, _) = compose_into(&screen, 120, 40);
        assert!(frame_text(&canvas).contains("speak with"), "prompt missing");
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40);
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
    fn composition_is_safe_across_sizes() {
        let mut screen = screen();
        screen.travel = Duration::from_millis(50);
        for width in [0, 1, 20, 40, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40, 60] {
                let _ = compose_into(&screen, width, height);
            }
        }
    }

    #[test]
    fn composition_is_deterministic() {
        let screen = screen();
        let (first, _) = compose_into(&screen, 120, 40);
        let (second, _) = compose_into(&screen, 120, 40);
        assert_eq!(frame_text(&first), frame_text(&second));
    }

    #[test]
    fn an_idle_game_asks_for_slow_frames_only() {
        let screen = screen();
        assert!(!screen.is_animating(), "idle must not be a 60fps loop");
        let wait = screen.next_frame().expect("ambient frames");
        assert!(wait > Duration::from_millis(50), "idle must be cheap");
        assert!(wait <= Duration::from_millis(200), "but still alive");
    }

    #[test]
    fn a_transition_asks_for_frames_immediately() {
        let mut screen = screen();
        screen.travel = Duration::from_millis(100);
        assert!(screen.is_animating());
        assert_eq!(Some(Duration::ZERO), screen.next_frame());
    }

    #[test]
    fn the_handshake_switches_to_release_reporting() {
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
        assert!(!screen.movement().reports_releases());
        screen.on_terminal_ready(true);
        assert!(screen.movement().reports_releases());
        screen.on_terminal_ready(false);
        assert!(!screen.movement().reports_releases());
    }

    #[test]
    fn the_terminal_cell_size_reaches_the_movement_model() {
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
        screen.on_terminal_ready(true);
        screen.on_cell_metrics(Some((14, 28)));
        assert_eq!(14, screen.movement().step_pixels(Direction::East));
        assert_eq!(28, screen.movement().step_pixels(Direction::North));
    }

    #[test]
    fn the_handshake_keeps_a_held_sprint() {
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None, None);
        screen.on_terminal_ready(true);
        screen.handle_key(Key::char('d').with_shift());
        assert!(screen.is_sprinting());
        screen.on_terminal_ready(true);
        assert!(screen.is_sprinting(), "the hold survived the handshake");
    }
}
