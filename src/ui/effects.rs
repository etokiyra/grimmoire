//! Transient world effects: the brief visual answer to what just happened.
//!
//! When a step is blocked or an interaction lands, the player should be able to
//! see it without reading the message line. This module owns that: short, named
//! pulses anchored to a world position, each with a birth time and a length.
//!
//! It is deliberately tiny and pure. Effects never change world state, never
//! block anything, and expire on their own; they are drawn by
//! [`crate::ui::scenery`] as a change of style on whatever is already there, so
//! a pulse can never hide a character, a wall, or an exit.
//!
//! Everything here is a function of elapsed time and a fixed seed, so the same
//! inputs always produce the same frame: no effect uses randomness or the frame
//! count.

use std::time::Duration;

use crate::game::Position;

/// How long a pulse lasts.
pub const LENGTH: Duration = Duration::from_millis(240);

/// The most effects kept at once.
///
/// A pulse is a nudge, not a history. Beyond a handful the screen would flicker
/// under its own feedback, so the oldest is dropped.
pub const CAPACITY: usize = 6;

/// What a pulse is saying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pulse {
    /// A step was refused: something solid is in the way.
    Blocked,
    /// Something was examined or spoken to.
    Touched,
    /// The player arrived somewhere new.
    Arrived,
}

/// One live pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Effect {
    at: Position,
    kind: Pulse,
    born: Duration,
}

/// The live effects and the clock they are measured against.
#[derive(Debug, Clone, Default)]
pub struct Effects {
    clock: Duration,
    live: Vec<Effect>,
}

impl Effects {
    /// Creates an empty set.
    #[must_use]
    pub const fn new() -> Self {
        Effects {
            clock: Duration::ZERO,
            live: Vec::new(),
        }
    }

    /// The elapsed time on the effects clock.
    #[must_use]
    pub const fn now(&self) -> Duration {
        self.clock
    }

    /// Advances the clock and drops anything that has finished.
    pub fn advance(&mut self, delta: Duration) {
        self.clock = self.clock.saturating_add(delta);
        let clock = self.clock;
        self.live
            .retain(|effect| clock.saturating_sub(effect.born) < LENGTH);
    }

    /// Starts a pulse at `at`, replacing any pulse already there.
    ///
    /// Replacing rather than appending is what keeps a held-against-a-wall step
    /// from stacking hundreds of identical effects.
    pub fn spawn(&mut self, at: Position, kind: Pulse) {
        self.live.retain(|effect| effect.at != at);
        if self.live.len() >= CAPACITY {
            self.live.remove(0);
        }
        self.live.push(Effect {
            at,
            kind,
            born: self.clock,
        });
    }

    /// Whether any pulse is still alive, which is when frames are needed.
    #[must_use]
    pub fn is_active(&self) -> bool {
        !self.live.is_empty()
    }

    /// How many pulses are alive.
    #[must_use]
    pub fn len(&self) -> usize {
        self.live.len()
    }

    /// Whether there are no pulses.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.live.is_empty()
    }

    /// The pulse at `at`, with an intensity that falls from 1 to 0.
    ///
    /// The curve is eased so the pulse lands hard and fades out, which reads as
    /// a flick rather than a slow fade.
    #[must_use]
    pub fn sample(&self, at: Position) -> Option<(Pulse, f32)> {
        self.live
            .iter()
            .rev()
            .find(|effect| effect.at == at)
            .map(|effect| {
                let age = self.clock.saturating_sub(effect.born);
                let t = (age.as_secs_f32() / LENGTH.as_secs_f32()).clamp(0.0, 1.0);
                (effect.kind, 1.0 - t * t)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{CAPACITY, Effects, LENGTH, Pulse};
    use crate::game::Position;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    use std::time::Duration;

    #[test]
    fn a_pulse_starts_at_full_strength_and_fades_out() {
        let mut effects = Effects::new();
        effects.spawn(Position::new(3, 4), Pulse::Touched);
        assert_eq!(
            Some((Pulse::Touched, 1.0)),
            effects.sample(Position::new(3, 4))
        );

        effects.advance(LENGTH / 2);
        let (kind, strength) = effects.sample(Position::new(3, 4)).expect("still live");
        assert_eq!(Pulse::Touched, kind);
        assert!(strength < 1.0 && strength > 0.0, "strength {strength}");
        assert!(effects.is_active());
    }

    #[test]
    fn a_pulse_expires_on_its_own() {
        let mut effects = Effects::new();
        effects.spawn(Position::new(1, 1), Pulse::Blocked);
        effects.advance(LENGTH);
        assert_eq!(None, effects.sample(Position::new(1, 1)));
        assert!(!effects.is_active());
        assert!(effects.is_empty());
    }

    #[test]
    fn the_same_position_is_replaced_rather_than_stacked() {
        let mut effects = Effects::new();
        for _ in 0..20 {
            effects.spawn(Position::new(2, 2), Pulse::Blocked);
            effects.advance(ms(5));
        }
        assert_eq!(1, effects.len());
    }

    #[test]
    fn the_capacity_is_bounded() {
        let mut effects = Effects::new();
        for x in 0..20u16 {
            effects.spawn(Position::new(x, 0), Pulse::Touched);
        }
        assert_eq!(CAPACITY, effects.len());
        // The oldest went; the newest stayed.
        assert!(effects.sample(Position::new(19, 0)).is_some());
        assert!(effects.sample(Position::new(0, 0)).is_none());
    }

    #[test]
    fn effects_for_different_positions_coexist() {
        let mut effects = Effects::new();
        effects.spawn(Position::new(0, 0), Pulse::Touched);
        effects.spawn(Position::new(5, 5), Pulse::Blocked);
        assert_eq!(2, effects.len());
        assert_eq!(
            Some((Pulse::Touched, 1.0)),
            effects.sample(Position::new(0, 0))
        );
        assert_eq!(
            Some((Pulse::Blocked, 1.0)),
            effects.sample(Position::new(5, 5))
        );
        assert_eq!(None, effects.sample(Position::new(1, 1)));
    }

    #[test]
    fn spawning_does_not_move_the_clock() {
        let mut effects = Effects::new();
        effects.advance(ms(100));
        effects.spawn(Position::new(0, 0), Pulse::Arrived);
        assert_eq!(ms(100), effects.now());
        assert_eq!(
            Some((Pulse::Arrived, 1.0)),
            effects.sample(Position::new(0, 0))
        );
    }
}
