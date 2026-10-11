//! Easing functions that shape animation progress.

/// Maps linear progress in `[0, 1]` onto an eased value.
///
/// Easing is applied to the normalised progress; the caller then interpolates
/// between the animation's `from` and `to` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Easing {
    /// Constant speed.
    #[default]
    Linear,
    /// Slow start, fast end.
    EaseInQuad,
    /// Fast start, slow end.
    EaseOutQuad,
    /// Slow at both ends, using a quadratic curve.
    EaseInOutQuad,
    /// Fast start, gentle end, using a cubic curve.
    EaseOutCubic,
    /// Slow at both ends, using a cubic curve.
    EaseInOutCubic,
    /// Overshoots slightly before settling, for a springy feel.
    EaseOutBack,
}

impl Easing {
    /// Applies the curve to `t`, clamping the input to `[0, 1]`.
    #[must_use]
    pub fn apply(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => t * (2.0 - t),
            Easing::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
            Easing::EaseOutCubic => {
                let inv = 1.0 - t;
                1.0 - inv * inv * inv
            }
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    let inv = -2.0 * t + 2.0;
                    1.0 - inv * inv * inv / 2.0
                }
            }
            Easing::EaseOutBack => {
                const C1: f32 = 1.70158;
                const C3: f32 = C1 + 1.0;
                let inv = t - 1.0;
                1.0 + C3 * inv * inv * inv + C1 * inv * inv
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Easing;

    const ALL: [Easing; 7] = [
        Easing::Linear,
        Easing::EaseInQuad,
        Easing::EaseOutQuad,
        Easing::EaseInOutQuad,
        Easing::EaseOutCubic,
        Easing::EaseInOutCubic,
        Easing::EaseOutBack,
    ];

    #[test]
    fn every_curve_is_pinned_at_the_ends() {
        for easing in ALL {
            assert!(easing.apply(0.0).abs() < 1e-6, "{easing:?} at 0");
            assert!((easing.apply(1.0) - 1.0).abs() < 1e-6, "{easing:?} at 1");
        }
    }

    #[test]
    fn input_is_clamped() {
        for easing in ALL {
            assert!(easing.apply(-5.0).abs() < 1e-6, "{easing:?} below range");
            assert!(
                (easing.apply(5.0) - 1.0).abs() < 1e-6,
                "{easing:?} above range"
            );
        }
    }

    #[test]
    fn linear_is_the_identity() {
        assert!((Easing::Linear.apply(0.25) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn ease_in_starts_slower_than_linear() {
        assert!(Easing::EaseInQuad.apply(0.5) < 0.5);
    }

    #[test]
    fn ease_out_ends_slower_than_linear() {
        assert!(Easing::EaseOutQuad.apply(0.5) > 0.5);
    }

    #[test]
    fn ease_out_back_overshoots_before_settling() {
        let peak = (1..100)
            .map(|i| Easing::EaseOutBack.apply(i as f32 / 100.0))
            .fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "expected overshoot, peak was {peak}");
    }
}
