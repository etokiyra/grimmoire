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

use std::time::Duration;

use super::dialogue::DialogueScreen;
use super::help::{Binding, HelpOverlay};
use super::motion::{Motion, Movement};
use super::scenery::Scenery;
use super::screen::{Command, Screen};
use super::showcase::Showcase;
use super::theme::Palette;
use super::transition;
use super::{FrameContext, viewport};
use crate::game::{Direction, Interaction, Prompt, World, areas};
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Frame, Rect, Style, glyph};
use crate::save::SaveStore;

/// How long the curtain takes to reveal a newly entered area.
const TRAVEL_DURATION: Duration = Duration::from_millis(260);
/// Rows the interface occupies at the bottom of the viewport.
const HUD_ROWS: u16 = 2;
/// How often the world's ambient detail (water, flames, motes) advances.
///
/// Slow enough that an idle game is nearly free, fast enough that the world does
/// not look painted on.
const AMBIENT_PERIOD: Duration = Duration::from_millis(220);

/// Bindings listed by the help overlay while exploring.
const CONTROLS: [Binding; 8] = [
    ("w a s d / arrows", "move (hold to keep walking)"),
    ("shift", "sprint while moving"),
    ("e", "interact with what you face"),
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
    /// Remaining travel-curtain time.
    travel: Duration,
    /// Time until the ambient detail next advances.
    ambient: Duration,
    /// Seconds of ambient time elapsed, for water and other motion.
    phase: f32,
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
            travel: Duration::ZERO,
            ambient: AMBIENT_PERIOD,
            phase: 0.0,
        }
    }

    /// Creates the screen with the prototype world, restoring a save if present.
    ///
    /// A save that is missing, unreadable, or inconsistent is ignored and a new
    /// game is started, so a stale file can never block play.
    #[must_use]
    pub fn from_save(save: Option<SaveStore>, portrait: Option<AssetId>) -> Self {
        let mut world = World::new(areas::all(), areas::CHAPEL);
        if let Some(store) = &save {
            if let Some(data) = store.load() {
                world.restore(&data);
            }
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
    fn step(&mut self, direction: Direction) {
        self.world.move_player(direction);
    }

    /// Applies a movement decision.
    fn apply(&mut self, motion: Motion) {
        if let Some(direction) = motion.direction() {
            self.step(direction);
        }
    }

    /// Interacts with whatever is within reach.
    fn interact(&mut self) -> Command {
        match self.world.interact() {
            Interaction::Dialogue(dialogue) => {
                Command::Push(Box::new(DialogueScreen::new(dialogue, self.portrait)))
            }
            Interaction::Travelled(_) => {
                self.travel = TRAVEL_DURATION;
                self.persist();
                Command::None
            }
            Interaction::Nothing | Interaction::Message => Command::None,
        }
    }

    /// Draws the world across the whole viewport.
    fn compose_world(&self, frame: &mut Frame, palette: &Palette) {
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        let mut view = frame.view(bounds);
        Scenery {
            world: &self.world,
            palette,
            phase: self.phase,
            moving: self.movement.is_held(),
        }
        .draw(&mut view);
    }

    /// Draws the two-row interface bar over the bottom of the screen.
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

        // Upper row: what just happened, or what is within reach. Raised, so the
        // two rows read as a hierarchy rather than as one block.
        view.fill_style(Rect::new(0, 0, view.width(), 1), palette.raised_style());
        let (line, style) = if !self.world.message().is_empty() {
            (self.world.message().to_string(), palette.text_style())
        } else if let Some(prompt) = self.world.prompt() {
            (
                prompt_line(prompt),
                Style::new().with_fg(palette.focus).bold(),
            )
        } else {
            (String::new(), palette.muted_style())
        };
        view.put_str(1, 0, &line, style);

        if self.movement.is_sprinting() {
            let label = "sprinting »";
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
        self.phase += delta.as_secs_f32();
        self.ambient = self.ambient.saturating_sub(delta);
        if self.ambient == Duration::ZERO {
            self.ambient = AMBIENT_PERIOD;
        }
        if self.travel > Duration::ZERO {
            self.travel = self.travel.saturating_sub(delta);
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        self.compose_world(frame, palette);
        self.compose_hud(frame, palette);

        if self.travel > Duration::ZERO {
            let progress = 1.0 - self.travel.as_secs_f32() / TRAVEL_DURATION.as_secs_f32();
            transition::compose_curtain(frame.canvas(), progress, palette);
        }
    }

    fn is_animating(&self) -> bool {
        // Only a transition needs continuous frames; the world's own motion is
        // slow and is paced by `next_frame`.
        self.travel > Duration::ZERO
    }

    fn next_frame(&self) -> Option<Duration> {
        if self.is_animating() {
            Some(Duration::ZERO)
        } else {
            Some(self.ambient)
        }
    }

    fn on_terminal_ready(&mut self, releases: bool) {
        // Preserve the held direction across the handshake; only the source of
        // release information changes.
        let held = self.movement.held();
        let sprinting = self.movement.is_sprinting();
        let cell_px = self.movement.step_pixels(Direction::North);
        self.movement = Movement::new(releases);
        let _ = cell_px;
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

/// Turns a prompt into a short line of text.
fn prompt_line(prompt: Prompt) -> String {
    match prompt {
        Prompt::Speak(name) => format!("e  speak with {name}"),
        Prompt::Examine(name) => format!("e  examine {name}"),
        Prompt::Enter(name) => format!("e  enter {name}"),
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
    use crate::game::{Position, World, areas};
    use crate::input::{Key, KeyCode, KeyKind};
    use crate::render::{Canvas, Frame, ImagePlacement};
    use crate::terminal::{Capabilities, GraphicsSupport, PresentStats};
    use crate::ui::art::Art;
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
    fn the_game_starts_in_the_first_area_at_its_spawn() {
        let screen = screen();
        assert_eq!("Ruined Chapel", screen.world().area_name());
        assert_eq!(Position::new(70, 44), screen.world().player());
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

        screen.update(screen.movement().interval(crate::game::Direction::North));
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
        use crate::game::Direction;
        let mut walking = screen();
        walking.handle_key(Key::char('w'));
        let mut sprinting = screen();
        sprinting.handle_key(Key::char('w').with_shift());
        assert!(sprinting.is_sprinting());
        assert!(!walking.is_sprinting());
        assert_eq!(43, walking.world().player().y);
        assert_eq!(43, sprinting.world().player().y);

        // One sprint cadence later the sprinter is ready again; the walker is
        // still inside its longer cadence.
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

    #[test]
    fn the_world_fills_the_viewport() {
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
        assert!(text.contains("(o)"), "the player is not drawn as a figure");
        assert!(text.contains('^'), "the player's hood is missing");
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
    fn the_prompt_appears_when_something_is_in_reach() {
        let mut screen = screen();
        screen
            .world_mut()
            .set_player_for_test(Position::new(57, 33));
        let (canvas, _) = compose_into(&screen, 120, 40);
        let text = frame_text(&canvas);
        assert!(text.contains("speak with"), "prompt missing");
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
        assert!(wait > Duration::from_millis(100), "idle must be cheap");
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
        assert_eq!(
            14,
            screen.movement().step_pixels(crate::game::Direction::East)
        );
        assert_eq!(
            28,
            screen.movement().step_pixels(crate::game::Direction::North)
        );
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
