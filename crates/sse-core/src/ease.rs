//! DOTween `Ease` curves (DOTween's `EaseManager.Evaluate`, overshoot 1.70158, period 0),
//! for tweens whose ease comes from scenario data. Deterministic: no transcendental std calls.

use crate::det_math::{cosf, sinf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ease {
    Linear,
    InSine,
    OutSine,
    InOutSine,
    InQuad,
    OutQuad,
    InOutQuad,
    InCubic,
    OutCubic,
    InOutCubic,
    InQuart,
    OutQuart,
    InOutQuart,
    InQuint,
    OutQuint,
    InOutQuint,
    InCirc,
    OutCirc,
    InOutCirc,
    InBack,
    OutBack,
    InOutBack,
}

impl Ease {
    /// `Enum.TryParse<Ease>(name, ignoreCase: true)`; `None` for names this module lacks.
    pub fn parse(name: &str) -> Option<Self> {
        use Ease::*;
        Some(match name.trim().to_ascii_lowercase().as_str() {
            "linear" => Linear,
            "insine" => InSine,
            "outsine" => OutSine,
            "inoutsine" => InOutSine,
            "inquad" => InQuad,
            "outquad" => OutQuad,
            "inoutquad" => InOutQuad,
            "incubic" => InCubic,
            "outcubic" => OutCubic,
            "inoutcubic" => InOutCubic,
            "inquart" => InQuart,
            "outquart" => OutQuart,
            "inoutquart" => InOutQuart,
            "inquint" => InQuint,
            "outquint" => OutQuint,
            "inoutquint" => InOutQuint,
            "incirc" => InCirc,
            "outcirc" => OutCirc,
            "inoutcirc" => InOutCirc,
            "inback" => InBack,
            "outback" => OutBack,
            "inoutback" => InOutBack,
            _ => return None,
        })
    }

    /// Eased progress for `t` in [0, 1].
    pub fn at(self, t: f32) -> f32 {
        use Ease::*;
        const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
        const S: f32 = 1.701_58;
        let t = t.clamp(0.0, 1.0);
        let pow = |x: f32, n: i32| x.powi(n);
        let in_out = |f: &dyn Fn(f32) -> f32| {
            if t < 0.5 {
                f(t * 2.0) * 0.5
            } else {
                1.0 - f((1.0 - t) * 2.0) * 0.5
            }
        };
        match self {
            Linear => t,
            InSine => 1.0 - cosf(t * HALF_PI),
            OutSine => sinf(t * HALF_PI),
            InOutSine => -0.5 * (cosf(std::f32::consts::PI * t) - 1.0),
            InQuad => t * t,
            OutQuad => -t * (t - 2.0),
            InOutQuad => in_out(&|x| x * x),
            InCubic => pow(t, 3),
            OutCubic => pow(t - 1.0, 3) + 1.0,
            InOutCubic => in_out(&|x| pow(x, 3)),
            InQuart => pow(t, 4),
            OutQuart => -(pow(t - 1.0, 4) - 1.0),
            InOutQuart => in_out(&|x| pow(x, 4)),
            InQuint => pow(t, 5),
            OutQuint => pow(t - 1.0, 5) + 1.0,
            InOutQuint => in_out(&|x| pow(x, 5)),
            InCirc => -((1.0 - t * t).max(0.0).sqrt() - 1.0),
            OutCirc => (1.0 - (t - 1.0) * (t - 1.0)).max(0.0).sqrt(),
            InOutCirc => in_out(&|x| 1.0 - (1.0 - x * x).max(0.0).sqrt()),
            InBack => t * t * ((S + 1.0) * t - S),
            OutBack => {
                let u = t - 1.0;
                u * u * ((S + 1.0) * u + S) + 1.0
            }
            InOutBack => {
                let s = S * 1.525;
                in_out(&|x| x * x * ((s + 1.0) * x - s))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_start_at_zero_and_end_at_one() {
        for e in [
            Ease::Linear,
            Ease::OutQuad,
            Ease::InOutCubic,
            Ease::OutBack,
            Ease::InSine,
        ] {
            assert!(e.at(0.0).abs() < 1e-5, "{e:?}");
            assert!((e.at(1.0) - 1.0).abs() < 1e-5, "{e:?}");
        }
        assert_eq!(Ease::parse("linear"), Some(Ease::Linear));
        assert!((Ease::InOutQuad.at(0.25) - 0.125).abs() < 1e-6);
    }
}
