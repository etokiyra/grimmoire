//! Movement input: press, hold, and release.
//!
//! Movement is a separate, explicit model rather than a throttle on the raw key
//! stream, because the three things a player notices are distinct:
//!
//! * a **fresh press** must step immediately - no gate, no latency;
//! * **holding** must walk at a steady, predictable cadence rather than at
//!   whatever rate the terminal happens to auto-repeat at;
//! * **releasing** must stop promptly.
//!
//! Steps are taken **on events, never on a timer**. A press moves at once, an
//! auto-repeat moves again if the cadence allows, and that is all: a tap is
//! exactly one step, and no sequence of lost or missing events can leave the
//! player walking on their own. Kitty reports key releases, so a release stops
//! the walk at once; a stall guard also clears the held state when events stop
//! arriving, so a lost release cannot even leave the movement indicator stuck.
//!
//! Discrete actions are deliberately *not* handled here - they must fire once per
//! press and never on auto-repeat, which callers enforce by checking
//! [`Key::is_down`](crate::input::Key::is_down) and
//! [`Key::is_repeat`](crate::input::Key::is_repeat).

use std::time::Duration;

use crate::game::Direction;
use crate::input::{Key, KeyCode};

/// How long between automatic steps while a direction is held.
///
/// Roughly eight steps a second: brisk enough to cross a room without feeling
/// laggy, slow enough to stop where you meant to. This is the one number to tune
/// when movement feels wrong.
pub const REPEAT_INTERVAL: Duration = Duration::from_millis(125);

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
    /// Time until the next automatic step.
    timer: Duration,
    /// Time since the last movement event, for the stall guard.
    silence: Duration,
    /// Whether the terminal reports key releases. Diagnostics only: the stall
    /// guard applies either way.
    releases: bool,
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
            timer: Duration::ZERO,
            silence: Duration::ZERO,
            releases,
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

    /// Feeds one key event.
    pub fn key(&mut self, key: Key) -> Motion {
        let Some(direction) = Self::direction_for(key.code) else {
            return Motion::Nothing;
        };

        if key.is_release() {
            if self.held == Some(direction) {
                self.held = None;
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

        if self.held == Some(direction) {
            // The same direction again: either the terminal auto-repeated, or
            // this is a second physical press. Both take the cadence, which
            // keeps the pace ours rather than the terminal's.
            return self.step_if_due(direction);
        }

        // A new direction moves at once, so turning and starting feel immediate.
        self.held = Some(direction);
        self.silence = Duration::ZERO;
        self.timer = REPEAT_INTERVAL;
        Motion::Step(direction)
    }

    /// Advances time. This never moves the player: motion is driven by events.
    pub fn update(&mut self, delta: Duration) -> Motion {
        self.timer = self.timer.saturating_sub(delta);
        self.silence = self.silence.saturating_add(delta);
        if self.held.is_some() && self.silence >= STALL_TIMEOUT {
            self.held = None;
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
        self.timer = Duration::ZERO;
        self.silence = Duration::ZERO;
    }

    /// Takes a step on the cadence if enough time has passed.
    fn step_if_due(&mut self, direction: Direction) -> Motion {
        self.silence = Duration::ZERO;
        if self.timer == Duration::ZERO {
            self.timer = REPEAT_INTERVAL;
            Motion::Step(direction)
        } else {
            Motion::Nothing
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Motion, Movement, REPEAT_INTERVAL, STALL_TIMEOUT};
    use crate::game::Direction;
    use crate::input::{Key, KeyCode};

    fn with_releases() -> Movement {
        Movement::new(true)
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
            assert_eq!(Motion::Step(Direction::North), movement.key(Key::char('w')));
            for _ in 0..40 {
                assert_eq!(
                    Motion::Nothing,
                    movement.update(REPEAT_INTERVAL),
                    "releases={releases}: time must not move the player"
                );
            }
        }
    }

    #[test]
    fn auto_repeats_walk_at_our_cadence_not_the_terminals() {
        let mut movement = with_releases();
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
        movement.update(REPEAT_INTERVAL);
        assert_eq!(
            Motion::Step(Direction::East),
            movement.key(Key::char('d').with_repeat())
        );
    }

    #[test]
    fn a_steady_repeat_stream_walks_steadily() {
        let mut movement = with_releases();
        movement.key(Key::char('d'));
        let mut steps = 0;
        for _ in 0..40 {
            movement.update(REPEAT_INTERVAL);
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
        movement.update(REPEAT_INTERVAL);
        assert!(movement.is_held());

        assert_eq!(Motion::Nothing, movement.key(Key::char('d').released()));
        assert_eq!(None, movement.held());
        // Time passing no longer moves anybody.
        assert_eq!(Motion::Nothing, movement.update(REPEAT_INTERVAL * 3));
    }

    #[test]
    fn changing_direction_is_immediate_and_switches_the_cadence() {
        let mut movement = with_releases();
        movement.key(Key::char('d'));
        // A new direction steps at once, even inside the previous cadence.
        assert_eq!(Motion::Step(Direction::West), movement.key(Key::char('a')));
        assert_eq!(Some(Direction::West), movement.held());
        // And the cadence restarts, so an immediate repeat is absorbed.
        assert_eq!(Motion::Nothing, movement.key(Key::char('a').with_repeat()));
        movement.update(REPEAT_INTERVAL);
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
        movement.update(REPEAT_INTERVAL);
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
        assert_eq!(Motion::Nothing, movement.update(REPEAT_INTERVAL * 2));
    }

    #[test]
    fn the_stall_guard_is_generous_enough_for_real_repeats() {
        // Well above a plausible auto-repeat interval (kitty repeats every few
        // tens of milliseconds), so a genuine hold is never cut short.
        assert!(STALL_TIMEOUT.as_millis() >= 250);
        assert!(STALL_TIMEOUT > REPEAT_INTERVAL);
    }

    #[test]
    fn the_reported_cadence_is_roughly_eight_steps_a_second() {
        let interval_ms = REPEAT_INTERVAL.as_millis();
        assert!(
            (100..=160).contains(&interval_ms),
            "cadence {interval_ms} ms is outside the intended range"
        );
    }

    use std::time::Duration;
}
