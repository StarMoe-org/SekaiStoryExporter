//! Motion playback with the game's semantics (ADR-0007).
//!
//! `CP.PlayableAnimator` owns two layers (`Live2DModel.MotionLayer { Body = 0, Face = 1 }`).
//! Each layer is an `AnimationMixerPlayable` whose inputs are crossfaded by `PlayableBlender`
//! linearly: `t = clamp01(elapsed / blendTime); w = a + (b − a)·t`. A property an input does
//! not animate contributes the default value. The face layer overrides the body layer for
//! every property it animates.

use std::collections::BTreeMap;
use std::sync::Arc;

use sse_assets::SseMotion;
use sse_assets::motion::{ATTR_VALUE, CurveData};

pub const LAYER_BODY: usize = 0;
pub const LAYER_FACE: usize = 1;

/// A clip bound to one model's parameter table.
#[derive(Debug)]
pub struct BoundClip {
    pub name: String,
    pub length: f32,
    pub looping: bool,
    start_time: f32,
    /// (parameter index, curve)
    curves: Vec<(usize, CurveData)>,
    pub events: Vec<(f32, String, String)>,
}

impl BoundClip {
    pub fn bind(motion: &SseMotion, parameter_ids: &[String]) -> Self {
        let hashes: BTreeMap<u32, usize> = parameter_ids
            .iter()
            .enumerate()
            .map(|(i, id)| (crc32fast::hash(format!("Parameters/{id}").as_bytes()), i))
            .collect();
        let curves = motion
            .curves
            .iter()
            .filter(|c| c.binding.attr_hash == ATTR_VALUE)
            .filter_map(|c| {
                hashes
                    .get(&c.binding.path_hash)
                    .map(|&i| (i, c.data.clone()))
            })
            .collect();
        Self {
            name: motion.name.clone(),
            length: motion.stop_time - motion.start_time,
            looping: motion.loop_time,
            start_time: motion.start_time,
            curves,
            events: motion
                .events
                .iter()
                .map(|e| (e.time, e.function.clone(), e.data.clone()))
                .collect(),
        }
    }

    /// Clip-local time → sampling time (non-looping clips hold their last pose).
    fn sample_time(&self, t: f32) -> f32 {
        let local = if self.looping && self.length > 0.0 {
            t % self.length
        } else {
            t.min(self.length)
        };
        self.start_time + local
    }
}

#[derive(Debug, Clone)]
struct Input {
    clip: Arc<BoundClip>,
    time: f32,
    weight: f32,
    /// Weight at the start of the current blend (`a`) and target (`b`).
    from: f32,
    to: f32,
}

#[derive(Debug, Clone, Default)]
struct Layer {
    inputs: Vec<Input>,
    blend_elapsed: f32,
    blend_time: f32,
}

/// An animation event that fired during an update.
#[derive(Debug, Clone)]
pub struct FiredEvent {
    pub function: String,
    pub data: String,
}

#[derive(Debug, Clone, Default)]
pub struct Animator {
    layers: [Layer; 2],
}

impl Animator {
    pub fn current(&self, layer: usize) -> Option<&str> {
        self.layers[layer]
            .inputs
            .last()
            .map(|i| i.clip.name.as_str())
    }

    /// `PlayableAnimator.ChangeAnimation(layer, clip, blend, rewind: true)`.
    pub fn change(&mut self, layer: usize, clip: Arc<BoundClip>, blend: f32) {
        let l = &mut self.layers[layer];
        if blend <= 0.0 || l.inputs.is_empty() {
            l.inputs.clear();
            l.inputs.push(Input {
                clip,
                time: 0.0,
                weight: 1.0,
                from: 1.0,
                to: 1.0,
            });
            l.blend_time = 0.0;
            l.blend_elapsed = 0.0;
            return;
        }
        for i in &mut l.inputs {
            i.from = i.weight;
            i.to = 0.0;
        }
        l.inputs.push(Input {
            clip,
            time: 0.0,
            weight: 0.0,
            from: 0.0,
            to: 1.0,
        });
        l.blend_time = blend;
        l.blend_elapsed = 0.0;
    }

    /// Advances clip times and blend weights by `delta`; returns the events crossed by the
    /// newest input of each layer.
    pub fn advance(&mut self, delta: f32) -> Vec<FiredEvent> {
        let mut fired = Vec::new();
        for l in &mut self.layers {
            if let Some(newest) = l.inputs.last() {
                let (t0, t1) = (newest.time, newest.time + delta);
                let c = &newest.clip;
                for (et, function, data) in &c.events {
                    let crossed = if c.looping && c.length > 0.0 {
                        let a = t0 % c.length;
                        let b = a + delta;
                        (*et >= a && *et < b) || (b > c.length && *et < b - c.length)
                    } else {
                        *et >= t0 && *et < t1
                    };
                    if crossed {
                        fired.push(FiredEvent {
                            function: function.clone(),
                            data: data.clone(),
                        });
                    }
                }
            }
            for i in &mut l.inputs {
                i.time += delta;
            }
            if l.blend_time > 0.0 {
                l.blend_elapsed += delta;
                let t = (l.blend_elapsed / l.blend_time).clamp(0.0, 1.0);
                for i in &mut l.inputs {
                    i.weight = i.from + (i.to - i.from) * t;
                }
                if t >= 1.0 {
                    l.inputs.retain(|i| i.to > 0.0);
                    l.blend_time = 0.0;
                }
            }
        }
        fired
    }

    /// Evaluates both layers into `values` (which must hold the default values on entry).
    pub fn evaluate(&self, values: &mut [f32], defaults: &[f32]) {
        let n = values.len();
        for l in &self.layers {
            if l.inputs.is_empty() {
                continue;
            }
            let mut animated = vec![false; n];
            for i in &l.inputs {
                for (p, _) in &i.clip.curves {
                    animated[*p] = true;
                }
            }
            let mut acc = vec![0.0_f32; n];
            for i in &l.inputs {
                let mut own = vec![false; n];
                let t = i.clip.sample_time(i.time);
                for (p, curve) in &i.clip.curves {
                    acc[*p] += i.weight * curve.evaluate(t);
                    own[*p] = true;
                }
                for p in 0..n {
                    if animated[p] && !own[p] {
                        acc[p] += i.weight * defaults[p];
                    }
                }
            }
            for p in 0..n {
                if animated[p] {
                    values[p] = acc[p];
                }
            }
        }
    }
}
