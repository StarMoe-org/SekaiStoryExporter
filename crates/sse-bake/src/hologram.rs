//! `SnippetActionSpecialEffect` case 22 (`SpecialEffectChangeCharacterShader`) state on one
//! model view (JP 6.8.1 / CN 6.4.0 `ScenarioPlayer.AttachModelEffect(type, id, intensity)`):
//!
//! - "hologram" (`ModelEffectType` 5) and "monitor" (6) add a `Live2DHologramController` to
//!   the model's `RawImage`; "hologram" also instantiates the prefab named after the bundle
//!   (`StringValSub`) under the model view (`AttachModelScenarioEffect`). A repeat of the
//!   current type does nothing.
//! - "none" runs `DetachModelEffect` on every loaded character and destroys this character's
//!   attached prefabs (`DetachEffectAll` → `ForceStop`).
//! - An attached prefab is active only while the model view is shown: it is deactivated when
//!   attached to a hidden character and when a hide fade completes, and re-activated (so its
//!   particle systems restart) by `AppearCharacter`.

use sse_core::consts;
use sse_core::rng::Rng;
use sse_params::HologramState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Hologram,
    Monitor,
}

/// `Live2DHologramController` with its material instance.
#[derive(Debug, Clone)]
pub struct Controller {
    pub kind: Kind,
    line: f32,
    alpha: f32,
    countdown: f32,
    /// Frame `AddComponent` ran (`Awake` → `Setup`); `Update` starts the next frame.
    born: u32,
    last: u32,
    rng: Rng,
}

impl Controller {
    pub fn new(kind: Kind, frame: u32, seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let [lo, hi] = consts::HOLOGRAM_FIRST_COUNTDOWN;
        let countdown = rng.range_f32(lo, hi);
        Self {
            kind,
            line: consts::HOLOGRAM_LINE,
            alpha: consts::HOLOGRAM_SUB_ALPHA,
            countdown,
            born: frame,
            last: frame,
            rng,
        }
    }

    /// Runs `Update` for every frame after the last one up to `frame`.
    pub fn advance(&mut self, frame: u32, dt: f32) {
        while self.last < frame {
            self.last += 1;
            if self.last > self.born {
                self.update(dt);
            }
        }
    }

    fn update(&mut self, dt: f32) {
        let (chance, of) = consts::HOLOGRAM_REROLL_CHANCE;
        if self.rng.range_i32(0, of) < chance || self.countdown <= 0.0 {
            let [a, b] = consts::HOLOGRAM_LINE_RANGE;
            self.line = self.rng.range_f32(a, b);
            let [a, b] = consts::HOLOGRAM_ALPHA_RANGE;
            self.alpha = self.rng.range_f32(a, b);
            let [a, b] = consts::HOLOGRAM_COUNTDOWN_RANGE;
            self.countdown = self.rng.range_f32(a, b);
        }
        self.countdown -= dt;
    }

    pub fn state(&self, time: f32) -> HologramState {
        HologramState {
            line: self.line,
            alpha: self.alpha,
            time,
        }
    }
}

/// A prefab attached to the model view.
#[derive(Debug, Clone)]
pub struct Attached {
    pub bundle: String,
    pub name: String,
    /// Frame the prefab was last activated; `None` while inactive.
    pub since: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_starts_the_frame_after_setup_and_rerolls_within_range() {
        let mut c = Controller::new(Kind::Hologram, 10, 7);
        c.advance(10, 1.0 / 60.0);
        assert_eq!(
            (c.line, c.alpha),
            (consts::HOLOGRAM_LINE, consts::HOLOGRAM_SUB_ALPHA)
        );
        c.advance(100, 1.0 / 60.0);
        assert!((0.6..=0.8).contains(&c.line));
        assert!((0.85..=0.9).contains(&c.alpha));
    }
}
