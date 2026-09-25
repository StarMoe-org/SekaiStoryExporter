//! Lip sync (ADR-0007).

use sse_assets::Library;
use sse_core::{TimeBase, consts, det_math};
use sse_ir::{AudioRef, CharacterId};
use sse_timeline::TalkTiming;

use crate::BakeError;

pub struct MonoPcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

pub fn load_mono(lib: &Library, a: &AudioRef) -> Result<MonoPcm, BakeError> {
    let file = a
        .files
        .first()
        .ok_or_else(|| BakeError::Live2d(a.cue.clone(), "voice cue has no waveform".into()))?;
    let pcm = lib.load_wav(&file.0)?;
    let ch = usize::from(pcm.channels.max(1));
    let samples = pcm
        .samples
        .chunks(ch)
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    Ok(MonoPcm {
        rate: pcm.sample_rate,
        samples,
    })
}

pub enum Lip {
    None,
    Voice {
        pcm: MonoPcm,
        start: u32,
        tvs: f32,
        pow_k: f32,
        prev: f32,
    },
    Text {
        timing: TalkTiming,
        end: Option<u32>,
    },
}

impl Lip {
    /// `SetLipSyncK`.
    pub fn voice(pcm: MonoPcm, start: u32, character: CharacterId, talk_tvs: f32) -> Self {
        let (tvs, pow_k) = if talk_tvs > 0.0 {
            (talk_tvs, talk_tvs * consts::LIPSYNC_DEFAULT_POW_K)
        } else if consts::LIPSYNC_LOUD_CHARACTERS.contains(&character) {
            (
                consts::LIPSYNC_LOUD_TARGET_VALUE_SCALE,
                consts::LIPSYNC_DEFAULT_POW_K,
            )
        } else {
            (
                consts::LIPSYNC_DEFAULT_TARGET_VALUE_SCALE,
                consts::LIPSYNC_DEFAULT_POW_K,
            )
        };
        Lip::Voice {
            pcm,
            start,
            tvs,
            pow_k,
            prev: 0.0,
        }
    }

    pub fn text(timing: TalkTiming) -> Self {
        Lip::Text { timing, end: None }
    }

    pub fn end_text(&mut self, frame: u32) {
        if let Lip::Text { end, .. } = self {
            *end = Some(frame);
        }
    }

    /// Mouth value for this frame, or `None` when lip sync does not write the parameter.
    pub fn value(&mut self, frame: u32, tb: TimeBase) -> Option<f32> {
        match self {
            Lip::None => None,
            Lip::Voice {
                pcm,
                start,
                tvs,
                pow_k,
                prev,
            } => {
                if frame < *start {
                    return None;
                }
                // RMS over the last frame's worth of samples (CriAtomExOutputAnalyzer window
                // not reversed).
                let t = tb.seconds(sse_core::SimFrame(frame - *start));
                let end = (t * f64::from(pcm.rate)) as usize;
                let len = (f64::from(pcm.rate) / f64::from(tb.fps())) as usize;
                let begin = end.saturating_sub(len);
                let rms = if begin >= pcm.samples.len() {
                    0.0
                } else {
                    let s = &pcm.samples[begin..end.min(pcm.samples.len())];
                    let sum: f32 = s.iter().map(|x| x * x).sum();
                    det_math::sqrtf(sum / s.len().max(1) as f32)
                };
                let a = rms * *tvs;
                let mut target = a * det_math::powf(a + 1.0, *pow_k);
                target = if target < 0.06 {
                    0.0
                } else {
                    target.clamp(0.2, 1.0)
                };
                // `Live2DVoice.UpdateParam`: the weights depend on the *current*
                // value (== prev, both are written with the result), not on the target
                let (kt, kp) = if *prev < 0.1 {
                    (0.6, 0.4)
                } else if (target - *prev).abs() < 0.1 {
                    (0.2, 0.8)
                } else {
                    (0.25, 0.75)
                };
                let value = target * kt + *prev * kp;
                *prev = value;
                if begin >= pcm.samples.len() && value < 0.001 {
                    *self = Lip::None;
                    return Some(0.0);
                }
                Some(value)
            }
            Lip::Text { timing, end } => {
                if end.is_some_and(|e| frame >= e) || frame >= timing.typing_end {
                    *self = Lip::None;
                    return Some(0.0);
                }
                if frame < timing.typing_start {
                    return None;
                }
                let half = timing.frames_per_half.max(1);
                let idx = ((frame - timing.typing_start) / half) as usize;
                Some(consts::LIP_LEVELS[idx % consts::LIP_LEVELS.len()])
            }
        }
    }
}
