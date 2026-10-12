//! Character animation timing.
//!
//! The walk cycle is locked to **steps**, not to a free-running clock: the legs
//! change when the player actually moves, so the figure keeps pace with the world
//! whether it is walking at 60 ms a step or 133, and stops dead when movement
//! stops. Everything else - the idle breath, the blink - runs on its own slow
//! clock, which is what makes a standing character look alive without making the
//! screen busy.
//!
//! All of it is a function of elapsed time, advanced explicitly by the screen, so
//! it is deterministic and testable: no frame counts, and no dependence on how
//! fast the machine is.

use std::time::Duration;

use crate::game::Direction;

/// The animation frame target: sixty updates a second.
///
/// This is a *target*, not a promise: a frame is still only composed when
/// something has changed, so the loop runs at this rate while the world is
/// moving and far slower when it is not.
pub const FRAME_INTERVAL: Duration = Duration::from_micros(16_667);

/// How many walk frames a direction has.
pub const WALK_FRAMES: usize = 2;

/// How often a standing character blinks.
pub const BLINK_PERIOD: Duration = Duration::from_millis(4_100);

/// How long a blink lasts.
pub const BLINK_LENGTH: Duration = Duration::from_millis(130);

/// What a character is doing, which is what a sprite is chosen for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Standing still.
    Idle,
    /// Walking: a stride cycle.
    Walk,
    /// Sprinting: the same stride, faster, with a lean and a trail.
    Sprint,
}

/// How a character should be drawn this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pose {
    /// Which way they face.
    pub facing: Direction,
    /// What they are doing.
    pub stage: Stage,
    /// Which stride frame, wrapped by the renderer.
    pub frame: usize,
    /// Whether the eyes are closed this frame.
    pub blinking: bool,
}

impl Pose {
    /// A settled pose facing `facing`, for tests and still figures.
    #[must_use]
    pub const fn idle(facing: Direction) -> Self {
        Pose {
            facing,
            stage: Stage::Idle,
            frame: 0,
            blinking: false,
        }
    }
}

/// The player's animation state.
#[derive(Debug, Clone)]
pub struct Performance {
    /// Which stride frame is showing.
    frame: usize,
    /// Time since the last step.
    since_step: Duration,
    /// How long the last step took, which is how long its animation lasts.
    cadence: Duration,
    /// Free-running clock for idle motion.
    idle: Duration,
}

impl Performance {
    /// Creates a settled performance.
    #[must_use]
    pub const fn new() -> Self {
        Performance {
            frame: 0,
            since_step: Duration::ZERO,
            cadence: Duration::ZERO,
            // Start just past a blink, so a figure arriving on screen has its eyes
            // open rather than mid-blink.
            idle: BLINK_LENGTH,
        }
    }

    /// Records that a step was taken, advancing the stride.
    ///
    /// Called once per successful step, so the cycle cannot drift away from the
    /// movement it is illustrating.
    pub fn stepped(&mut self, cadence: Duration) {
        self.frame = (self.frame + 1) % WALK_FRAMES;
        self.since_step = Duration::ZERO;
        self.cadence = cadence;
    }

    /// Advances the clocks.
    pub fn advance(&mut self, delta: Duration) {
        self.since_step = self.since_step.saturating_add(delta);
        self.idle = self.idle.saturating_add(delta);
    }

    /// Whether a step is still playing out.
    #[must_use]
    pub fn is_stepping(&self) -> bool {
        self.since_step < self.cadence
    }

    /// Whether the character should be drawn as moving.
    ///
    /// A key held down counts, and so does a step still playing out after the key
    /// was released, so a tap does not snap back to idle mid-stride.
    #[must_use]
    pub fn is_moving(&self, held: bool) -> bool {
        held || self.is_stepping()
    }

    /// The pose to draw for a character facing `facing`.
    #[must_use]
    pub fn pose(&self, facing: Direction, held: bool, sprinting: bool) -> Pose {
        if self.is_moving(held) {
            Pose {
                facing,
                stage: if sprinting {
                    Stage::Sprint
                } else {
                    Stage::Walk
                },
                frame: self.frame,
                blinking: false,
            }
        } else {
            Pose {
                facing,
                stage: Stage::Idle,
                frame: 0,
                blinking: self.blinking(),
            }
        }
    }

    /// Whether a standing figure's eyes are closed this instant.
    #[must_use]
    pub fn blinking(&self) -> bool {
        self.idle.as_nanos() % BLINK_PERIOD.as_nanos() < BLINK_LENGTH.as_nanos()
    }

    /// Whether anything about the performance needs continuous frames.
    ///
    /// While walking this is true; while standing it is false, because the idle
    /// breath is slow enough for the screen's ambient tick.
    #[must_use]
    pub fn is_animated(&self, held: bool) -> bool {
        held || self.is_stepping()
    }

    /// The stride frame, for diagnostics.
    #[must_use]
    pub const fn frame(&self) -> usize {
        self.frame
    }
}

impl Default for Performance {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{BLINK_LENGTH, BLINK_PERIOD, Performance, Stage, WALK_FRAMES};
    use crate::game::Direction;
    use std::time::Duration;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    #[test]
    fn a_settled_performance_is_idle_and_still() {
        let performance = Performance::new();
        let pose = performance.pose(Direction::South, false, false);
        assert_eq!(Stage::Idle, pose.stage);
        assert!(!performance.is_moving(false));
        assert!(!performance.is_animated(false));
    }

    #[test]
    fn each_step_advances_the_stride_and_wraps() {
        let mut performance = Performance::new();
        assert_eq!(0, performance.frame());
        performance.stepped(ms(120));
        assert_eq!(1, performance.frame());
        performance.stepped(ms(120));
        assert_eq!(
            0,
            performance.frame(),
            "the stride wraps after {WALK_FRAMES}"
        );
    }

    #[test]
    fn a_step_keeps_playing_after_the_key_is_released() {
        // A tap should finish its stride rather than snap to idle.
        let mut performance = Performance::new();
        performance.stepped(ms(120));
        assert!(performance.is_moving(false), "the step is still playing");
        assert_eq!(
            Stage::Walk,
            performance.pose(Direction::East, false, false).stage
        );
        // Once it has played out, the figure settles.
        performance.advance(ms(200));
        assert!(!performance.is_moving(false));
        assert_eq!(
            Stage::Idle,
            performance.pose(Direction::East, false, false).stage
        );
    }

    #[test]
    fn holding_a_key_keeps_the_walk_pose_between_steps() {
        let mut performance = Performance::new();
        performance.stepped(ms(60));
        performance.advance(ms(500));
        assert!(performance.is_moving(true));
        assert_eq!(
            Stage::Walk,
            performance.pose(Direction::North, true, false).stage
        );
        assert!(performance.is_animated(true));
    }

    #[test]
    fn sprinting_is_a_different_stage_from_walking() {
        let mut performance = Performance::new();
        performance.stepped(ms(60));
        assert_eq!(
            Stage::Sprint,
            performance.pose(Direction::South, true, true).stage
        );
        assert_eq!(
            Stage::Walk,
            performance.pose(Direction::South, true, false).stage
        );
    }

    #[test]
    fn the_pose_carries_the_facing_direction() {
        let performance = Performance::new();
        for facing in Direction::ALL {
            assert_eq!(facing, performance.pose(facing, false, false).facing);
        }
    }

    #[test]
    fn a_standing_figure_blinks_periodically_without_animating_the_frame_loop() {
        let mut performance = Performance::new();
        assert!(!performance.blinking(), "eyes open when it arrives");

        // A blink comes round within one period.
        performance.advance(BLINK_PERIOD - BLINK_LENGTH);
        assert!(performance.blinking());
        performance.advance(BLINK_LENGTH);
        assert!(!performance.blinking(), "and it opens again");

        // Blinking never requests a continuous frame.
        assert!(!performance.is_animated(false));
    }

    #[test]
    fn a_moving_figure_does_not_blink() {
        let mut performance = Performance::new();
        // Wind the idle clock into the middle of a blink.
        performance.advance(BLINK_PERIOD - BLINK_LENGTH / 2);
        assert!(performance.blinking(), "the idle clock is mid-blink");
        assert!(
            !performance.pose(Direction::South, true, false).blinking,
            "a walking figure has its eyes open"
        );
    }

    #[test]
    fn timing_is_a_function_of_elapsed_time_not_of_call_count() {
        // The same total time in different increments must give the same pose.
        let mut coarse = Performance::new();
        coarse.stepped(ms(120));
        coarse.advance(ms(120));

        let mut fine = Performance::new();
        fine.stepped(ms(120));
        for _ in 0..12 {
            fine.advance(ms(10));
        }
        assert_eq!(coarse.frame(), fine.frame());
        assert_eq!(coarse.is_stepping(), fine.is_stepping());
        assert_eq!(coarse.blinking(), fine.blinking());
    }

    #[test]
    fn a_delayed_frame_does_not_skip_or_double_advance() {
        // One long delta must behave exactly like the sum of short ones.
        let mut stalled = Performance::new();
        stalled.stepped(ms(120));
        stalled.advance(ms(400));

        let mut stepped = Performance::new();
        stepped.stepped(ms(120));
        for _ in 0..40 {
            stepped.advance(ms(10));
        }
        assert_eq!(stepped.frame(), stalled.frame());
        assert!(!stalled.is_stepping());
        assert!(!stepped.is_stepping());
    }
}
