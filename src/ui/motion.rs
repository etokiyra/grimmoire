//! Movement input: press, hold, release, and sprint.
//!
//! Movement is a separate, explicit model rather than a throttle on the raw key
//! stream, because the things a player notices are distinct:
//!
//! * a **fresh press** must step immediately - no gate, no latency;
//! * **holding** must walk at a steady, predictable cadence rather than at
//!   whatever rate the terminal happens to auto-repeat at;
//! * **releasing** must stop promptly;
//! * **sprinting** must be noticeably faster without being a different kind of
//!   movement.
//!
//! Steps are taken **on events, never on a timer**. A press moves at once, an
//! auto-repeat moves again if the cadence allows, and that is all: a tap is
//! exactly one step, and no sequence of lost or missing events can leave the
//! player walking on their own. Kitty reports key releases, so a release stops
//! the walk at once; a stall guard also clears the held state when events stop
//! arriving.
//!
//! # Why the two axes do *not* share a cadence
//!
//! A step moves the view exactly one cell, and terminal cells are about twice as
//! tall as they are wide - roughly nine by twenty pixels here. At one cadence a
//! step north would cross more than twice the screen distance of a step east, so
//! holding a direction would look fast vertically and sluggish horizontally.
//!
//! The fix is to move at a constant **screen speed** rather than a constant step
//! rate: north and south step on the cell *height*, east and west on the cell
//! *width*, both derived from one speed in pixels per second
//! ([`WALK_SPEED`]). The two axes then cross the screen at the same rate, which
//! is what "feels consistent" actually means. The cell size comes from the
//! terminal, so this adapts instead of being tuned for one font.
//!
//! Discrete actions are deliberately *not* handled here - they fire once per
//! press and never on auto-repeat, which callers enforce by checking
//! [`Key::is_down`](crate::input::Key::is_down) and
//! [`Key::is_repeat`](crate::input::Key::is_repeat).

use std::time::Duration;

use crate::game::Direction;
use crate::input::{Key, KeyCode};

/// How fast the view moves while walking, in screen pixels per second.
///
/// The one number to tune when a walk feels wrong. Both axes are derived from
/// it, so they always agree on screen however the terminal is configured.
pub const WALK_SPEED: f32 = 150.0;

/// How fast the view moves while sprinting, in screen pixels per second.
///
/// Twice the walking speed: unmistakably faster, still controlled.
pub const SPRINT_SPEED: f32 = 300.0;

/// The fastest a step may repeat, however fast the screen speed implies.
///
/// A narrow terminal can give cells only a few pixels wide, which would otherwise
/// ask for more steps a second than a keyboard can deliver.
pub const MIN_INTERVAL: Duration = Duration::from_millis(45);

/// The cell size assumed when the terminal will not report one.
pub const DEFAULT_CELL_PX: (u16, u16) = (10, 20);

/// How long the held state survives with no further events.
///
/// Only affects the reported state, never whether the player moves: a genuine
/// hold keeps arriving as auto-repeats, which are far more frequent than this.
pub const STALL_TIMEOUT: Duration = Duration::from_millis(400);

/// What the movement model wants the game to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    /// Take one step in this direction.
    Step(Direction),
    /// Nothing this time.
    Nothing,
}

impl Motion {
    /// The direction, if this is a step.
    #[must_use]
    pub const fn direction(self) -> Option<Direction> {
        match self {
            Motion::Step(direction) => Some(direction),
            Motion::Nothing => None,
        }
    }
}

/// The movement state machine.
#[derive(Debug, Clone)]
pub struct Movement {
    held: Option<Direction>,
    sprinting: bool,
    /// Time until the next automatic step.
    timer: Duration,
    /// Time since the last movement event, for the stall guard.
    silence: Duration,
    /// Whether the terminal reports key releases. Diagnostics only: the stall
    /// guard applies either way.
    releases: bool,
    /// One cell in device pixels, which sets the per-axis cadence.
    cell_px: (u16, u16),
}

impl Movement {
    /// Creates the state machine.
    ///
    /// `releases` should be true when the terminal reports key releases (Kitty
    /// does once the keyboard protocol is enabled).
    #[must_use]
    pub const fn new(releases: bool) -> Self {
        Movement {
            held: None,
            sprinting: false,
            timer: Duration::ZERO,
            silence: Duration::ZERO,
            releases,
            cell_px: DEFAULT_CELL_PX,
        }
    }

    /// Whether releases are being reported.
    #[must_use]
    pub const fn reports_releases(&self) -> bool {
        self.releases
    }

    /// The direction currently held, if any.
    #[must_use]
    pub const fn held(&self) -> Option<Direction> {
        self.held
    }

    /// Whether the player is mid-move, which is when the loop must keep ticking.
    #[must_use]
    pub const fn is_held(&self) -> bool {
        self.held.is_some()
    }

    /// Whether the current hold is a sprint.
    #[must_use]
    pub const fn is_sprinting(&self) -> bool {
        self.sprinting
    }

    /// Tells the model how big a cell is, so it can pace the two axes so they
    /// cross the screen at the same rate.
    pub fn set_cell_px(&mut self, cell_px: (u16, u16)) {
        let cell_px = if cell_px.0 == 0 || cell_px.1 == 0 {
            DEFAULT_CELL_PX
        } else {
            cell_px
        };
        self.cell_px = cell_px;
        self.timer = self
            .timer
            .min(self.interval(self.held.unwrap_or(Direction::North)));
    }

    /// The device pixels a step covers in `direction`.
    #[must_use]
    pub const fn step_pixels(&self, direction: Direction) -> u16 {
        match direction {
            Direction::North | Direction::South => self.cell_px.1,
            Direction::East | Direction::West => self.cell_px.0,
        }
    }

    /// How long a step in `direction` takes at the current pace.
    ///
    /// Derived from the screen speed and the cell size, so a step east and a step
    /// north cover the same number of *pixels* in the same second.
    #[must_use]
    pub fn interval(&self, direction: Direction) -> Duration {
        let speed = if self.sprinting {
            SPRINT_SPEED
        } else {
            WALK_SPEED
        };
        let seconds = f32::from(self.step_pixels(direction)) / speed;
        Duration::from_secs_f32(seconds).max(MIN_INTERVAL)
    }

    /// The cadence currently in force for the held direction, for diagnostics.
    #[must_use]
    pub fn cadence(&self) -> Duration {
        self.interval(self.held.unwrap_or(Direction::North))
    }

    /// How long until the next automatic step, for diagnostics and tests.
    #[must_use]
    pub const fn pending(&self) -> Duration {
        self.timer
    }

    /// The direction a key stands for, if it is a movement key.
    #[must_use]
    pub fn direction_for(code: KeyCode) -> Option<Direction> {
        match code {
            KeyCode::Char('w') | KeyCode::Char('W') | KeyCode::Up => Some(Direction::North),
            KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Down => Some(Direction::South),
            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Left => Some(Direction::West),
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Right => Some(Direction::East),
            _ => None,
        }
    }

    /// Whether this event asks for a sprint.
    ///
    /// Shift is the natural modifier, and Kitty's keyboard protocol reports it on
    /// the movement key itself. An upper-case movement character also counts, so
    /// the control still works on a terminal that reports `W` without a flag.
    #[must_use]
    pub fn wants_sprint(key: &Key) -> bool {
        key.modifiers.shift || matches!(key.code, KeyCode::Char(ch) if ch.is_ascii_uppercase())
    }

    /// Feeds one key event.
    pub fn key(&mut self, key: Key) -> Motion {
        let Some(direction) = Self::direction_for(key.code) else {
            return Motion::Nothing;
        };

        if key.is_release() {
            if self.held == Some(direction) {
                self.held = None;
                self.sprinting = false;
                self.timer = Duration::ZERO;
            }
            self.silence = Duration::ZERO;
            return Motion::Nothing;
        }
        // A repeat that does not continue an existing hold is not a new press:
        // only a press starts moving. This is what makes a release final, even if
        // stray repeats arrive afterwards.
        if key.is_repeat() && self.held != Some(direction) {
            return Motion::Nothing;
        }

        // Any press or repeat proves the key is still down.
        self.silence = Duration::ZERO;
        let sprint = Self::wants_sprint(&key);

        if self.held == Some(direction) {
            // The same direction again: either the terminal auto-repeated, or
            // this is a second physical press. Both take the cadence, which
            // keeps the pace ours rather than the terminal's.
            self.set_sprint(sprint);
            return self.step_if_due(direction);
        }

        // A new direction moves at once, so turning and starting feel immediate.
        self.held = Some(direction);
        self.sprinting = sprint;
        self.silence = Duration::ZERO;
        self.timer = self.interval(direction);
        Motion::Step(direction)
    }

    /// Advances time. This never moves the player: motion is driven by events.
    pub fn update(&mut self, delta: Duration) -> Motion {
        self.timer = self.timer.saturating_sub(delta);
        self.silence = self.silence.saturating_add(delta);
        if self.held.is_some() && self.silence >= STALL_TIMEOUT {
            self.held = None;
            self.sprinting = false;
            self.silence = Duration::ZERO;
        }
        Motion::Nothing
    }

    /// Forgets any held direction, without stepping.
    ///
    /// Used when a blocking overlay opens, so a key held across the transition
    /// cannot keep walking once it closes.
    pub fn release_all(&mut self) {
        self.held = None;
        self.sprinting = false;
        self.timer = Duration::ZERO;
        self.silence = Duration::ZERO;
    }

    /// Changes the sprint state, tightening the pending step if there is one.
    ///
    /// Pressing Shift mid-stride must speed the walk up rather than wait out the
    /// slower cadence that was already running.
    fn set_sprint(&mut self, sprint: bool) {
        if self.sprinting == sprint {
            return;
        }
        self.sprinting = sprint;
        self.timer = self.timer.min(self.cadence());
    }

    /// Takes a step on the cadence if enough time has passed.
    fn step_if_due(&mut self, direction: Direction) -> Motion {
        self.silence = Duration::ZERO;
        if self.timer == Duration::ZERO {
            self.timer = self.interval(direction);
            Motion::Step(direction)
        } else {
            Motion::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_CELL_PX, MIN_INTERVAL, Motion, Movement, SPRINT_SPEED, STALL_TIMEOUT, WALK_SPEED,
    };
    use crate::game::Direction;
    use crate::input::{Key, KeyCode};
    use std::time::Duration;

    /// The reference cell: a narrow terminal font, as measured here.
    const CELL: (u16, u16) = (9, 20);

    fn with_releases() -> Movement {
        let mut movement = Movement::new(true);
        movement.set_cell_px(CELL);
        movement
    }

    fn sprint_key(ch: char) -> Key {
        Key::char(ch).with_shift()
    }

    fn east(interval: Duration) -> Duration {
        interval * 6
    }

    #[test]
    fn a_fresh_press_steps_immediately() {
        let mut movement = with_releases();
        assert_eq!(Motion::Step(Direction::North), movement.key(Key::char('w')));
        assert_eq!(Some(Direction::North), movement.held());
    }

    #[test]
    fn movement_keys_cover_wasd_and_arrows() {
        for (code, expected) in [
            (KeyCode::Char('w'), Direction::North),
            (KeyCode::Char('S'), Direction::South),
            (KeyCode::Left, Direction::West),
            (KeyCode::Right, Direction::East),
        ] {
            assert_eq!(Some(expected), Movement::direction_for(code), "{code:?}");
        }
        assert_eq!(None, Movement::direction_for(KeyCode::Char('e')));
        assert_eq!(None, Movement::direction_for(KeyCode::Enter));
    }

    #[test]
    fn a_tap_is_exactly_one_step() {
        // Even with no release and no repeats - which is what injected or pasted
        // input looks like - one press is one step, and time alone never moves.
        for releases in [true, false] {
            let mut movement = Movement::new(releases);
            movement.set_cell_px(CELL);
            assert_eq!(Motion::Step(Direction::North), movement.key(Key::char('w')));
            for _ in 0..40 {
                assert_eq!(
                    Motion::Nothing,
                    movement.update(Duration::from_millis(200)),
                    "releases={releases}: time must not move the player"
                );
            }
        }
    }

    #[test]
    fn auto_repeats_walk_at_our_cadence_not_the_terminals() {
        let mut movement = with_releases();
        let cadence = movement.interval(Direction::East);
        movement.key(Key::char('d'));

        // A burst of repeats inside the interval is absorbed.
        let mut steps = 0;
        for _ in 0..20 {
            if movement
                .key(Key::char('d').with_repeat())
                .direction()
                .is_some()
            {
                steps += 1;
            }
        }
        assert_eq!(0, steps, "repeats inside the cadence must not move");

        // Once the interval passes, the next repeat steps.
        movement.update(cadence);
        assert_eq!(
            Motion::Step(Direction::East),
            movement.key(Key::char('d').with_repeat())
        );
    }

    #[test]
    fn a_steady_repeat_stream_walks_steadily() {
        let mut movement = with_releases();
        let cadence = movement.interval(Direction::East);
        movement.key(Key::char('d'));
        let mut steps = 0;
        for _ in 0..40 {
            movement.update(cadence);
            if movement
                .key(Key::char('d').with_repeat())
                .direction()
                .is_some()
            {
                steps += 1;
            }
        }
        assert_eq!(40, steps, "one step per cadence interval");
        assert_eq!(Some(Direction::East), movement.held());
    }

    #[test]
    fn releasing_stops_the_walk() {
        let mut movement = with_releases();
        movement.key(Key::char('d'));
        movement.update(east(movement.interval(Direction::East)));
        assert!(movement.is_held());

        assert_eq!(Motion::Nothing, movement.key(Key::char('d').released()));
        assert_eq!(None, movement.held());
        assert_eq!(Motion::Nothing, movement.update(Duration::from_millis(500)));
    }

    #[test]
    fn changing_direction_is_immediate_and_switches_the_cadence() {
        let mut movement = with_releases();
        movement.key(Key::char('d'));
        assert_eq!(Motion::Step(Direction::West), movement.key(Key::char('a')));
        assert_eq!(Some(Direction::West), movement.held());
        assert_eq!(Motion::Nothing, movement.key(Key::char('a').with_repeat()));
        movement.update(movement.interval(Direction::West));
        assert_eq!(
            Motion::Step(Direction::West),
            movement.key(Key::char('a').with_repeat())
        );
    }

    #[test]
    fn a_second_press_of_the_held_direction_respects_the_cadence() {
        let mut movement = with_releases();
        assert_eq!(Motion::Step(Direction::North), movement.key(Key::char('w')));
        assert_eq!(Motion::Nothing, movement.key(Key::char('w')));
        assert_eq!(Motion::Nothing, movement.key(Key::char('w').with_repeat()));
        movement.update(movement.interval(Direction::North));
        assert_eq!(
            Motion::Step(Direction::North),
            movement.key(Key::char('w').with_repeat())
        );
    }

    #[test]
    fn a_missing_release_eventually_clears_the_held_state() {
        let mut movement = Movement::new(false);
        movement.key(Key::char('d'));
        assert!(movement.is_held());
        movement.update(STALL_TIMEOUT + Duration::from_millis(1));
        assert_eq!(None, movement.held(), "a lost release must not linger");
    }

    #[test]
    fn a_release_of_some_other_key_is_ignored() {
        let mut movement = with_releases();
        movement.key(Key::char('w'));
        movement.key(Key::char('s').released());
        assert_eq!(Some(Direction::North), movement.held());
    }

    #[test]
    fn non_movement_keys_do_not_disturb_movement() {
        let mut movement = with_releases();
        movement.key(Key::char('w'));
        assert_eq!(Motion::Nothing, movement.key(Key::char('e')));
        assert_eq!(Motion::Nothing, movement.key(Key::new(KeyCode::Esc)));
        assert_eq!(Some(Direction::North), movement.held());
    }

    #[test]
    fn release_all_forgets_everything() {
        let mut movement = with_releases();
        movement.key(Key::char('w'));
        movement.release_all();
        assert_eq!(None, movement.held());
        assert!(!movement.is_sprinting());
        assert_eq!(Motion::Nothing, movement.update(Duration::from_millis(500)));
    }

    // --- the two axes -------------------------------------------------------

    /// The screen distance a direction covers per second at the current pace.
    fn screen_speed(movement: &Movement, direction: Direction) -> f32 {
        f32::from(movement.step_pixels(direction)) / movement.interval(direction).as_secs_f32()
    }

    #[test]
    fn every_direction_moves_at_the_same_screen_speed() {
        // This is the whole point of the per-axis cadence: a step east and a step
        // north cross the same number of pixels in the same time, so one axis
        // does not feel faster than the other.
        let movement = with_releases();
        for direction in Direction::ALL {
            assert!(
                (screen_speed(&movement, direction) - WALK_SPEED).abs() < 1.0,
                "{direction:?} moves at {} px/s, not {WALK_SPEED}",
                screen_speed(&movement, direction)
            );
        }
    }

    #[test]
    fn horizontal_steps_repeat_more_often_than_vertical_ones() {
        // Because cells are taller than they are wide.
        let movement = with_releases();
        assert!(
            CELL.0 < CELL.1,
            "the reference cell is taller than it is wide"
        );
        assert!(
            movement.interval(Direction::East) < movement.interval(Direction::North),
            "a narrow step must repeat sooner to cover the same ground"
        );
    }

    #[test]
    fn the_cadence_follows_the_terminal_rather_than_a_constant() {
        let mut movement = Movement::new(true);
        movement.set_cell_px((18, 20));
        let wide = movement.interval(Direction::East);
        movement.set_cell_px((9, 20));
        let narrow = movement.interval(Direction::East);
        assert!(narrow < wide, "narrower cells should step sooner");
    }

    #[test]
    fn a_zero_cell_size_falls_back_rather_than_dividing_by_zero() {
        let mut movement = Movement::new(true);
        movement.set_cell_px((0, 0));
        assert_eq!(DEFAULT_CELL_PX.0, movement.step_pixels(Direction::East));
        assert_eq!(DEFAULT_CELL_PX.1, movement.step_pixels(Direction::North));
    }

    #[test]
    fn the_pace_is_never_faster_than_a_keyboard_can_deliver() {
        let mut movement = Movement::new(true);
        movement.set_cell_px((2, 20));
        for direction in Direction::ALL {
            assert!(
                movement.interval(direction) >= MIN_INTERVAL,
                "{direction:?} asked for {:?}",
                movement.interval(direction)
            );
        }
    }

    // --- sprint -------------------------------------------------------------

    #[test]
    fn sprinting_is_faster_than_walking() {
        // Twice the pace: unmistakable, still controlled.
        const _: () = assert!(SPRINT_SPEED > WALK_SPEED);
        assert_eq!(2.0, SPRINT_SPEED / WALK_SPEED);
    }

    #[test]
    fn shift_makes_the_hold_a_sprint() {
        let mut movement = with_releases();
        movement.key(sprint_key('d'));
        assert!(movement.is_sprinting());
        let sprint = movement.interval(Direction::East);
        assert!(sprint < movement.interval(Direction::North));

        movement.update(sprint);
        assert_eq!(
            Motion::Step(Direction::East),
            movement.key(sprint_key('d').with_repeat())
        );
    }

    #[test]
    fn an_upper_case_letter_sprints_even_without_a_modifier_flag() {
        let mut movement = with_releases();
        movement.key(Key::char('D'));
        assert!(movement.is_sprinting());
    }

    #[test]
    fn releasing_shift_returns_to_a_walk() {
        let mut movement = with_releases();
        movement.key(sprint_key('d'));
        movement.update(movement.interval(Direction::East));
        movement.key(sprint_key('d').with_repeat());
        // Let go of shift: the next repeat is an ordinary walking press.
        movement.key(Key::char('d').with_repeat());
        assert!(!movement.is_sprinting());
        assert_eq!(
            Motion::Nothing,
            movement.key(Key::char('d').with_repeat()),
            "the walk cadence must re-apply at once"
        );
        movement.update(movement.interval(Direction::East));
        assert_eq!(
            Motion::Step(Direction::East),
            movement.key(Key::char('d').with_repeat())
        );
    }

    #[test]
    fn pressing_shift_mid_walk_speeds_the_next_step_up() {
        let mut movement = with_releases();
        movement.key(Key::char('d'));
        assert_eq!(Motion::Nothing, movement.key(Key::char('d')));
        assert!(!movement.is_sprinting());

        // Shift arrives: the pending step is pulled forward to the sprint rate.
        movement.key(sprint_key('d').with_repeat());
        assert!(movement.is_sprinting());
        movement.update(movement.interval(Direction::East));
        assert_eq!(
            Motion::Step(Direction::East),
            movement.key(sprint_key('d').with_repeat()),
            "sprinting should take effect immediately"
        );
    }

    #[test]
    fn sprinting_still_respects_collision_and_boundaries() {
        // The state machine only ever says "step"; the world decides whether the
        // step is legal.
        let mut movement = with_releases();
        movement.key(sprint_key('w'));
        let cadence = movement.interval(Direction::North);
        let mut steps = 0;
        for _ in 0..50 {
            movement.update(cadence);
            if movement
                .key(sprint_key('w').with_repeat())
                .direction()
                .is_some()
            {
                steps += 1;
            }
        }
        assert_eq!(50, steps);
        assert!(movement.is_held());
    }

    #[test]
    fn a_sprint_release_stops_the_walk_immediately() {
        let mut movement = with_releases();
        movement.key(sprint_key('w'));
        movement.update(movement.interval(Direction::North));
        let release = Key::char('w').released().with_shift();
        assert_eq!(Motion::Nothing, movement.key(release));
        assert_eq!(None, movement.held());
        assert!(!movement.is_sprinting());
        assert_eq!(Motion::Nothing, movement.update(Duration::from_millis(400)));
    }

    #[test]
    fn sprinting_is_event_driven_too() {
        for releases in [true, false] {
            let mut movement = Movement::new(releases);
            movement.set_cell_px(CELL);
            assert_eq!(
                Motion::Step(Direction::South),
                movement.key(sprint_key('s'))
            );
            for _ in 0..40 {
                assert_eq!(Motion::Nothing, movement.update(Duration::from_millis(120)));
            }
        }
    }

    #[test]
    fn the_stall_guard_is_generous_enough_for_real_repeats() {
        assert!(STALL_TIMEOUT.as_millis() >= 250);
        assert!(STALL_TIMEOUT > with_releases().interval(Direction::North));
    }

    #[test]
    fn the_speeds_are_sane() {
        assert!(
            (100.0..=260.0).contains(&WALK_SPEED),
            "walk speed {WALK_SPEED}"
        );
        assert!((220.0..=600.0).contains(&SPRINT_SPEED));
    }

    #[test]
    fn changing_the_cell_size_tightens_a_pending_step() {
        let mut movement = Movement::new(true);
        movement.set_cell_px((40, 40));
        movement.key(Key::char('d'));
        // A pending step must not wait out a cadence that no longer applies.
        movement.set_cell_px(CELL);
        assert!(movement.pending() <= movement.interval(Direction::East));
    }
}
