//! Sounds of scenario effect prefabs: animation events handled by `CommandAnimator` (JP 6.8.1):
//!
//! - `OnPlayEnvironmentSE(cue)`: `SoundManager.PlayEnvironmentSE(cue, loopSeFadeTime,
//!   loopSeVolume, player 2)` — a looping SE on the environment player, faded in;
//! - `OnStopEnvironmentSE(cue)`: `StopEnvironmentSE(loopSeFadeTime)` — that player, faded out;
//! - `OnSetEnvironmentSEFadeTime(f)` / `OnSetEnvironmentSEVolume(f)` (defaults 0.25 s / 1);
//! - `OnPlaySE(cue)`: `PlaySEOneShot`.
//!
//! The cues come from the effect bundle's own ACB (`SoundBundleBuildData`). Loop SEs still
//! registered when the effect finishes are stopped by `FinishLoopSE`; the finish (all
//! particles gone) is a renderer-side matter, so an unstopped loop ends with the stop clip.
//!
//! The Animator walk mirrors `sse-render`'s effect instance: the default state, exit-time
//! transitions (switching at the exit time), and the "Stop" state on `Stop()`.

use std::collections::BTreeMap;

use serde_json::Value;
use sse_assets::{Library, SseMotion};

#[derive(Debug, Clone, PartialEq)]
pub enum SoundEvent {
    PlayEnv(String),
    StopEnv,
    FadeTime(f32),
    Volume(f32),
    PlaySe(String),
}

struct State {
    name_hash: u32,
    clip: Option<usize>,
    speed: f32,
    exit: Option<(usize, f32)>,
}

struct Clip {
    length: f32,
    looping: bool,
    start: f32,
    events: Vec<(f32, SoundEvent)>,
}

/// The effect's sound events on its own timeline.
pub struct EffectSounds {
    states: Vec<State>,
    clips: Vec<Clip>,
    default_state: usize,
    /// cue name → library waveform files
    pub cues: BTreeMap<String, Vec<String>>,
}

fn pid(v: &Value) -> i64 {
    v["m_PathID"].as_i64().unwrap_or(0)
}

impl EffectSounds {
    pub fn load(lib: &Library, bundle: &str) -> Option<Self> {
        let dir = lib.path(bundle);
        let obj = lib.object_map(bundle).ok()?;
        let by = |id: i64| obj.get(&id.to_string());
        // cues of the bundle's ACBs
        let mut cues = BTreeMap::new();
        for entry in sse_core::fs::read_dir_sorted(&dir).ok()? {
            let name = entry.to_string_lossy().into_owned();
            if !name.ends_with(".cues.json") {
                continue;
            }
            let index: Value = sse_assets::read_json(&entry).ok()?;
            let waves = &index["waveforms"];
            for c in index["cues"].as_array().into_iter().flatten() {
                let files: Vec<String> = c["waveforms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|w| waves[w.as_str()?]["file"].as_str())
                    .map(|f| format!("{bundle}/{f}"))
                    .collect();
                if let Some(n) = c["name"].as_str() {
                    cues.insert(n.to_owned(), files);
                }
            }
        }
        if cues.is_empty() {
            return None;
        }
        // ScenarioEffector.mainAnimator → Animator.m_Controller
        let script_class = |mb: &Value| -> Option<String> {
            let s = by(pid(&mb["tree"]["m_Script"]))?;
            Some(s["tree"]["m_ClassName"].as_str()?.to_owned())
        };
        let effector = obj.values().find(|o| {
            o["classId"].as_i64() == Some(114)
                && script_class(o).as_deref() == Some("ScenarioEffector")
        })?;
        let animator = by(pid(&effector["tree"]["mainAnimator"]))?;
        let ctrl = &by(pid(&animator["tree"]["m_Controller"]))?["tree"];
        let mut motions: BTreeMap<i64, SseMotion> = BTreeMap::new();
        for p in sse_core::fs::read_dir_sorted(&dir).ok()? {
            if p.to_string_lossy().ends_with(".sse-motion.json")
                && let Ok(m) = sse_assets::read_json::<SseMotion>(&p)
            {
                motions.insert(m.source.path_id, m);
            }
        }
        let clips: Vec<Clip> = ctrl["m_AnimationClips"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| match motions.get(&pid(c)) {
                Some(m) => Clip {
                    length: m.stop_time - m.start_time,
                    looping: m.loop_time,
                    start: m.start_time,
                    events: m
                        .events
                        .iter()
                        .filter_map(|e| {
                            let ev = match e.function.as_str() {
                                "OnPlayEnvironmentSE" => SoundEvent::PlayEnv(e.data.clone()),
                                "OnStopEnvironmentSE" => SoundEvent::StopEnv,
                                "OnSetEnvironmentSEFadeTime" => {
                                    SoundEvent::FadeTime(e.float_parameter)
                                }
                                "OnSetEnvironmentSEVolume" => SoundEvent::Volume(e.float_parameter),
                                "OnPlaySE" => SoundEvent::PlaySe(e.data.clone()),
                                _ => return None,
                            };
                            Some((e.time, ev))
                        })
                        .collect(),
                },
                None => Clip {
                    length: 0.0,
                    looping: false,
                    start: 0.0,
                    events: Vec::new(),
                },
            })
            .collect();
        let sm = &ctrl["m_Controller"]["m_StateMachineArray"][0]["data"];
        let f = |v: &Value| v.as_f64().unwrap_or(0.0) as f32;
        let states = sm["m_StateConstantArray"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|s| {
                let d = &s["data"];
                State {
                    name_hash: d["m_NameID"].as_u64().unwrap_or(0) as u32,
                    clip: d["m_BlendTreeConstantArray"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|bt| bt["data"]["m_NodeArray"].as_array()?.first().cloned())
                        .and_then(|n| n["data"]["m_ClipID"].as_u64())
                        .map(|i| i as usize)
                        .filter(|&i| i < clips.len()),
                    speed: d.get("m_Speed").map_or(1.0, f),
                    exit: d["m_TransitionConstantArray"].as_array().and_then(|ts| {
                        ts.iter().find_map(|t| {
                            let t = &t["data"];
                            let has_exit = t["m_HasExitTime"].as_bool().unwrap_or(false)
                                || t["m_HasExitTime"].as_i64() == Some(1);
                            let conds = t["m_ConditionConstantArray"]
                                .as_array()
                                .map_or(0, |a| a.len());
                            (has_exit && conds == 0).then(|| {
                                (
                                    t["m_DestinationState"].as_u64().unwrap_or(0) as usize,
                                    f(&t["m_ExitTime"]),
                                )
                            })
                        })
                    }),
                }
            })
            .collect();
        Some(Self {
            states,
            clips,
            default_state: sm["m_DefaultState"].as_u64().unwrap_or(0) as usize,
            cues,
        })
    }

    /// Events from `state` played for `duration` seconds (following exit transitions), as
    /// (seconds from the state's start, event).
    fn walk(&self, mut state: usize, duration: f32) -> Vec<(f32, SoundEvent)> {
        let mut out = Vec::new();
        let mut at = 0.0_f32;
        for _ in 0..64 {
            let Some(s) = self.states.get(state) else {
                break;
            };
            let clip = s.clip.map(|c| &self.clips[c]);
            let len = clip.map_or(0.0, |c| c.length).max(1e-4) / s.speed.max(1e-4);
            let stay = match s.exit {
                Some((_, exit)) => len * exit,
                None => f32::INFINITY,
            }
            .min(duration - at);
            if let Some(c) = clip {
                let plays = if c.looping {
                    (stay / len).ceil().max(1.0) as u32
                } else {
                    1
                };
                for k in 0..plays {
                    for (t, ev) in &c.events {
                        let local = k as f32 * len + (t - c.start) / s.speed.max(1e-4);
                        if local < stay || (local == 0.0 && stay <= 0.0) {
                            out.push((at + local, ev.clone()));
                        }
                    }
                }
            }
            at += stay;
            match s.exit {
                Some((dest, _)) if at < duration => state = dest,
                _ => break,
            }
        }
        out
    }

    /// Events of one instance played from 0, stopped at `stop` seconds (if ever).
    pub fn events(&self, stop: Option<f32>, horizon: f32) -> Vec<(f32, SoundEvent)> {
        let mut out = self.walk(self.default_state, stop.unwrap_or(horizon));
        if let Some(stop) = stop {
            let hash = crc32fast::hash(b"Stop");
            if let Some(st) = self
                .states
                .iter()
                .position(|s| s.name_hash == hash && s.clip.is_some())
            {
                let len = self.clips[self.states[st].clip.unwrap_or(0)].length;
                out.extend(
                    self.walk(st, len.max(1e-3))
                        .into_iter()
                        .map(|(t, e)| (stop + t, e)),
                );
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0));
        out
    }
}
