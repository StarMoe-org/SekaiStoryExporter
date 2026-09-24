//! # sse-timeline -- timeline compiler
//!
//! Compiles the relative semantics of the IR into absolute simulation frames by replaying
//! the game's `ScenarioPlayer` coroutine scheduling one frame at a time (decision Q33).
//! **Sole owner of pacing correctness.** Behaviour references:
//! `docs/reverse/versions/cn-6.4.0/scenario-player.md` and `special-effects.md`.
//!
//! ## Model
//! - Every frame, running snippet coroutines are resumed in start order, then `PlayCore`
//!   advances. (Unity does not specify the order between MonoBehaviours; this choice can
//!   shift a `WaitFinished` gate by one frame — recorded in the report.)
//! - Waits follow Unity: `while (elapsed < d) { elapsed += deltaTime; yield; }` with f32
//!   accumulation; `WaitForSeconds(d)` resumes on the first frame where the accumulated time
//!   reaches `d`. Both are `TimeBase::frames_for`.
//! - Starting a coroutine runs it synchronously up to its first yield, so a run of `Now`
//!   snippets with no delay all take effect in the same frame.
//!
//! ## Not responsible for
//! - Any rendering or state evaluation (that is `sse-bake`)
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-ir`, `sse-assets`.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use sse_assets::Library;
use sse_core::{TimeBase, consts};
use sse_ir::*;

/// Timing of one instruction, in simulation frames.
#[derive(Debug, Clone, Serialize)]
pub struct InstrTiming {
    pub index: u32,
    /// Registered in the running set (`executeSnippets.Add`).
    pub start: u32,
    /// The delay has elapsed and the action takes effect.
    pub act: u32,
    /// `FinishSnippet`: removed from the running set.
    pub finish: u32,
    pub talk: Option<TalkTiming>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TalkTiming {
    /// First `ShowWords` iteration.
    pub typing_start: u32,
    /// Frames per character (two `WaitForSeconds(wordInterval / 2)`).
    pub frames_per_char: u32,
    /// Frames per half character (one of the two waits).
    pub frames_per_half: u32,
    /// UTF-16 length of the body (C# `string.Length`, what `Substring` counts).
    pub length: u32,
    /// Frame the last iteration finished (all text visible).
    pub typing_end: u32,
    /// Seconds of the longest voice, when any voice plays.
    pub voice_seconds: Option<f32>,
}

impl TalkTiming {
    /// Number of UTF-16 units visible at `frame` (`words.Substring(0, n)`).
    pub fn visible_at(&self, frame: u32) -> u32 {
        if frame < self.typing_start {
            return 0;
        }
        ((frame - self.typing_start) / self.frames_per_char).min(self.length)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Timeline {
    pub fps: u32,
    /// `PlayCore` starts executing snippets at this frame (10 warm-up frames).
    pub first_frame: u32,
    /// One past the last frame of the episode (after `playedWaitTime`).
    pub end_frame: u32,
    pub instrs: BTreeMap<u32, InstrTiming>,
    pub notes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum TimelineError {
    #[error(transparent)]
    Asset(#[from] sse_assets::AssetError),
    #[error("scheduler did not finish within {0} frames (stuck at snippet {1})")]
    Stuck(u32, u32),
}

/// Safety bound: two hours at 60 fps.
const MAX_FRAMES: u32 = 60 * 60 * 120;

pub fn compile(lib: &Library, ep: &Episode, tb: TimeBase) -> Result<Timeline, TimelineError> {
    let instrs = ep.instrs();
    let mut durations = AudioDurations::default();
    for i in &instrs {
        for f in audio_files(i) {
            durations.load(lib, f)?;
        }
    }
    Scheduler::new(tb, &instrs, &durations).run()
}

fn audio_files(i: &Instr) -> Vec<&LibPath> {
    let mut out = Vec::new();
    match &i.kind {
        InstrKind::Talk(t) => {
            for v in &t.voices {
                if let Some(a) = &v.voice {
                    out.extend(a.files.iter());
                }
            }
        }
        InstrKind::Effect(Effect {
            op: EffectOp::FullScreenText { voice: Some(a), .. },
            ..
        }) => out.extend(a.files.iter()),
        _ => {}
    }
    out
}

#[derive(Default)]
struct AudioDurations(BTreeMap<String, f32>);

impl AudioDurations {
    fn load(&mut self, lib: &Library, f: &LibPath) -> Result<(), sse_assets::AssetError> {
        if !self.0.contains_key(&f.0) {
            let pcm = lib.load_wav(&f.0)?;
            self.0.insert(f.0.clone(), pcm.duration() as f32);
        }
        Ok(())
    }

    fn of(&self, a: &AudioRef) -> f32 {
        a.files
            .iter()
            .filter_map(|f| self.0.get(&f.0))
            .fold(0.0, |m, &d| m.max(d))
    }
}

/// What a running snippet coroutine is waiting for.
#[derive(Debug, Clone)]
enum Wait {
    /// The delay loop; the action runs when `frame >= until`.
    Delay { until: u32 },
    /// Same-character layout gate (`busyLayoutCharacter`).
    Busy { character: CharacterId },
    /// Resume at `until`, then continue with `then`.
    Until { until: u32, then: Next },
    /// Talk: typing done at `typing_end`; voice ends at `voice_end`.
    TalkAuto { stage: TalkStage },
}

#[derive(Debug, Clone, Copy)]
enum Next {
    Finish,
    /// Layout: release the busy gate then finish.
    ReleaseBusy(CharacterId),
}

#[derive(Debug, Clone, Copy)]
enum TalkStage {
    /// Waiting for typing to complete.
    Typing { typing_end: u32 },
    /// A timed wait (`autoTextNextPageDelay` without voice, or the voice with its 30 s
    /// timeout); afterwards the gate runs with `then_wait` pending.
    Pause { until: u32, then_wait: f32 },
    /// `CheckAutoModeTalkNext`: no layout snippet may be running.
    Gate { wait: f32 },
    /// Final `WaitForSeconds(wait)`, then `OnClick` → finish.
    Final { until: u32 },
}

enum Step {
    Blocked,
    Next(TalkStage),
    Done,
}

struct Task {
    index: u32,
    pos: usize,
    wait: Wait,
}

struct Scheduler<'a> {
    tb: TimeBase,
    instrs: &'a [&'a Instr],
    durations: &'a AudioDurations,
    timings: BTreeMap<u32, InstrTiming>,
    running: Vec<Task>,
    busy: BTreeSet<CharacterId>,
    notes: Vec<String>,
}

impl<'a> Scheduler<'a> {
    fn new(tb: TimeBase, instrs: &'a [&'a Instr], durations: &'a AudioDurations) -> Self {
        Self {
            tb,
            instrs,
            durations,
            timings: BTreeMap::new(),
            running: Vec::new(),
            busy: BTreeSet::new(),
            notes: vec![
                "running coroutines are resumed before PlayCore within a frame (order not specified by Unity)".into(),
                "talk window is assumed ready immediately; its 0.15 s show fade is not waited for".into(),
            ],
        }
    }

    fn run(mut self) -> Result<Timeline, TimelineError> {
        let first = consts::PLAYCORE_WARMUP_FRAMES;
        let mut seq = 0usize;
        let mut frame = first;
        loop {
            // 1. resume running coroutines, in start order
            let mut k = 0;
            while k < self.running.len() {
                if self.resume(k, frame) {
                    let t = self.running.remove(k);
                    self.timings.get_mut(&t.index).expect("started").finish = frame;
                } else {
                    k += 1;
                }
            }
            // 2. PlayCore
            while seq < self.instrs.len() {
                let instr = self.instrs[seq];
                if instr.progress == Progress::WaitFinished && !self.running.is_empty() {
                    break;
                }
                self.start(seq, frame);
                seq += 1;
            }
            if seq >= self.instrs.len() && self.running.is_empty() {
                break;
            }
            frame += 1;
            if frame > MAX_FRAMES {
                let at = self.instrs.get(seq).map_or(u32::MAX, |i| i.index);
                return Err(TimelineError::Stuck(MAX_FRAMES, at));
            }
        }
        // WaitAllFinished → cleanup → WaitForSeconds(playedWaitTime)
        let end = frame + self.tb.frames_for(consts::PLAYED_WAIT_TIME);
        Ok(Timeline {
            fps: self.tb.fps(),
            first_frame: first,
            end_frame: end + 1,
            instrs: self.timings,
            notes: self.notes,
        })
    }

    /// `executeSnippets.Add` + `executer.Execute(action)`: runs synchronously to the first yield.
    fn start(&mut self, pos: usize, frame: u32) {
        let instr = self.instrs[pos];
        self.timings.insert(
            instr.index,
            InstrTiming {
                index: instr.index,
                start: frame,
                act: frame,
                finish: frame,
                talk: None,
            },
        );
        let delay_frames = self.tb.frames_for(instr.delay);
        let mut task = Task {
            index: instr.index,
            pos,
            wait: Wait::Delay {
                until: frame + delay_frames,
            },
        };
        if delay_frames == 0
            && self.advance(&mut task, frame) {
                self.timings.get_mut(&instr.index).expect("inserted").finish = frame;
                return;
            }
        self.running.push(task);
    }

    /// Resumes running task `k`. Returns true when it finished this frame.
    fn resume(&mut self, k: usize, frame: u32) -> bool {
        let mut task = std::mem::replace(
            &mut self.running[k],
            Task {
                index: 0,
                pos: 0,
                wait: Wait::Delay { until: 0 },
            },
        );
        let done = self.advance(&mut task, frame);
        self.running[k] = task;
        done
    }

    /// Advances a task as far as it can go this frame.
    fn advance(&mut self, task: &mut Task, frame: u32) -> bool {
        loop {
            match task.wait.clone() {
                Wait::Delay { until } => {
                    if frame < until {
                        return false;
                    }
                    self.timings.get_mut(&task.index).expect("started").act = frame;
                    match self.act(task.pos, frame) {
                        Some(w) => task.wait = w,
                        None => return true,
                    }
                }
                Wait::Busy { character } => {
                    if self.busy.contains(&character) {
                        return false;
                    }
                    self.busy.insert(character);
                    task.wait = self.layout_body(task.pos, frame);
                }
                Wait::Until { until, then } => {
                    if frame < until {
                        return false;
                    }
                    if let Next::ReleaseBusy(c) = then {
                        self.busy.remove(&c);
                    }
                    return true;
                }
                Wait::TalkAuto { stage } => match self.talk_step(task, stage, frame) {
                    Step::Blocked => return false,
                    Step::Next(next) => task.wait = Wait::TalkAuto { stage: next },
                    Step::Done => return true,
                },
            }
        }
    }

    /// One step of the talk's auto-advance logic (`TalkWindow` auto mode).
    fn talk_step(&self, task: &Task, stage: TalkStage, frame: u32) -> Step {
        match stage {
            TalkStage::Typing { typing_end } => {
                if frame < typing_end {
                    return Step::Blocked;
                }
                let timing = &self.timings[&task.index];
                let t = timing.talk.as_ref().expect("talk timing");
                Step::Next(match t.voice_seconds {
                    None => TalkStage::Pause {
                        until: frame + self.tb.frames_for(consts::AUTO_NEXT_PAGE_DELAY),
                        then_wait: 0.0,
                    },
                    Some(voice) => {
                        let voice_end = timing.act + self.tb.frames_for(voice);
                        if frame >= voice_end {
                            TalkStage::Gate {
                                wait: consts::AUTO_NEXT_PAGE_DELAY,
                            }
                        } else {
                            let timeout = frame + self.tb.frames_for(consts::AUTO_VOICE_TIMEOUT);
                            TalkStage::Pause {
                                until: voice_end.min(timeout),
                                then_wait: consts::AUTO_WAIT_AFTER_VOICE,
                            }
                        }
                    }
                })
            }
            TalkStage::Pause { until, then_wait } => {
                if frame < until {
                    Step::Blocked
                } else {
                    Step::Next(TalkStage::Gate { wait: then_wait })
                }
            }
            TalkStage::Gate { wait } => {
                let layout_running = self.running.iter().any(|t| {
                    t.index != task.index && matches!(self.instrs[t.pos].kind, InstrKind::Layout(_))
                });
                if layout_running {
                    // each looped frame sets `wait = 0.5`
                    return if wait == consts::AUTO_WAIT_AFTER_VOICE {
                        Step::Blocked
                    } else {
                        Step::Next(TalkStage::Gate {
                            wait: consts::AUTO_WAIT_AFTER_VOICE,
                        })
                    };
                }
                let n = if wait > 0.0 { self.tb.frames_for(wait) } else { 0 };
                Step::Next(TalkStage::Final { until: frame + n })
            }
            TalkStage::Final { until } => {
                if frame < until {
                    Step::Blocked
                } else {
                    Step::Done
                }
            }
        }
    }

    /// The action body right after the delay. `None` = finished immediately.
    fn act(&mut self, pos: usize, frame: u32) -> Option<Wait> {
        let instr = self.instrs[pos];
        let after = |tb: TimeBase, seconds: f32| {
            let n = tb.frames_for(seconds);
            (n > 0).then_some(Wait::Until {
                until: frame + n,
                then: Next::Finish,
            })
        };
        match &instr.kind {
            InstrKind::Wait
            | InstrKind::ChangeMotion { .. }
            | InstrKind::Sound { .. }
            | InstrKind::SetLayoutMode { .. } => None,
            InstrKind::Talk(t) => Some(self.talk_start(instr.index, t, frame)),
            InstrKind::Layout(l) => {
                if self.busy.contains(&l.character) {
                    Some(Wait::Busy {
                        character: l.character,
                    })
                } else {
                    self.busy.insert(l.character);
                    Some(self.layout_body(pos, frame))
                }
            }
            InstrKind::Effect(e) => match self.effect_seconds(instr.index, e) {
                Some(s) => after(self.tb, s),
                None => None,
            },
            InstrKind::Unsupported(u) => match u.finish {
                FinishRule::Immediate => None,
                FinishRule::After { seconds } => after(self.tb, seconds),
            },
        }
    }

    fn layout_body(&mut self, pos: usize, frame: u32) -> Wait {
        let InstrKind::Layout(l) = &self.instrs[pos].kind else {
            unreachable!("layout_body on non-layout")
        };
        let seconds = match &l.op {
            LayoutOp::Move { duration, .. } => *duration,
            LayoutOp::Appear { .. } | LayoutOp::Hide => consts::CHARACTER_FADE_DURATION,
            LayoutOp::Shake { .. } => {
                self.note_once("layout Shake duration not reversed; finishes immediately");
                0.0
            }
            LayoutOp::Depth { .. } => 0.0,
        };
        Wait::Until {
            until: frame + self.tb.frames_for(seconds),
            then: Next::ReleaseBusy(l.character),
        }
    }

    fn talk_start(&mut self, index: u32, t: &Talk, frame: u32) -> Wait {
        let half = self.tb.frames_for(consts::WORD_INTERVAL * 0.5);
        let per_char = half * 2;
        let length = t.body.encode_utf16().count() as u32;
        // ShowWords: iterations i = 0..=length, each shows i units then waits two halves.
        let typing_end = frame + (length + 1) * per_char;
        let voice = t
            .voices
            .iter()
            .filter_map(|v| v.voice.as_ref())
            .map(|a| self.durations.of(a))
            .fold(None, |m: Option<f32>, d| Some(m.map_or(d, |m| m.max(d))));
        self.timings.get_mut(&index).expect("started").talk = Some(TalkTiming {
            typing_start: frame,
            frames_per_char: per_char,
            frames_per_half: half,
            length,
            typing_end,
            voice_seconds: voice,
        });
        Wait::TalkAuto {
            stage: TalkStage::Typing { typing_end },
        }
    }

    /// Seconds until `FinishSnippet` after the delay; `None` = immediate.
    fn effect_seconds(&mut self, index: u32, e: &Effect) -> Option<f32> {
        use EffectOp::*;
        let d = e.duration;
        match &e.op {
            Fade { .. } | ChangeBackground { .. } | Blur { .. } | SideFade { .. } => Some(d),
            CameraMove { .. } | CameraZoom { .. } => Some(d),
            Telop { .. } => Some(
                consts::TELOP_ANIM_CLIP_LENGTH * 2.0 + consts::TELOP_AUTO_HOLD,
            )
            .map(|_| {
                // frame-accurate: two clip plays + the hold loop, each rounded separately
                let n = self.tb.frames_for(consts::TELOP_ANIM_CLIP_LENGTH) * 2
                    + self.tb.frames_for(consts::TELOP_AUTO_HOLD);
                n as f32 / self.tb.fps() as f32 - 1e-6
            }),
            FullScreenText { voice, .. } => {
                self.note_once(
                    "FullScreenText completion not reversed: approximated as voice length + 0.5 s (min 2 s)",
                );
                let v = voice.as_ref().map_or(0.0, |a| self.durations.of(a));
                Some((v + consts::AUTO_WAIT_AFTER_VOICE).max(consts::AUTO_NEXT_PAGE_DELAY))
            }
            SekaiTransition { .. } => {
                self.note_once(
                    "Sekai transition completion not reversed: approximated as max(Duration, 1 s)",
                );
                Some(d.max(1.0))
            }
            ShakeScreen | ShakeWindow => {
                let _ = index;
                None
            }
            _ => None,
        }
    }

    fn note_once(&mut self, note: &str) {
        if !self.notes.iter().any(|n| n == note) {
            self.notes.push(note.to_owned());
        }
    }
}
