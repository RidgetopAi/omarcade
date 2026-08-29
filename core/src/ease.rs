//! Easing curves and interpolation.
//!
//! Motion that starts and stops abruptly reads as cheap, and a game whose
//! state changes are step functions has nowhere to put the difference.
//! These are the curves that make a transition feel considered.
//!
//! Every function maps `0.0..=1.0` to a progress value, clamping its
//! input — a caller computing `elapsed / duration` will overshoot on a
//! long frame, and a curve that returns 1.4 there would visibly glitch.
//! Most return `0.0..=1.0`; the overshooting curves say so.
//!
//! Nothing here allocates or touches a clock: a game owns its own
//! elapsed time, which keeps the headless harnesses deterministic.

/// Linear interpolation from `a` to `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Where `v` sits between `a` and `b`, as `0.0..=1.0`. The inverse of
/// [`lerp`]. Returns 0.0 for a zero-width range rather than dividing by it.
pub fn inverse_lerp(a: f32, b: f32, v: f32) -> f32 {
    if (b - a).abs() < f32::EPSILON {
        return 0.0;
    }
    ((v - a) / (b - a)).clamp(0.0, 1.0)
}

/// No easing.
pub fn linear(t: f32) -> f32 {
    t.clamp(0.0, 1.0)
}

/// Slow start. Good for something leaving rest.
pub fn in_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t
}

/// Fast start, gentle stop. The default for something arriving —
/// a panel sliding in, a score counting up.
pub fn out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease at both ends. The workhorse for a move between two rest states.
pub fn in_out_quad(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

/// Sharper arrival than [`out_quad`]. The one to reach for when motion
/// should feel snappy rather than soft.
pub fn out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

pub fn in_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * t
}

pub fn in_out_cubic(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// Overshoots past 1.0 and settles back — a small "pop".
///
/// **Returns beyond `0.0..=1.0`.** Do not feed it to anything that will
/// index or clamp on the assumption of a unit range; it is for scale and
/// position, where the overshoot is the entire point.
pub fn out_back(t: f32) -> f32 {
    // 1.70158 is the classic Penner constant: ~10% overshoot.
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let t = t.clamp(0.0, 1.0);
    1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
}

/// Decaying bounce, settling at 1.0. For an impact landing.
pub fn out_bounce(t: f32) -> f32 {
    const N1: f32 = 7.5625;
    const D1: f32 = 2.75;
    let mut t = t.clamp(0.0, 1.0);

    if t < 1.0 / D1 {
        N1 * t * t
    } else if t < 2.0 / D1 {
        t -= 1.5 / D1;
        N1 * t * t + 0.75
    } else if t < 2.5 / D1 {
        t -= 2.25 / D1;
        N1 * t * t + 0.9375
    } else {
        t -= 2.625 / D1;
        N1 * t * t + 0.984375
    }
}

/// A value that decays from 1.0 to 0.0 over `duration` seconds.
///
/// The shape most game juice needs: a hit flash, a screen shake, a
/// particle's life. Drive it with `dt` from `Game::update` and read
/// [`Decay::progress`] in `render`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decay {
    elapsed: f32,
    duration: f32,
}

impl Decay {
    /// A decay that has already finished, so it draws nothing until fired.
    pub fn new(duration: f32) -> Self {
        Decay { elapsed: duration.max(f32::EPSILON), duration: duration.max(f32::EPSILON) }
    }

    /// Restart from full.
    pub fn fire(&mut self) {
        self.elapsed = 0.0;
    }

    /// Advance. Safe to call every frame whether or not it is running.
    pub fn tick(&mut self, dt: f32) {
        if self.elapsed < self.duration {
            self.elapsed = (self.elapsed + dt.max(0.0)).min(self.duration);
        }
    }

    /// `1.0` just after [`fire`](Self::fire), falling to `0.0`.
    pub fn progress(&self) -> f32 {
        1.0 - (self.elapsed / self.duration).clamp(0.0, 1.0)
    }

    pub fn is_active(&self) -> bool {
        self.elapsed < self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every curve must pin both ends, or a transition starts or lands
    /// somewhere other than where the caller placed it.
    #[test]
    fn curves_start_at_zero_and_end_at_one() {
        let curves: [(&str, fn(f32) -> f32); 8] = [
            ("linear", linear),
            ("in_quad", in_quad),
            ("out_quad", out_quad),
            ("in_out_quad", in_out_quad),
            ("in_cubic", in_cubic),
            ("out_cubic", out_cubic),
            ("in_out_cubic", in_out_cubic),
            ("out_bounce", out_bounce),
        ];
        for (name, f) in curves {
            assert!(f(0.0).abs() < 1e-5, "{name}(0) = {}", f(0.0));
            assert!((f(1.0) - 1.0).abs() < 1e-5, "{name}(1) = {}", f(1.0));
        }
    }

    /// A caller computing elapsed/duration WILL overshoot on a long
    /// frame. Clamping here rather than at every call site.
    #[test]
    fn curves_clamp_out_of_range_input() {
        let curves: [fn(f32) -> f32; 8] = [
            linear, in_quad, out_quad, in_out_quad,
            in_cubic, out_cubic, in_out_cubic, out_bounce,
        ];
        for f in curves {
            assert!((f(-5.0)).abs() < 1e-5);
            assert!((f(9.0) - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn curves_are_monotonic_except_the_overshooting_ones() {
        let curves: [(&str, fn(f32) -> f32); 7] = [
            ("linear", linear),
            ("in_quad", in_quad),
            ("out_quad", out_quad),
            ("in_out_quad", in_out_quad),
            ("in_cubic", in_cubic),
            ("out_cubic", out_cubic),
            ("in_out_cubic", in_out_cubic),
        ];
        for (name, f) in curves {
            let mut prev = f(0.0);
            for i in 1..=100 {
                let v = f(i as f32 / 100.0);
                assert!(v >= prev - 1e-6, "{name} went backwards at {i}: {prev} -> {v}");
                prev = v;
            }
        }
    }

    #[test]
    fn out_back_actually_overshoots() {
        let peak = (0..=100).map(|i| out_back(i as f32 / 100.0)).fold(f32::MIN, f32::max);
        assert!(peak > 1.0, "out_back should exceed 1.0, peaked at {peak}");
        assert!((out_back(1.0) - 1.0).abs() < 1e-5, "but must still land on 1.0");
    }

    #[test]
    fn ease_out_is_faster_than_linear_early() {
        // The defining property: an "out" curve covers more ground first.
        assert!(out_cubic(0.25) > linear(0.25));
        assert!(in_cubic(0.25) < linear(0.25));
    }

    #[test]
    fn lerp_hits_both_ends_and_the_middle() {
        assert_eq!(lerp(10.0, 20.0, 0.0), 10.0);
        assert_eq!(lerp(10.0, 20.0, 1.0), 20.0);
        assert_eq!(lerp(10.0, 20.0, 0.5), 15.0);
        // Clamped, not extrapolated.
        assert_eq!(lerp(10.0, 20.0, 3.0), 20.0);
        assert_eq!(lerp(10.0, 20.0, -1.0), 10.0);
    }

    #[test]
    fn inverse_lerp_round_trips() {
        let t = inverse_lerp(100.0, 200.0, 150.0);
        assert!((t - 0.5).abs() < 1e-6);
        assert!((lerp(100.0, 200.0, t) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn inverse_lerp_survives_a_zero_width_range() {
        assert_eq!(inverse_lerp(5.0, 5.0, 5.0), 0.0);
    }

    #[test]
    fn decay_starts_finished() {
        let d = Decay::new(0.5);
        assert!(!d.is_active());
        assert_eq!(d.progress(), 0.0);
    }

    #[test]
    fn decay_falls_from_one_to_zero() {
        let mut d = Decay::new(1.0);
        d.fire();
        assert!((d.progress() - 1.0).abs() < 1e-5);

        d.tick(0.5);
        assert!((d.progress() - 0.5).abs() < 1e-5);

        d.tick(0.5);
        assert_eq!(d.progress(), 0.0);
        assert!(!d.is_active());
    }

    #[test]
    fn decay_does_not_run_past_zero() {
        let mut d = Decay::new(0.2);
        d.fire();
        d.tick(100.0);
        assert_eq!(d.progress(), 0.0);
        // A second tick must not wrap it back around.
        d.tick(100.0);
        assert_eq!(d.progress(), 0.0);
    }

    #[test]
    fn decay_survives_a_zero_duration() {
        // A caller passing 0.0 must not produce a division by zero.
        let mut d = Decay::new(0.0);
        d.fire();
        d.tick(0.016);
        assert!(d.progress().is_finite());
    }
}
