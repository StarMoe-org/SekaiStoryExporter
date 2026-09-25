//! DOTween's shake tween as the game's DOTween build does it (JP 6.8.1 `DOTween.dll`,
//! `DOTween.Shake(getter, setter, duration, Vector3 strength, vibrato, randomness, ignoreZAxis,
//! vectorBased, fadeOut)` and `Vector3ArrayPlugin.EvaluateAndApply`):
//!
//! - `n = max(2, (int)(vibrato * duration))` segments;
//! - segment durations `duration / n`, or with `fadeOut` `(i + 1) / n * duration`, then all scaled
//!   by `duration / sum`;
//! - offsets: `ang = Random(0, 360)`; for every segment but the last, `ang = ang - 180 +
//!   Random(-r, r)` from the second on; the offset is `Vector3FromAngle(ang, magnitude)`, rotated
//!   about the up axis by `Random(-r, r)` unless `ignoreZAxis`; with `fadeOut` the magnitude drops
//!   by `strength / n` per segment; the last offset is zero;
//! - each segment eases on its own: `start[i] + Ease(segmentElapsed, segmentDuration) * change[i]`
//!   with the tween's ease (DOTween's default OutQuad).
//!
//! `UnityEngine.Random` cannot be replayed, so the offsets come from the bake's seeded RNG: the
//! shake has the game's amplitude, rhythm and decay, not its exact path.

use sse_core::det_math::{cosf, sinf};
use sse_core::rng::Rng;

pub struct Shake {
    start: u32,
    fps: f32,
    /// Segment end times in seconds, cumulative.
    ends: Vec<f32>,
    /// Offsets reached at each segment end (x, y), in the shaken object's local units.
    tos: Vec<[f32; 2]>,
}

impl Shake {
    /// `DOShakePosition` (`ignore_z = false`) / `DOShakeAnchorPos` (`ignore_z = true`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        rng: &mut Rng,
        start: u32,
        fps: f32,
        duration: f32,
        strength: f32,
        vibrato: i32,
        randomness: f32,
        ignore_z: bool,
        fade_out: bool,
    ) -> Self {
        let n = ((vibrato as f32 * duration) as i32).max(2) as usize;
        let decay = strength / n as f32;
        let mut durations: Vec<f32> = (0..n)
            .map(|i| {
                if fade_out {
                    (i + 1) as f32 / n as f32 * duration
                } else {
                    duration / n as f32
                }
            })
            .collect();
        let sum: f32 = durations.iter().sum();
        for d in &mut durations {
            *d *= duration / sum;
        }
        let mut ends = Vec::with_capacity(n);
        let mut acc = 0.0;
        for d in &durations {
            acc += d;
            ends.push(acc);
        }
        let mut magnitude = strength;
        let mut ang = rng.range_f32(0.0, 360.0);
        let mut tos = Vec::with_capacity(n);
        for i in 0..n {
            if i + 1 < n {
                if i > 0 {
                    ang = ang - 180.0 + rng.range_f32(-randomness, randomness);
                }
                let r = ang.to_radians();
                let (mut x, y) = (magnitude * cosf(r), magnitude * sinf(r));
                if !ignore_z {
                    // `Quaternion.AngleAxis(q, Vector3.up) * v`: x turns into z, which the
                    // orthographic scenario camera does not see.
                    x *= cosf(rng.range_f32(-randomness, randomness).to_radians());
                }
                tos.push([x, y]);
                if fade_out {
                    magnitude -= decay;
                }
            } else {
                tos.push([0.0, 0.0]);
            }
        }
        Shake {
            start,
            fps,
            ends,
            tos,
        }
    }

    /// The offset at `frame`; zero before the start and after the end.
    pub fn at(&self, frame: u32) -> [f32; 2] {
        if frame < self.start {
            return [0.0, 0.0];
        }
        let t = (frame - self.start) as f32 / self.fps;
        let Some(i) = self.ends.iter().position(|&e| e >= t) else {
            return [0.0, 0.0];
        };
        let seg_start = if i == 0 { 0.0 } else { self.ends[i - 1] };
        let seg = self.ends[i] - seg_start;
        let mut p = if seg > 0.0 {
            (t - seg_start) / seg
        } else {
            1.0
        };
        p = -p * (p - 2.0); // OutQuad
        let from = if i == 0 { [0.0, 0.0] } else { self.tos[i - 1] };
        let to = self.tos[i];
        [
            from[0] + (to[0] - from[0]) * p,
            from[1] + (to[1] - from[1]) * p,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_decay_and_end_at_rest() {
        let mut rng = Rng::new(1);
        // ScreenShake: DOShakePosition(duration, 10, 16, 90, false, fadeOut: true)
        let s = Shake::new(&mut rng, 100, 60.0, 0.5, 10.0, 16, 90.0, false, true);
        assert_eq!(s.tos.len(), 8);
        assert!((s.ends.last().unwrap() - 0.5).abs() < 1e-5);
        let first = s.tos[0][0].hypot(s.tos[0][1]);
        assert!(first <= 10.0 + 1e-4);
        assert!(s.tos[6][0].hypot(s.tos[6][1]) < first);
        assert_eq!(s.at(99), [0.0, 0.0]);
        assert_eq!(s.at(100 + 30), [0.0, 0.0]);
        // fade-out segments get longer
        assert!(s.ends[1] - s.ends[0] > s.ends[0]);
    }
}
