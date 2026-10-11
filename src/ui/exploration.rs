//! The playable exploration screen.
//!
//! This is the game: it owns a [`World`], maps keys onto the world's rules,
//! composes the view, and is the root screen of the application. It draws with
//! the renderer's primitives and holds no rendering logic beyond deciding how
//! terrain and entities look.

use std::time::Duration;

use super::dialogue::DialogueScreen;
use super::help::{Binding, HelpOverlay};
use super::screen::{Command, Screen};
use super::showcase::Showcase;
use super::theme::Palette;
use super::{FrameContext, transition};
use crate::game::{Direction, EntityKind, Interaction, Position, Prompt, Tile, World, areas};
use crate::input::{Key, KeyCode};
use crate::render::{Canvas, Rect, Style, glyph};
use crate::save::SaveStore;

/// The minimum interval between movement steps.
///
/// A uniform step interval is what makes held keys behave predictably: some
/// terminals send auto-repeats as ordinary presses, so gating on the repeat flag
/// alone would let those terminals sprint. Rapid taps are throttled by the same
/// interval, which costs nothing in a grid game.
const MOVE_INTERVAL: Duration = Duration::from_millis(110);

/// How long the curtain takes to reveal a newly entered area.
const TRAVEL_DURATION: Duration = Duration::from_millis(220);

/// The narrowest terminal that can draw the interface.
const MIN_WIDTH: u16 = 24;
/// The shortest terminal that can draw the interface.
const MIN_HEIGHT: u16 = 8;

/// The glyph used for the player.
const PLAYER_GLYPH: char = '@';

/// Bindings listed by the help overlay while exploring.
const CONTROLS: [Binding; 5] = [
    ("w a s d / arrows", "move"),
    ("e", "interact with what you face"),
    ("h / ? / F1", "controls and diagnostics"),
    ("F2", "renderer showcase (development)"),
    ("q / Esc / Ctrl+C", "quit"),
];

/// The exploration screen.
#[derive(Debug)]
pub struct ExplorationScreen {
    world: World,
    save: Option<SaveStore>,
    move_cooldown: Duration,
    travel: Duration,
}

impl ExplorationScreen {
    /// Creates an exploration screen over `world`, optionally saving to `save`.
    #[must_use]
    pub fn new(world: World, save: Option<SaveStore>) -> Self {
        ExplorationScreen {
            world,
            save,
            move_cooldown: Duration::ZERO,
            travel: Duration::ZERO,
        }
    }

    /// Creates the screen with the prototype world, restoring a save if present.
    ///
    /// A save that is missing, unreadable, or inconsistent is ignored and a new
    /// game is started, so a stale file can never block play.
    #[must_use]
    pub fn from_save(save: Option<SaveStore>) -> Self {
        let mut world = World::new(areas::all(), areas::CHAPEL);
        if let Some(store) = &save {
            if let Some(data) = store.load() {
                world.restore(&data);
            }
        }
        ExplorationScreen::new(world, save)
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

    /// Steps the player, respecting the movement interval.
    fn try_move(&mut self, direction: Direction) {
        if self.move_cooldown > Duration::ZERO {
            return;
        }
        if self.world.move_player(direction) {
            self.move_cooldown = MOVE_INTERVAL;
        }
    }

    /// Interacts with whatever is within reach.
    fn interact(&mut self) -> Command {
        match self.world.interact() {
            Interaction::Dialogue(dialogue) => {
                Command::Push(Box::new(DialogueScreen::new(dialogue)))
            }
            Interaction::Travelled(_) => {
                self.travel = TRAVEL_DURATION;
                self.persist();
                Command::None
            }
            Interaction::Nothing | Interaction::Message => Command::None,
        }
    }

    /// Draws the header strip: where we are, and how much has been seen.
    fn compose_header(&self, frame: &mut Canvas, rect: Rect, palette: &Palette) {
        let mut view = frame.view(rect);
        if view.height() == 0 {
            return;
        }
        view.put_str(
            0,
            0,
            self.world.area_name(),
            Style::new().with_fg(palette.arcane).bold(),
        );
        let visited = format!(
            "visited {} of {}",
            self.world.visited_count(),
            self.world.area_count()
        );
        view.put_str(
            view.width().saturating_sub(glyph::display_width(&visited)),
            0,
            &visited,
            palette.muted(),
        );
    }

    /// Draws the world through a camera that follows the player.
    fn compose_world(&self, frame: &mut Canvas, rect: Rect, palette: &Palette) {
        if rect.is_empty() {
            return;
        }
        let mut view = frame.view(rect);
        let area = self.world.area();
        let view_width = i32::from(view.width());
        let view_height = i32::from(view.height());
        let camera_x = axis_camera(area.width(), view.width(), self.world.player().x);
        let camera_y = axis_camera(area.height(), view.height(), self.world.player().y);

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
                view.put_char(
                    screen_x as u16,
                    screen_y as u16,
                    tile_glyph(tile),
                    tile_style(tile, palette),
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
                entity_style(entity.kind, palette),
            );
        }

        let player_x = i32::from(self.world.player().x) - camera_x;
        let player_y = i32::from(self.world.player().y) - camera_y;
        if player_x >= 0 && player_y >= 0 && player_x < view_width && player_y < view_height {
            view.put_char(
                player_x as u16,
                player_y as u16,
                PLAYER_GLYPH,
                player_style(palette),
            );
        }
    }

    /// Draws the feedback line: the last message, or a prompt for what is near.
    fn compose_message(&self, frame: &mut Canvas, rect: Rect, palette: &Palette) {
        let mut view = frame.view(rect);
        if view.height() == 0 {
            return;
        }

        let message = self.world.message();
        if !message.is_empty() {
            view.put_str(0, 0, "> ", palette.muted());
            view.put_str(2, 0, message, Style::new().with_fg(palette.parchment));
            return;
        }

        if let Some(prompt) = self.world.prompt() {
            view.put_str(0, 0, "e  ", Style::new().with_fg(palette.frost).bold());
            view.put_str(3, 0, &prompt_line(prompt), palette.muted());
        }
    }

    /// Draws the status bar: position, facing, and the essential controls.
    fn compose_status(&self, frame: &mut Canvas, rect: Rect, palette: &Palette) {
        let mut view = frame.view(rect);
        if view.height() == 0 {
            return;
        }

        let player = self.world.player();
        view.put_str(0, 0, "position", palette.muted());
        let coords = format!("{}, {}", player.x, player.y);
        view.put_str(
            glyph::display_width("position").saturating_add(2),
            0,
            &coords,
            Style::new().with_fg(palette.frost),
        );

        let facing = format!("facing {}", self.world.facing().name());
        view.put_str(
            view.width().saturating_sub(glyph::display_width(&facing)),
            0,
            &facing,
            palette.muted(),
        );

        if view.height() > 1 {
            view.put_str_centered(
                Rect::new(0, 1, view.width(), 1),
                1,
                "wasd / arrows move    e interact    h help    q quit",
                palette.muted(),
            );
        }
    }
}

impl Screen for ExplorationScreen {
    fn name(&self) -> &'static str {
        "exploration"
    }

    fn handle_key(&mut self, key: Key) -> Command {
        // Discrete actions never fire on auto-repeat: holding `e` must not
        // interact over and over, and holding `h` must not toggle an overlay.
        match key.code {
            KeyCode::Esc => return Command::Quit,
            KeyCode::Char('h') | KeyCode::Char('?') | KeyCode::Function(1) => {
                if key.is_repeat() {
                    return Command::None;
                }
                return Command::Push(Box::new(HelpOverlay::new(&CONTROLS)));
            }
            KeyCode::Function(2) => {
                if key.is_repeat() {
                    return Command::None;
                }
                return Command::Push(Box::new(Showcase::new()));
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if key.is_repeat() {
                    return Command::None;
                }
                return self.interact();
            }
            _ => {}
        }

        if let Some(direction) = movement_direction(key.code) {
            self.try_move(direction);
        }
        Command::None
    }

    fn update(&mut self, delta: Duration) {
        self.move_cooldown = self.move_cooldown.saturating_sub(delta);
        self.travel = self.travel.saturating_sub(delta);
    }

    fn compose(&self, frame: &mut Canvas, context: &FrameContext) {
        let area = frame.bounds();
        let palette = &context.palette;

        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            compose_too_small(frame, palette);
            return;
        }

        let layout = ExplorationLayout::compute(area);
        self.compose_header(frame, layout.header, palette);
        self.compose_world(frame, layout.view, palette);
        self.compose_message(frame, layout.message, palette);
        self.compose_status(frame, layout.status, palette);

        if self.travel > Duration::ZERO {
            let progress = 1.0 - self.travel.as_secs_f32() / TRAVEL_DURATION.as_secs_f32();
            transition::compose_curtain(frame, progress, palette);
        }
    }

    fn is_animating(&self) -> bool {
        self.move_cooldown > Duration::ZERO || self.travel > Duration::ZERO
    }

    fn on_exit(&mut self) {
        self.persist();
    }
}

/// The regions of the exploration interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExplorationLayout {
    /// The area name strip along the top.
    header: Rect,
    /// The play area.
    view: Rect,
    /// The one-line feedback strip.
    message: Rect,
    /// The two-line status and controls bar.
    status: Rect,
}

impl ExplorationLayout {
    /// Computes the regions for `area`.
    fn compute(area: Rect) -> Self {
        let header = Rect::new(area.x, area.y, area.width, 1);
        let status_height = area.height.min(2);
        let status = Rect::new(
            area.x,
            area.bottom().saturating_sub(status_height),
            area.width,
            status_height,
        );
        let message = Rect::new(area.x, status.y.saturating_sub(1), area.width, 1);
        let view_y = header.bottom();
        let view = Rect::new(area.x, view_y, area.width, message.y.saturating_sub(view_y));
        ExplorationLayout {
            header,
            view,
            message,
            status,
        }
    }
}

/// The world-space origin of the visible window for one axis.
///
/// When the area is smaller than the view it is centred (a negative origin);
/// otherwise the camera follows the focus and clamps to the area's edges.
fn axis_camera(area: u16, view: u16, focus: u16) -> i32 {
    if area <= view {
        -((i32::from(view) - i32::from(area)) / 2)
    } else {
        let maximum = i32::from(area - view);
        (i32::from(focus) - i32::from(view) / 2).clamp(0, maximum)
    }
}

/// The glyph drawn for a tile.
fn tile_glyph(tile: Tile) -> char {
    match tile {
        Tile::Floor => '.',
        Tile::Path => ',',
        Tile::Wall => '#',
        Tile::Water => '~',
        Tile::Tree => 'T',
        Tile::Door => '+',
    }
}

/// The style drawn for a tile.
fn tile_style(tile: Tile, palette: &Palette) -> Style {
    match tile {
        Tile::Floor => Style::new().with_fg(palette.shadow),
        Tile::Path => Style::new().with_fg(palette.parchment),
        Tile::Wall => Style::new().with_fg(palette.frost_dim),
        Tile::Water => Style::new().with_fg(palette.frost),
        Tile::Tree => Style::new().with_fg(palette.moss),
        Tile::Door => Style::new().with_fg(palette.ember).bold(),
    }
}

/// The style drawn for an entity, by kind.
fn entity_style(kind: EntityKind, palette: &Palette) -> Style {
    match kind {
        EntityKind::Npc { .. } => Style::new().with_fg(palette.arcane).bold(),
        EntityKind::Object { .. } => Style::new().with_fg(palette.relic).bold(),
    }
}

/// The style drawn for the player. Deliberately the loudest thing on screen.
fn player_style(palette: &Palette) -> Style {
    Style::new()
        .with_fg(palette.highlight_fg)
        .with_bg(palette.parchment)
        .bold()
}

/// Turns a prompt into a short line of text.
fn prompt_line(prompt: Prompt) -> String {
    match prompt {
        Prompt::Speak(name) => format!("speak with {name}"),
        Prompt::Examine(name) => format!("examine {name}"),
        Prompt::Enter(name) => format!("enter {name}"),
    }
}

/// Maps a key onto a movement direction.
fn movement_direction(code: KeyCode) -> Option<Direction> {
    match code {
        KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Up => Some(Direction::North),
        KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Down => Some(Direction::South),
        KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Left => Some(Direction::West),
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Right => Some(Direction::East),
        _ => None,
    }
}

/// The fallback frame for terminals too small for the interface.
fn compose_too_small(frame: &mut Canvas, palette: &Palette) {
    let area = frame.bounds();
    if area.is_empty() {
        return;
    }
    let mut view = frame.view(area);
    let row = area.centered_y(2);
    view.put_str_centered(
        view.bounds(),
        row,
        "Grimmoire",
        Style::new().with_fg(palette.arcane).bold(),
    );
    let hint = format!("needs {MIN_WIDTH}x{MIN_HEIGHT}    q quits");
    view.put_str_centered(
        view.bounds(),
        row.saturating_add(1),
        &hint,
        Style::new().with_fg(palette.frost_dim),
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use super::{ExplorationScreen, MOVE_INTERVAL, axis_camera, movement_direction};
    use crate::game::{Direction, Position, Tile, World, areas};
    use crate::input::{Key, KeyCode};
    use crate::render::Canvas;
    use crate::save::SaveStore;
    use crate::terminal::{Capabilities, PresentStats};
    use crate::ui::screen::{Command, Screen};
    use crate::ui::{FrameContext, Palette};

    fn context() -> FrameContext {
        FrameContext {
            capabilities: Capabilities::conservative(),
            palette: Palette::BASIC,
            stats: PresentStats::default(),
            frame_time: Duration::ZERO,
            animating: false,
        }
    }

    fn screen() -> ExplorationScreen {
        ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), None)
    }

    /// Steps one tile, clearing the movement cooldown first.
    fn step(screen: &mut ExplorationScreen, direction: Direction) {
        screen.update(MOVE_INTERVAL);
        screen.try_move(direction);
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

    fn scratch(tag: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("grimmoire-explore-{tag}-{unique}"))
    }

    #[test]
    fn the_game_starts_in_the_first_area_at_its_spawn() {
        let screen = screen();
        assert_eq!("Ruined Chapel", screen.world().area_name());
        assert_eq!(Position::new(23, 12), screen.world().player());
    }

    #[test]
    fn keys_map_onto_directions_including_wasd_and_arrows() {
        assert_eq!(
            Some(Direction::North),
            movement_direction(KeyCode::Char('w'))
        );
        assert_eq!(
            Some(Direction::South),
            movement_direction(KeyCode::Char('S'))
        );
        assert_eq!(Some(Direction::West), movement_direction(KeyCode::Left));
        assert_eq!(Some(Direction::East), movement_direction(KeyCode::Right));
        assert_eq!(None, movement_direction(KeyCode::Char('z')));
    }

    #[test]
    fn movement_keys_move_the_player() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(23, 11), screen.world().player());
    }

    #[test]
    fn the_step_interval_throttles_repeated_movement() {
        let mut screen = screen();
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(23, 11), screen.world().player());

        // A second step inside the interval is dropped, and the screen asks for
        // more frames while it waits.
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(23, 11), screen.world().player());
        assert!(screen.is_animating());

        screen.update(MOVE_INTERVAL);
        screen.handle_key(Key::char('w'));
        assert_eq!(Position::new(23, 10), screen.world().player());
    }

    #[test]
    fn walking_into_a_wall_leaves_the_player_in_place() {
        let mut screen = screen();
        // Walk north up the path until the wall row stops us.
        for _ in 0..11 {
            step(&mut screen, Direction::North);
        }
        assert_eq!(Position::new(23, 1), screen.world().player());
        step(&mut screen, Direction::North);
        assert_eq!(Position::new(23, 1), screen.world().player());
        assert_eq!(Direction::North, screen.world().facing());
    }

    #[test]
    fn the_player_can_reach_the_door_and_travel() {
        let mut screen = screen();
        while screen.world().player().y < 14 {
            step(&mut screen, Direction::South);
        }
        assert_eq!(Position::new(23, 14), screen.world().player());

        // The door itself blocks a step, which is what turns the player to face it.
        step(&mut screen, Direction::South);
        screen.handle_key(Key::char('e'));

        assert_eq!("Sunken Cellar", screen.world().area_name());
        assert!(screen.is_travelling());
    }

    #[test]
    fn door_tiles_block_walking_but_are_useable() {
        let mut screen = screen();
        while screen.world().player().y < 14 {
            step(&mut screen, Direction::South);
        }
        step(&mut screen, Direction::South); // the door tile itself is not walkable
        assert_eq!(Position::new(23, 14), screen.world().player());
        assert_eq!(
            Tile::Door,
            screen.world().area().tile(Position::new(23, 15))
        );
        assert!(matches!(
            screen.world().prompt(),
            Some(crate::game::Prompt::Enter(_))
        ));
    }

    #[test]
    fn interacting_with_nothing_records_a_message() {
        let mut screen = screen();
        screen.handle_key(Key::char('e'));
        assert!(screen.world().message().contains("nothing"));
        assert_eq!(1, screen.world().visited_count());
        assert_eq!("Ruined Chapel", screen.world().area_name());
    }

    #[test]
    fn facing_the_npc_and_interacting_pushes_a_dialogue() {
        let mut screen = screen();
        // The NPC stands at (21, 8), beside the path.
        for _ in 0..4 {
            step(&mut screen, Direction::North);
        }
        assert_eq!(Position::new(23, 8), screen.world().player());
        step(&mut screen, Direction::West); // (22, 8), facing the NPC
        assert_eq!(Position::new(22, 8), screen.world().player());
        assert!(matches!(
            screen.world().prompt(),
            Some(crate::game::Prompt::Speak(_))
        ));

        let command = screen.handle_key(Key::char('e'));
        assert!(matches!(command, Command::Push(_)));
    }

    #[test]
    fn examining_an_object_sets_a_message_without_pushing_a_screen() {
        let mut screen = screen();
        // The basin sits at (25, 8), on the far side of the path.
        for _ in 0..4 {
            step(&mut screen, Direction::North);
        }
        step(&mut screen, Direction::East); // (24, 8), facing the basin
        assert_eq!(Position::new(24, 8), screen.world().player());

        let command = screen.handle_key(Key::char('e'));
        assert!(matches!(command, Command::None));
        assert!(screen.world().message().contains("rained"));
    }

    #[test]
    fn auto_repeat_does_not_interact_or_open_overlays() {
        let mut screen = screen();
        assert!(matches!(
            screen.handle_key(Key::char('e').with_repeat()),
            Command::None
        ));
        assert!(matches!(
            screen.handle_key(Key::char('h').with_repeat()),
            Command::None
        ));
        assert!(screen.world().message().is_empty());
    }

    #[test]
    fn help_and_escape_are_wired_up() {
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
    fn travel_is_persisted_and_reloaded() {
        let directory = scratch("travel");
        let store = SaveStore::at(directory.join("save.txt"));
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), Some(store.clone()));

        while screen.world().player().y < 14 {
            step(&mut screen, Direction::South);
        }
        screen.handle_key(Key::char('e'));

        assert_eq!("Sunken Cellar", screen.world().area_name());

        let reloaded = ExplorationScreen::from_save(Some(store));
        assert_eq!("Sunken Cellar", reloaded.world().area_name());
        assert_eq!(Position::new(17, 1), reloaded.world().player());
        assert_eq!(2, reloaded.world().visited_count());

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn exiting_persists_progress() {
        let directory = scratch("exit");
        let store = SaveStore::at(directory.join("save.txt"));
        let mut screen =
            ExplorationScreen::new(World::new(areas::all(), areas::CHAPEL), Some(store.clone()));

        step(&mut screen, Direction::North);
        screen.on_exit();

        let data = store.load().expect("saved");
        assert_eq!("ruined-chapel", data.area);
        assert_eq!(11, data.y);

        fs::remove_dir_all(&directory).expect("cleanup");
    }

    #[test]
    fn a_missing_save_starts_a_new_game() {
        let store = SaveStore::at(scratch("missing").join("save.txt"));
        let screen = ExplorationScreen::from_save(Some(store));
        assert_eq!("Ruined Chapel", screen.world().area_name());
    }

    #[test]
    fn compose_draws_the_area_the_player_and_the_status() {
        let mut canvas = Canvas::new(80, 24);
        screen().compose(&mut canvas, &context());
        let text = frame_text(&canvas);
        assert!(text.contains("Ruined Chapel"), "area name missing");
        assert!(text.contains('@'), "player missing");
        assert!(text.contains("position"), "status missing");
        assert!(text.contains("23, 12"), "coordinates missing");
    }

    #[test]
    fn compose_is_safe_across_many_sizes() {
        for width in [1, 10, 24, 46, 80, 200] {
            for height in [1, 4, 8, 16, 40] {
                let mut canvas = Canvas::new(width, height);
                screen().compose(&mut canvas, &context());
            }
        }
    }

    #[test]
    fn compose_is_deterministic() {
        let mut a = Canvas::new(80, 24);
        let mut b = Canvas::new(80, 24);
        screen().compose(&mut a, &context());
        screen().compose(&mut b, &context());
        assert_eq!(frame_text(&a), frame_text(&b));
    }

    #[test]
    fn the_camera_centres_small_areas_and_follows_the_player_in_large_ones() {
        // Smaller area than the view: centred.
        assert_eq!(-4, axis_camera(10, 18, 3));
        // Larger area than the view: follows, clamped.
        assert_eq!(0, axis_camera(100, 20, 2));
        assert_eq!(40, axis_camera(100, 20, 50));
        assert_eq!(80, axis_camera(100, 20, 99));
    }

    #[test]
    fn the_too_small_fallback_still_names_the_game() {
        let mut canvas = Canvas::new(20, 6);
        screen().compose(&mut canvas, &context());
        assert!(frame_text(&canvas).contains("Grimmoire"));
    }
}
