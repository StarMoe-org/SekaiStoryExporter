//! Interactive (block sequence) BGM: `SoundManager.PlayBGM` of a cue whose ACB reference is a
//! block sequence, `SetFirstBGMBlockIndex(0)` before it (`SnippetActionSound` default case),
//! and `SetBgmBlockIndex` (PlayMode 6).
//!
//! JP 6.8.1 `ScenarioPlayer.SetBgmBlockIndex`: while the playback's current block index is odd
//! the request waits (`UniTask.WaitUntil(GetCurrentBlockIndex() % 2 == 0)`, cancelled by a
//! newer request), then `CriAtomExPlayback.SetBlockIndex(target)`.
//!
//! The block transition itself happens inside the CRI runtime (native, not reversed here); the
//! ACB data gives each block's length, playback type (1: loop until told otherwise), loop
//! count, and transition timing (1 with value N: at the next of N equal parts of the block,
//! otherwise at the block's end). A block's tracks each play one waveform from the block's
//! start; the sequence's tracks are the vertical layers, each with its local AISAC.

use std::sync::Arc;

use sse_assets::acb::CueStructure;
use sse_params::{AisacCurve, AudioCue, AudioKind};

pub struct Sequencer {
    cue: Arc<CueStructure>,
    block: usize,
    /// Start of the current block play, seconds.
    start: f64,
    plays: u32,
    /// `SetBlockIndex` target and when it was called (seconds).
    pending: Option<(usize, f64)>,
    /// A request waiting for an even block.
    deferred: Option<usize>,
    volume: f32,
    fade_in: u32,
    fps: f64,
    /// `AudioCue` indices of the current block play.
    pub cues: Vec<usize>,
    pub ended: bool,
}

impl Sequencer {
    pub fn start(
        cue: Arc<CueStructure>,
        at: f64,
        volume: f32,
        fade_in: u32,
        fps: f64,
        audio: &mut Vec<AudioCue>,
    ) -> Self {
        let mut s = Self {
            cue,
            block: 0,
            start: at,
            plays: 0,
            pending: None,
            deferred: None,
            volume,
            fade_in,
            fps,
            cues: Vec::new(),
            ended: false,
        };
        s.emit(audio);
        s
    }

    fn frame(&self, t: f64) -> u32 {
        (t * self.fps).round().max(0.0) as u32
    }

    fn emit(&mut self, audio: &mut Vec<AudioCue>) {
        self.cues.clear();
        let Some(b) = self.cue.blocks.get(self.block) else {
            self.ended = true;
            return;
        };
        let (start, stop) = (self.frame(self.start), self.frame(self.start + b.length));
        for (layer, wave) in b.waves.iter().enumerate() {
            let Some(wave) = wave else { continue };
            let aisac = self
                .cue
                .layers
                .get(layer)
                .cloned()
                .flatten()
                .map(|a| AisacCurve {
                    points: a.points,
                    default: a.default,
                });
            audio.push(AudioCue {
                files: vec![wave.clone()],
                start_frame: start,
                stop_frame: Some(stop.max(start + 1)),
                looping: false,
                volume: self.volume,
                fade_in: self.fade_in,
                fade_out: 0,
                kind: AudioKind::Bgm,
                aisac,
            });
            self.cues.push(audio.len() - 1);
        }
        self.fade_in = 0;
    }

    /// When the current block play ends or a pending transition fires, in seconds.
    fn boundary(&self) -> f64 {
        let Some(b) = self.cue.blocks.get(self.block) else {
            return f64::INFINITY;
        };
        let end = self.start + b.length;
        match (self.pending, b.grid) {
            (Some((_, at)), Some(n)) => {
                let part = b.length / f64::from(n);
                let k = ((at - self.start) / part).ceil().max(1.0);
                (self.start + k * part).min(end)
            }
            _ => end,
        }
    }

    /// Advances the sequence up to `t` seconds, emitting block plays.
    pub fn advance(&mut self, t: f64, audio: &mut Vec<AudioCue>) {
        while !self.ended && t >= self.boundary() {
            let at = self.boundary();
            let fired = self.pending.take();
            if let Some((target, _)) = fired {
                // cut the current play at the transition point
                let stop = self.frame(at);
                for &i in &self.cues {
                    if audio[i].stop_frame.is_none_or(|s| s > stop) {
                        audio[i].stop_frame = Some(stop.max(audio[i].start_frame + 1));
                    }
                }
                self.block = target;
                self.plays = 0;
            } else {
                let b = &self.cue.blocks[self.block];
                if b.looping || self.plays < b.repeats {
                    self.plays += 1;
                } else {
                    self.block += 1;
                    self.plays = 0;
                }
            }
            self.start = at;
            // a deferred request goes out once the block index is even
            if self.block.is_multiple_of(2)
                && let Some(target) = self.deferred.take()
            {
                self.pending = Some((target, at));
            }
            self.emit(audio);
        }
    }

    /// `ScenarioPlayer.SetBgmBlockIndex(target)` at `t` seconds.
    pub fn request(&mut self, target: usize, t: f64, audio: &mut Vec<AudioCue>) {
        self.advance(t, audio);
        if target >= self.cue.blocks.len() {
            return;
        }
        if !self.block.is_multiple_of(2) {
            self.deferred = Some(target);
        } else {
            self.deferred = None;
            self.pending = Some((target, t));
        }
    }

    /// Stops every playing layer at `frame` with a fade of `fade` frames.
    pub fn stop(&mut self, frame: u32, fade: u32, audio: &mut [AudioCue]) {
        for &i in &self.cues {
            let c = &mut audio[i];
            if c.stop_frame.is_none_or(|s| s > frame + fade) {
                c.stop_frame = Some(frame + fade);
                c.fade_out = fade;
            }
        }
        self.ended = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sse_assets::acb::Block;

    fn block(length: f64, looping: bool, grid: Option<u32>, wave: &str) -> Block {
        Block {
            length,
            looping,
            repeats: 0,
            grid,
            waves: vec![Some(wave.into())],
        }
    }

    #[test]
    fn blocks_loop_and_switch_on_the_grid_after_an_even_block() {
        let cue = Arc::new(CueStructure {
            layers: vec![None],
            blocks: vec![
                block(1.0, false, None, "a"),
                block(1.0, false, None, "b"),
                block(2.0, true, Some(4), "c"),
                block(1.0, false, None, "d"),
                block(2.0, true, None, "e"),
            ],
            waves: Vec::new(),
        });
        let mut audio = Vec::new();
        let mut s = Sequencer::start(cue, 0.0, 1.0, 0, 10.0, &mut audio);
        // a request during block 1 (odd) waits for block 2
        s.request(4, 1.5, &mut audio);
        s.advance(6.0, &mut audio);
        let starts: Vec<(String, u32, Option<u32>)> = audio
            .iter()
            .map(|c| (c.files[0].clone(), c.start_frame, c.stop_frame))
            .collect();
        // a 0–1, b 1–2, c from 2; the request fires at the first quarter boundary of c (2.5)
        assert_eq!(starts[0], ("a".into(), 0, Some(10)));
        assert_eq!(starts[1], ("b".into(), 10, Some(20)));
        assert_eq!(starts[2], ("c".into(), 20, Some(25)));
        assert_eq!(starts[3], ("e".into(), 25, Some(45)));
        assert_eq!(starts[4], ("e".into(), 45, Some(65)));
    }
}
