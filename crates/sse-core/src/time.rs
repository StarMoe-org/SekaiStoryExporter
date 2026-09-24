//! Time base (decisions Q33 / Q34, `docs/spec/coordinate-systems.md`).
//!
//! The simulation frame is the authoritative time unit. Seconds are derived from frames,
//! never the other way round. Accumulation follows Unity: `elapsed += Time.deltaTime` in f32.

/// A frame of the scheduler simulation, stepped at the game's frame rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SimFrame(pub u32);

/// A frame of the exported video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FrameNo(pub u32);

/// Fixed-step time base. The story runs at `consts::STORY_TARGET_FRAME_RATE` (decision Q34).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeBase {
    fps: u32,
    delta: f32,
}

impl TimeBase {
    /// The game's story frame rate.
    pub fn story() -> Self {
        Self::new(crate::consts::STORY_TARGET_FRAME_RATE)
    }

    pub fn new(fps: u32) -> Self {
        assert!(fps > 0, "fps must be positive");
        Self {
            fps,
            delta: 1.0 / fps as f32,
        }
    }

    pub fn fps(self) -> u32 {
        self.fps
    }

    /// `Time.deltaTime` for one fixed step.
    pub fn delta(self) -> f32 {
        self.delta
    }

    /// Derived: seconds at the start of `frame`, in f64 (for audio placement only).
    pub fn seconds(self, frame: SimFrame) -> f64 {
        f64::from(frame.0) / f64::from(self.fps)
    }

    /// Number of frames a Unity-style `while (elapsed < duration) { elapsed += dt; yield; }`
    /// loop waits. `duration <= 0` waits zero frames.
    pub fn frames_for(self, duration: f32) -> u32 {
        let mut elapsed = 0.0_f32;
        let mut frames = 0;
        while elapsed < duration {
            elapsed += self.delta;
            frames += 1;
        }
        frames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unity_wait_rounds_up_to_whole_frames() {
        let tb = TimeBase::new(60);
        assert_eq!(tb.frames_for(0.0), 0);
        assert_eq!(tb.frames_for(-1.0), 0);
        assert_eq!(tb.frames_for(0.5), 30);
        assert_eq!(tb.frames_for(0.51), 31);
    }
}
