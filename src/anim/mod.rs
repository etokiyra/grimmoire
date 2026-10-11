//! Time-based animation, decoupled from the frame rate and from input.
//!
//! Animations are functions of elapsed time, not of frame counts. An [`Animator`]
//! owns a single clock that the application advances explicitly, which makes
//! progress deterministic and testable: tests advance the clock by exact
//! durations instead of sleeping.
//!
//! ```no_run
//! use std::time::Duration;
//! use grimmoire::anim::{Animation, Animator, Easing};
//!
//! #[derive(Clone, Copy, PartialEq, Eq)]
//! enum Key { Reveal }
//!
//! let mut animator = Animator::new();
//! animator.start(Key::Reveal, Animation::new(Duration::from_millis(400)).eased(Easing::EaseOutCubic));
//! animator.tick(Duration::from_millis(200));
//! assert!(animator.is_active(Key::Reveal));
//! ```

mod easing;

pub use easing::Easing;

use std::time::Duration;

/// A single animation interpolating a scalar over a fixed duration.
///
/// The value is `from` before any delay elapses and exactly `to` once the
/// animation completes, so it is safe to read at any time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    delay: Duration,
    duration: Duration,
    from: f32,
    to: f32,
    easing: Easing,
}

impl Animation {
    /// Creates a linear animation from `0.0` to `1.0` lasting `duration`.
    #[must_use]
    pub const fn new(duration: Duration) -> Self {
        Animation {
            delay: Duration::ZERO,
            duration,
            from: 0.0,
            to: 1.0,
            easing: Easing::Linear,
        }
    }

    /// Sets the values interpolated between.
    #[must_use]
    pub const fn from_to(mut self, from: f32, to: f32) -> Self {
        self.from = from;
        self.to = to;
        self
    }

    /// Delays the start of the animation by `delay` after it is begun.
    #[must_use]
    pub const fn delayed(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    /// Sets the easing curve.
    #[must_use]
    pub const fn eased(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    /// The delay before the animation starts.
    #[must_use]
    pub const fn delay(&self) -> Duration {
        self.delay
    }

    /// The active duration, excluding any delay.
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    /// The total time from start to completion, including any delay.
    #[must_use]
    pub const fn total(&self) -> Duration {
        self.delay.saturating_add(self.duration)
    }

    /// Un-eased progress in `[0, 1]` at `elapsed` since the animation began.
    #[must_use]
    pub fn progress(&self, elapsed: Duration) -> f32 {
        if elapsed < self.delay {
            return 0.0;
        }
        if self.duration.is_zero() {
            return 1.0;
        }
        let active = elapsed.saturating_sub(self.delay);
        (active.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// The eased, interpolated value at `elapsed` since the animation began.
    #[must_use]
    pub fn value_at(&self, elapsed: Duration) -> f32 {
        let eased = self.easing.apply(self.progress(elapsed));
        self.from + (self.to - self.from) * eased
    }

    /// Whether the animation has finished at `elapsed`.
    #[must_use]
    pub fn is_complete(&self, elapsed: Duration) -> bool {
        elapsed >= self.total()
    }
}

/// One tracked animation and the clock value at which it began.
#[derive(Debug, Clone, Copy)]
struct Entry<K> {
    key: K,
    animation: Animation,
    started: Duration,
}

/// Drives any number of keyed animations from a single, manually advanced clock.
///
/// Keys let several animations run concurrently and be looked up by name. The
/// clock is a plain [`Duration`] that grows monotonically as the application
/// ticks the animator, which keeps all progress values deterministic.
#[derive(Debug, Clone)]
pub struct Animator<K: Copy + Eq> {
    now: Duration,
    entries: Vec<Entry<K>>,
}

impl<K: Copy + Eq> Animator<K> {
    /// Creates an animator with no animations and a zero clock.
    #[must_use]
    pub const fn new() -> Self {
        Animator {
            now: Duration::ZERO,
            entries: Vec::new(),
        }
    }

    /// The elapsed time on the shared clock.
    #[must_use]
    pub const fn now(&self) -> Duration {
        self.now
    }

    /// Advances the shared clock by `delta`.
    pub fn tick(&mut self, delta: Duration) {
        self.now = self.now.saturating_add(delta);
    }

    /// Starts (or restarts) the animation stored under `key`.
    ///
    /// Starting an existing key replaces the animation and restarts it from the
    /// current clock value, which is how a screen replays a transition.
    pub fn start(&mut self, key: K, animation: Animation) {
        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == key) {
            entry.animation = animation;
            entry.started = self.now;
        } else {
            self.entries.push(Entry {
                key,
                animation,
                started: self.now,
            });
        }
    }

    /// Cancels `key`, returning whether it was present.
    pub fn cancel(&mut self, key: K) -> bool {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.key != key);
        self.entries.len() != before
    }

    /// Whether `key` is currently tracked.
    #[must_use]
    pub fn contains(&self, key: K) -> bool {
        self.entries.iter().any(|entry| entry.key == key)
    }

    /// The elapsed time since `entry` began.
    fn elapsed(&self, entry: &Entry<K>) -> Duration {
        self.now.saturating_sub(entry.started)
    }

    /// The current value of `key`, if it is tracked.
    #[must_use]
    pub fn value(&self, key: K) -> Option<f32> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.animation.value_at(self.elapsed(entry)))
    }

    /// The current value of `key`, or `default` when it is not tracked.
    #[must_use]
    pub fn value_or(&self, key: K, default: f32) -> f32 {
        self.value(key).unwrap_or(default)
    }

    /// The current un-eased progress of `key`, if tracked.
    #[must_use]
    pub fn progress(&self, key: K) -> Option<f32> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.animation.progress(self.elapsed(entry)))
    }

    /// Whether `key` is tracked and has not yet completed.
    #[must_use]
    pub fn is_active(&self, key: K) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .is_some_and(|entry| !entry.animation.is_complete(self.elapsed(entry)))
    }

    /// Whether `key` is tracked and has completed.
    #[must_use]
    pub fn is_complete(&self, key: K) -> bool {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .is_some_and(|entry| entry.animation.is_complete(self.elapsed(entry)))
    }

    /// The number of animations still running.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !entry.animation.is_complete(self.elapsed(entry)))
            .count()
    }

    /// The number of tracked animations, including completed ones.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether no animations are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drops animations that have completed.
    pub fn retain_active(&mut self) {
        let now = self.now;
        self.entries.retain(|entry| {
            !entry
                .animation
                .is_complete(now.saturating_sub(entry.started))
        });
    }

    /// Removes every tracked animation.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl<K: Copy + Eq> Default for Animator<K> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{Animation, Animator, Easing};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Key {
        A,
        B,
    }

    fn ms(millis: u64) -> std::time::Duration {
        std::time::Duration::from_millis(millis)
    }

    #[test]
    fn value_tracks_elapsed_time_linearly() {
        let animation = Animation::new(ms(1000));
        assert!((animation.value_at(ms(0)) - 0.0).abs() < 1e-6);
        assert!((animation.value_at(ms(500)) - 0.5).abs() < 1e-6);
        assert!((animation.value_at(ms(1000)) - 1.0).abs() < 1e-6);
        assert!((animation.value_at(ms(5000)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn from_to_and_easing_shape_the_value() {
        let animation = Animation::new(ms(100))
            .from_to(10.0, 20.0)
            .eased(Easing::EaseInQuad);
        assert!((animation.value_at(ms(50)) - 12.5).abs() < 1e-4);
        assert!((animation.value_at(ms(100)) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn delay_holds_the_start_value() {
        let animation = Animation::new(ms(100)).from_to(0.0, 1.0).delayed(ms(200));
        assert!((animation.value_at(ms(150)) - 0.0).abs() < 1e-6);
        assert!((animation.value_at(ms(250)) - 0.5).abs() < 1e-4);
        assert!(animation.is_complete(ms(300)));
        assert!(!animation.is_complete(ms(299)));
    }

    #[test]
    fn zero_duration_completes_immediately() {
        let animation = Animation::new(std::time::Duration::ZERO);
        assert!(animation.is_complete(ms(0)));
        assert!((animation.progress(ms(0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn animator_advances_only_when_ticked() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(100)));
        assert!((animator.value(Key::A).unwrap() - 0.0).abs() < 1e-6);

        animator.tick(ms(50));
        assert!((animator.value(Key::A).unwrap() - 0.5).abs() < 1e-4);
        assert!(animator.is_active(Key::A));

        animator.tick(ms(50));
        assert!(animator.is_complete(Key::A));
        assert_eq!(0, animator.active_count());
    }

    #[test]
    fn several_animations_run_concurrently() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(100)));
        animator.tick(ms(50));
        animator.start(Key::B, Animation::new(ms(200)));

        // B began at t=50, so at t=100 it is a quarter through.
        animator.tick(ms(50));
        assert!((animator.value(Key::A).unwrap() - 1.0).abs() < 1e-4);
        assert!((animator.value(Key::B).unwrap() - 0.25).abs() < 1e-4);
        assert_eq!(1, animator.active_count());
    }

    #[test]
    fn restarting_a_key_replays_from_the_current_clock() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(100)));
        animator.tick(ms(80));
        animator.start(Key::A, Animation::new(ms(100)));
        assert!((animator.value(Key::A).unwrap() - 0.0).abs() < 1e-6);
        assert_eq!(1, animator.len());
    }

    #[test]
    fn cancel_removes_the_animation() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(100)));
        assert!(animator.cancel(Key::A));
        assert!(!animator.cancel(Key::A));
        assert_eq!(None, animator.value(Key::A));
        assert!(animator.is_empty());
    }

    #[test]
    fn unknown_keys_report_defaults() {
        let animator: Animator<Key> = Animator::new();
        assert_eq!(None, animator.value(Key::A));
        assert_eq!(0.7, animator.value_or(Key::A, 0.7));
        assert!(!animator.is_active(Key::A));
    }

    #[test]
    fn retain_active_drops_finished_animations_only() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(50)));
        animator.start(Key::B, Animation::new(ms(500)));
        animator.tick(ms(100));

        animator.retain_active();
        assert!(!animator.contains(Key::A));
        assert!(animator.contains(Key::B));
    }

    #[test]
    fn value_or_returns_the_default_after_retention() {
        let mut animator = Animator::new();
        animator.start(Key::A, Animation::new(ms(50)));
        animator.tick(ms(100));
        animator.retain_active();
        assert_eq!(0.0, animator.value_or(Key::A, 0.0));
    }
}
