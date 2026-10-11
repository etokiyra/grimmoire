//! The playable exploration screen.
//!
//! This is the game. It owns a [`World`], interprets keys through [`Movement`],
//! composes the frame, and is the root screen of the application.
//!
//! # Composition
//!
//! The world is drawn across the **entire viewport** and the camera scrolls with
//! the player, so the environment dominates the screen. The HUD is an overlay of
//! two rows at the bottom, painted over the world rather than stealing rows from
//! it, plus an arrival plate when you enter an area.
//!
//! Floors are tinted by distance from the player, which reads as a pool of light
//! around them and gives the space depth without any extra geometry.

use std::time::Duration;

use super::art::Art;
use super::dialogue::DialogueScreen;
use super::help::{Binding, HelpOverlay};
use super::motion::{Motion, Movement};
use super::screen::{Command, Screen};
use super::showcase::Showcase;
use super::theme::Palette;
use super::{FrameContext, transition};
use crate::game::{Direction, EntityKind, Interaction, Position, Prompt, Tile, World, areas};
use crate::input::{Key, KeyCode};
use crate::render::{AssetId, Color, Frame, Rect, Style, glyph};
use crate::save::SaveStore;

/// How long the curtain takes to reveal a newly entered area.
const TRAVEL_DURATION: Duration = Duration::from_millis(240);
/// How long the arrival plate lingers once the curtain has cleared.
const PLATE_DURATION: Duration = Duration::from_millis(1500);
/// Rows the HUD occupies at the bottom of the viewport.
const HUD_ROWS: u16 = 2;

/// Bindings listed by the help overlay while exploring.
const CONTROLS: [Binding; 7] = [
    ("w a s d / arrows", "move (hold to keep walking)"),
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
    /// Artwork for the dialogue overlay, resolved once at startup.
    portrait: Option<AssetId>,
    movement: Movement,
    /// Remaining travel-curtain time.
    travel: Duration,
    /// Remaining arrival-plate time.
    plate: Duration,
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
            plate: Duration::ZERO,
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
        // The diffed presenter writes only the cells that actually changed, so a
        // scrolled step costs a few hundred cells rather than a whole frame.
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
                self.plate = PLATE_DURATION;
                self.persist();
                Command::None
            }
            Interaction::Nothing | Interaction::Message => Command::None,
        }
    }

    /// The arrival plate for the current area, if there is one.
    fn plate_for(&self, art: &Art) -> Option<AssetId> {
        let id = self.world.area_id();
        if id == areas::CHAPEL {
            art.chapel
        } else if id == areas::CELLAR {
            art.cellar
        } else {
            None
        }
    }

    /// Draws the world across the whole viewport.
    fn compose_world(&self, frame: &mut Frame, palette: &Palette) {
        let bounds = frame.bounds();
        if bounds.is_empty() {
            return;
        }
        let area = self.world.area();
        let player = self.world.player();
        let mut view = frame.view(bounds);

        let view_width = i32::from(view.width());
        let view_height = i32::from(view.height());
        let camera_x = axis_camera(area.width(), view.width(), player.x);
        let camera_y = axis_camera(area.height(), view.height(), player.y);

        for y in 0..area.height() {
            for x in 0..area.width() {
                let screen_x = i32::from(x) - camera_x;
                let screen_y = i32::from(y) - camera_y;
                if screen_x < 0 || screen_y < 0 || screen_x >= view_width || screen_y >= view_height
                {
                    continue;
                }
                let position = Position::new(x, y);
                let tile = area.tile(position);
                // Floors are tinted by distance from the player: a pool of light
                // that gives the space depth and keeps the surroundings legible.
                let background = if tile.is_walkable() {
                    palette.shade_at(squared_distance(position, player))
                } else {
                    palette.background
                };
                view.put_char(
                    screen_x as u16,
                    screen_y as u16,
                    tile_glyph(tile),
                    Style::new()
                        .with_fg(tile_colour(tile, palette))
                        .with_bg(background),
                );
            }
        }

        for entity in area.entities() {
            let screen_x = i32::from(entity.position.x) - camera_x;
            let screen_y = i32::from(entity.position.y) - camera_y;
            if screen_x < 0 || screen_y < 0 || screen_x >= view_width || screen_y >= view_height {
                continue;
            }
            view.put_char(
                screen_x as u16,
                screen_y as u16,
                entity.glyph,
                Style::new()
                    .with_fg(entity_colour(entity.kind, palette))
                    .bold(),
            );
        }

        let player_x = i32::from(player.x) - camera_x;
        let player_y = i32::from(player.y) - camera_y;
        if player_x >= 0 && player_y >= 0 && player_x < view_width && player_y < view_height {
            view.put_char(
                player_x as u16,
                player_y as u16,
                PLAYER_GLYPH,
                Style::new()
                    .with_fg(palette.player)
                    .with_bg(palette.accent_dim)
                    .bold(),
            );
        }
    }

    /// Draws the arrival plate and its caption over the world.
    fn compose_plate(&self, frame: &mut Frame, palette: &Palette, art: &Art) {
        if self.travel > Duration::ZERO || self.plate == Duration::ZERO {
            return;
        }
        let Some(asset) = self.plate_for(art) else {
            return;
        };
        let bounds = frame.bounds();
        let width = bounds.width.clamp(24, 96);
        let height = bounds.height.saturating_sub(8).clamp(6, 20);
        let plate = Rect::new(
            bounds.centered_x(width),
            bounds.centered_y(height).saturating_sub(1),
            width,
            height,
        );
        frame.place_image(plate, asset);

        // The caption sits below the plate, because the plate's own cells are
        // reserved for the image.
        let caption = Rect::new(bounds.x, plate.bottom().saturating_add(0), bounds.width, 1);
        let mut view = frame.view(caption);
        view.put_str_centered(
            view.bounds(),
            0,
            self.world.area_name(),
            Style::new().with_fg(palette.accent).bold(),
        );
    }

    /// Draws the HUD: a prompt line and a status line, over the world.
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
        view.fill_style(view.bounds(), palette.surface_style());

        // Prompt or last message.
        let (line, style) = if !self.world.message().is_empty() {
            (self.world.message().to_string(), palette.text_style())
        } else if let Some(prompt) = self.world.prompt() {
            (
                prompt_line(prompt),
                Style::new().with_fg(palette.focus).bold(),
            )
        } else {
            ("nothing within reach".to_string(), palette.muted_style())
        };
        view.put_str(1, 0, &line, style);

        // Status.
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

        let keys = "wasd move · e interact · h help · q quit";
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
                    // Resolved without the frame context here; the dialogue
                    // portrait is attached on the next frame's compose path.
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
        // Movement is driven by time, so holding a key walks at our cadence
        // rather than the terminal's auto-repeat rate.
        let motion = self.movement.update(delta);
        self.apply(motion);

        if self.travel > Duration::ZERO {
            self.travel = self.travel.saturating_sub(delta);
        } else {
            self.plate = self.plate.saturating_sub(delta);
        }
    }

    fn compose(&self, frame: &mut Frame, context: &FrameContext) {
        let palette = &context.palette;
        self.compose_world(frame, palette);
        self.compose_plate(frame, palette, &context.art);
        self.compose_hud(frame, palette);

        if self.travel > Duration::ZERO {
            let progress = 1.0 - self.travel.as_secs_f32() / TRAVEL_DURATION.as_secs_f32();
            transition::compose_curtain(frame.canvas(), progress, palette);
        }
    }

    fn is_animating(&self) -> bool {
        // Movement is event-driven, so a held key needs no frames: only the
        // transition and the arrival plate do.
        self.travel > Duration::ZERO || self.plate > Duration::ZERO
    }

    fn on_terminal_ready(&mut self, releases: bool) {
        // Preserve the held direction across the handshake; only the source of
        // release information changes.
        let held = self.movement.held();
        self.movement = Movement::new(releases);
        if let Some(direction) = held {
            self.movement.key(Key::char(match direction {
                Direction::North => 'w',
                Direction::South => 's',
                Direction::West => 'a',
                Direction::East => 'd',
            }));
        }
    }

    fn on_exit(&mut self) {
        self.persist();
    }
}

/// The glyph used for the player.
const PLAYER_GLYPH: char = '@';

/// The world-space origin of the visible window for one axis.
///
/// When the area is larger than the view the camera follows the focus and clamps
/// to the area's edges; an area smaller than the view is centred, which only
/// happens for a pathologically small map.
fn axis_camera(area: u16, view: u16, focus: u16) -> i32 {
    if area <= view {
        -((i32::from(view) - i32::from(area)) / 2)
    } else {
        let maximum = i32::from(area - view);
        (i32::from(focus) - i32::from(view) / 2).clamp(0, maximum)
    }
}

/// The squared distance between two positions, avoiding a square root.
fn squared_distance(a: Position, b: Position) -> u32 {
    let dx = (i32::from(a.x) - i32::from(b.x)).unsigned_abs();
    let dy = (i32::from(a.y) - i32::from(b.y)).unsigned_abs();
    dx * dx + dy * dy
}

/// The glyph drawn for a tile.
fn tile_glyph(tile: Tile) -> char {
    match tile {
        Tile::Floor => '·',
        Tile::Path => '·',
        Tile::Wall => '#',
        Tile::Water => '~',
        Tile::Tree => '♣',
        Tile::Door => '+',
    }
}

/// The foreground colour for a tile.
fn tile_colour(tile: Tile, palette: &Palette) -> Color {
    match tile {
        Tile::Floor => palette.separator,
        Tile::Path => palette.text_muted,
        Tile::Wall => palette.text_secondary,
        Tile::Water => palette.accent_alt,
        Tile::Tree => palette.nature,
        Tile::Door => palette.effect,
    }
}

/// The colour for an entity, by kind.
fn entity_colour(kind: EntityKind, palette: &Palette) -> Color {
    match kind {
        EntityKind::Npc { .. } => palette.character,
        EntityKind::Object { .. } => palette.interactive,
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{ExplorationScreen, axis_camera, squared_distance};
    use crate::game::{Direction, Position, World, areas};
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
            graphics: GraphicsSupport::Forced,
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
        screen
    }

    fn compose_into(
        screen: &ExplorationScreen,
        width: u16,
        height: u16,
        context: &FrameContext,
    ) -> (Canvas, Vec<ImagePlacement>) {
        let mut canvas = Canvas::new(width, height);
        canvas.clear_with(context.palette.backdrop());
        let mut images = Vec::new();
        {
            let mut frame = Frame::begin(&mut canvas, &mut images);
            screen.compose(&mut frame, context);
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

    /// Presses and releases a direction, letting the cadence elapse.
    fn press(screen: &mut ExplorationScreen, key: char) {
        screen.handle_key(Key::char(key));
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

        // A burst of auto-repeats inside the cadence must not sprint.
        for _ in 0..40 {
            screen.handle_key(Key::char('w').with_kind(KeyKind::Repeat));
        }
        assert_eq!(Position::new(70, 43), screen.world().player());

        // Once the cadence has passed, each repeat is one step.
        screen.update(crate::ui::motion::REPEAT_INTERVAL);
        screen.handle_key(Key::char('w').with_kind(KeyKind::Repeat));
        assert_eq!(Position::new(70, 42), screen.world().player());
    }

    #[test]
    fn releasing_stops_the_walk_immediately() {
        let mut screen = screen();
        screen.handle_key(Key::char('d'));
        let after_press = screen.world().player();
        screen.handle_key(Key::char('d').released());

        // Repeats after a release are inert: the key is no longer held.
        for _ in 0..20 {
            screen.handle_key(Key::char('d').with_kind(KeyKind::Repeat));
        }
        screen.update(crate::ui::motion::REPEAT_INTERVAL * 4);
        assert_eq!(after_press, screen.world().player());
        assert!(!screen.movement().is_held());
    }

    #[test]
    fn time_alone_never_moves_the_player() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        let after_press = screen.world().player();
        for _ in 0..50 {
            screen.update(crate::ui::motion::REPEAT_INTERVAL);
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
        // A released `e` must not open a dialogue or change anything.
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
        // Walk west until something stops us; the wall must hold however long
        // the key is held.
        for _ in 0..400 {
            screen.handle_key(Key::char('a'));
            screen.update(crate::ui::motion::REPEAT_INTERVAL);
        }
        let stuck = screen.world().player();
        for _ in 0..50 {
            screen.handle_key(Key::char('a'));
            screen.update(crate::ui::motion::REPEAT_INTERVAL);
        }
        assert_eq!(stuck, screen.world().player(), "walls are authoritative");
        assert!(screen.world().area().tile_walkable(stuck));
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
    fn the_world_fills_the_viewport_at_the_reference_size() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40, &context());
        let text = frame_text(&canvas);

        // The world is drawn into every row above the HUD: the environment owns
        // the viewport, and nothing is left blank for a margin.
        let world_rows = 38;
        for y in 0..world_rows {
            let drawn = (0..120)
                .filter(|x| canvas.get(*x, y).is_some_and(|cell| cell.ch != ' '))
                .count();
            assert_eq!(
                120, drawn,
                "row {y} of the world is not fully drawn ({drawn} cells)"
            );
        }
        // Only two rows are given to the HUD.
        assert!(text.contains("Ruined Chapel"));
        assert!(text.contains("interact"));
    }

    #[test]
    fn the_camera_scrolls_rather_than_centring_a_large_area() {
        // The chapel is larger than the view, so the camera follows the player.
        assert_eq!(0, axis_camera(140, 120, 10));
        assert_eq!(20, axis_camera(140, 120, 80));
        assert_eq!(20, axis_camera(140, 120, 139));
        // A tiny area is still centred rather than pinned to the corner.
        assert_eq!(-4, axis_camera(10, 18, 3));
    }

    #[test]
    fn the_hud_reports_position_and_facing() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40, &context());
        let text = frame_text(&canvas);
        assert!(text.contains("70,44"), "coordinates missing");
        assert!(text.contains("facing"), "facing missing");
        assert!(text.contains("interact"), "controls missing");
    }

    #[test]
    fn the_prompt_appears_when_something_is_in_reach() {
        let mut screen = screen();
        // Stand where the hooded figure is within reach: it is at (56,33).
        screen
            .world_mut()
            .set_player_for_test(Position::new(57, 33));
        let (canvas, _) = compose_into(&screen, 120, 40, &context());
        assert!(frame_text(&canvas).contains("speak with"));
    }

    #[test]
    fn the_arrival_plate_is_placed_only_when_art_exists_and_after_travel() {
        let mut screen = screen();
        let with_art = FrameContext {
            art: Art {
                chapel: Some(crate::render::AssetId::new(0)),
                cellar: Some(crate::render::AssetId::new(1)),
                portrait: Some(crate::render::AssetId::new(2)),
            },
            ..context()
        };

        // Nothing is shown at rest.
        let (_, images) = compose_into(&screen, 120, 40, &with_art);
        assert!(images.is_empty());

        // Mid-transition the curtain covers the screen instead.
        screen.travel = Duration::from_millis(100);
        screen.plate = Duration::from_millis(1000);
        let (_, images) = compose_into(&screen, 120, 40, &with_art);
        assert!(images.is_empty(), "the curtain comes first");

        // After the curtain, the plate appears.
        screen.travel = Duration::ZERO;
        let (_, images) = compose_into(&screen, 120, 40, &with_art);
        assert_eq!(1, images.len(), "expected an arrival plate");

        // Without artwork there is nothing to place.
        let (_, images) = compose_into(&screen, 120, 40, &context());
        assert!(images.is_empty());
    }

    #[test]
    fn composition_is_safe_across_sizes() {
        let mut screen = screen();
        screen.travel = Duration::from_millis(50);
        for width in [0, 1, 20, 40, 80, 120, 240] {
            for height in [0, 1, 6, 12, 24, 40, 60] {
                let mut canvas = Canvas::new(width, height);
                canvas.clear_with(Palette::EXTENDED.backdrop());
                let mut images = Vec::new();
                let mut frame = Frame::begin(&mut canvas, &mut images);
                screen.compose(&mut frame, &context());
            }
        }
    }

    #[test]
    fn every_composed_cell_owns_its_background() {
        let screen = screen();
        let (canvas, _) = compose_into(&screen, 120, 40, &context());
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
    fn composition_is_deterministic() {
        let screen = screen();
        let (first, _) = compose_into(&screen, 120, 40, &context());
        let (second, _) = compose_into(&screen, 120, 40, &context());
        assert_eq!(frame_text(&first), frame_text(&second));
    }

    #[test]
    fn squared_distance_orders_near_before_far() {
        let origin = Position::new(5, 5);
        assert_eq!(0, squared_distance(origin, origin));
        assert_eq!(1, squared_distance(origin, Position::new(6, 5)));
        assert!(
            squared_distance(origin, Position::new(6, 5))
                < squared_distance(origin, Position::new(20, 20))
        );
    }

    #[test]
    fn facing_follows_the_pressed_direction() {
        let mut screen = screen();
        press(&mut screen, 'a');
        assert_eq!(Direction::West, screen.world().facing());
        press(&mut screen, 's');
        assert_eq!(Direction::South, screen.world().facing());
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
}
