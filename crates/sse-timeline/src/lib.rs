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
    pub full_screen_text: Option<FstTiming>,
}

/// `ScenarioFullScreenTextDialog.PlayCore` (special effect 24), in simulation frames.
#[derive(Debug, Clone, Serialize)]
pub struct FstTiming {
    /// `ViewType.First`: the previous snippet is not a full-screen text, so the dialog opens
    /// and the cinemascope bars slide in (`PlayCinemascope(true)`, then `Delay(0.5)`).
    pub first: bool,
    /// `ViewType.Last`: the next snippet is not a full-screen text, so the text fades out
    /// together with the bars (`WhenAll(FadeOutAll(1), PlayCinemascope(false))`).
    pub last: bool,
    /// Voice and `TextAppearFade.Play` start.
    pub text_start: u32,
    /// Frames each `characterInfo` slot takes to fade in (`textWait` = 0.125 s).
    pub frames_per_slot: u32,
    /// Length of TMP's `characterInfo` array: the fade walks the whole array, empty slots
    /// included. The array only grows while the same dialog is reused.
    pub slots: u32,
    /// All slots are opaque.
    pub text_end: u32,
    /// `FadeOutAll` / bars out start (`Last` only).
    pub fade_start: Option<u32>,
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
    let motions = MotionClips::load(lib, ep)?;
    Scheduler::new(tb, ep, &instrs, &durations, &motions).run()
}

/// Length and loop flag of every body-motion clip the episode can reference, keyed by
/// (costume, motion name). Only the body layer matters for pacing: `CheckAutoModeTalkNext`
/// asks `ScenarioModel.IsMotionFinished`, which is `PlayableAnimator.IsFinishedAnimation(0)`.
#[derive(Default)]
struct MotionClips(BTreeMap<(String, String), (f32, bool)>);

impl MotionClips {
    fn load(lib: &Library, ep: &Episode) -> Result<Self, sse_assets::AssetError> {
        let mut out = Self::default();
        let mut names = BTreeSet::new();
        for p in &ep.initial.layout {
            names.extend(p.motion.clone());
        }
        for i in ep.instrs() {
            match &i.kind {
                InstrKind::Talk(t) => names.extend(t.motions.iter().filter_map(|m| m.motion.clone())),
                InstrKind::Layout(Layout {
                    op: LayoutOp::Appear { motion: Some(m), .. },
                    ..
                }) => {
                    names.insert(m.clone());
                }
                InstrKind::ChangeMotion { motion: Some(m), .. } => {
                    names.insert(m.clone());
                }
                _ => {}
            }
        }
        for cast in ep.cast.values() {
            for (costume, entry) in &cast.costumes {
                for name in &names {
                    if let Some(path) = entry.motions.get(name) {
                        let m = lib.load_motion(&path.0)?;
                        out.0.insert(
                            (costume.clone(), name.clone()),
                            (m.stop_time - m.start_time, m.loop_time),
                        );
                    }
                }
            }
        }
        Ok(out)
    }
}

/// What the pacing needs to know about one character (`ScenarioModelView` + its model).
#[derive(Debug, Clone, Default)]
struct CharState {
    costume: Option<String>,
    /// `CurrentDisplayStatus == Appear`.
    shown: bool,
    /// Body layer: frame the clip was set, frames until `normalizedTime >= 1`, looping.
    body: Option<(u32, u32, bool)>,
    /// Talk-embedded body motions not played yet: (frame, motion).
    pending: Vec<(u32, String)>,
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
    /// Frame `PlayCore` started it (its synchronous part already ran then).
    started: u32,
    pos: usize,
    wait: Wait,
}

struct Scheduler<'a> {
    tb: TimeBase,
    ep: &'a Episode,
    instrs: &'a [&'a Instr],
    durations: &'a AudioDurations,
    motions: &'a MotionClips,
    chars: BTreeMap<CharacterId, CharState>,
    /// TMP `characterInfo` length of the current full-screen text.
    fst_slots: u32,
    timings: BTreeMap<u32, InstrTiming>,
    running: Vec<Task>,
    busy: BTreeSet<CharacterId>,
    notes: Vec<String>,
}

impl<'a> Scheduler<'a> {
    fn new(
        tb: TimeBase,
        ep: &'a Episode,
        instrs: &'a [&'a Instr],
        durations: &'a AudioDurations,
        motions: &'a MotionClips,
    ) -> Self {
        let mut s = Self {
            tb,
            ep,
            instrs,
            durations,
            motions,
            chars: BTreeMap::new(),
            fst_slots: 0,
            timings: BTreeMap::new(),
            running: Vec::new(),
            busy: BTreeSet::new(),
            notes: vec![
                "PlayCore is resumed before the snippet coroutines within a frame (Unity queue order)".into(),
                "talk window is assumed ready immediately; its 0.15 s show fade is not waited for".into(),
            ],
        };
        for p in &ep.initial.layout {
            s.appear(p.character, p.costume.as_deref(), p.motion.as_deref(), 0);
        }
        s
    }

    /// `ScenarioModelView` appears (display status `Appear`), optionally changing costume
    /// and body motion.
    fn appear(&mut self, id: CharacterId, costume: Option<&str>, motion: Option<&str>, frame: u32) {
        let initial = self.ep.cast.get(&id).and_then(|c| c.initial_costume.clone());
        let c = self.chars.entry(id).or_default();
        if let Some(costume) = costume {
            c.costume = Some(costume.to_owned());
        } else if c.costume.is_none() {
            c.costume = initial;
        }
        c.shown = true;
        self.set_body(id, motion, frame);
    }

    /// Plays a body motion (`ChangeAnimation` on layer 0). Unknown names keep the previous
    /// clip, as the game does (`Live2DModel.RegisterMotion` miss).
    fn set_body(&mut self, id: CharacterId, motion: Option<&str>, frame: u32) {
        let Some(name) = motion else { return };
        let Some(c) = self.chars.get_mut(&id) else { return };
        let Some(costume) = c.costume.clone() else { return };
        if let Some(&(len, looping)) = self.motions.0.get(&(costume, name.to_owned())) {
            c.body = Some((frame, self.tb.frames_for(len), looping));
        }
    }

    /// Talk-embedded motions due by `frame` (`TalkMotionChangeSyncVoiceTime`).
    fn play_pending(&mut self, frame: u32) {
        let due: Vec<(CharacterId, String)> = self
            .chars
            .iter_mut()
            .flat_map(|(id, c)| {
                let (now, later): (Vec<_>, Vec<_>) =
                    std::mem::take(&mut c.pending).into_iter().partition(|p| p.0 <= frame);
                c.pending = later;
                now.into_iter().map(|p| (*id, p.1)).collect::<Vec<_>>()
            })
            .collect();
        for (id, m) in due {
            self.set_body(id, Some(&m), frame);
        }
    }

    /// `CheckAutoModeTalkNext`'s second half: every shown character whose body motion is
    /// not looping must have finished it (`normalizedTime >= 1`).
    fn motions_settled(&self, frame: u32) -> bool {
        self.chars.values().all(|c| match (c.shown, c.body) {
            (true, Some((start, frames, false))) => frame >= start + frames,
            _ => true,
        })
    }

    fn run(mut self) -> Result<Timeline, TimelineError> {
        let first = consts::PLAYCORE_WARMUP_FRAMES;
        let mut seq = 0usize;
        let mut frame = first;
        loop {
            // 1. PlayCore. Unity resumes `yield return null` coroutines in the order they
            //    were queued; PlayCore has been looping since the episode started, so it runs
            //    before every snippet coroutine and sees their finishes one frame late.
            while seq < self.instrs.len() {
                let instr = self.instrs[seq];
                if instr.progress == Progress::WaitFinished && !self.running.is_empty() {
                    break;
                }
                self.start(seq, frame);
                seq += 1;
            }
            // 2. resume the snippet coroutines queued in earlier frames, in start order
            self.play_pending(frame);
            let mut k = 0;
            while k < self.running.len() {
                if self.running[k].started == frame {
                    k += 1;
                } else if self.resume(k, frame) {
                    let t = self.running.remove(k);
                    self.timings.get_mut(&t.index).expect("started").finish = frame;
                } else {
                    k += 1;
                }
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
                full_screen_text: None,
            },
        );
        let delay_frames = self.tb.frames_for(instr.delay);
        let mut task = Task {
            index: instr.index,
            started: frame,
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
                started: 0,
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
                    Step::Done => {
                        // OnFinishTalkWindow → StopTalkMotionTimingCoroutine
                        for c in self.chars.values_mut() {
                            c.pending.clear();
                        }
                        return true;
                    }
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
                    // No voice: `while (waitTime < autoTextNextPageDelay)` then `OnClick`
                    // straight away -- this path never asks `CheckAutoModeTalkNext`.
                    None => TalkStage::Final {
                        until: frame + self.tb.frames_for(consts::AUTO_NEXT_PAGE_DELAY),
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
                if layout_running || !self.motions_settled(frame) {
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
            InstrKind::ChangeMotion { character, motion, .. } => {
                self.set_body(*character, motion.as_deref(), frame);
                None
            }
            InstrKind::Wait | InstrKind::Sound { .. } | InstrKind::SetLayoutMode { .. } => None,
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
            InstrKind::Effect(Effect {
                op: EffectOp::FullScreenText { text, voice, .. },
                ..
            }) => {
                let n = self.full_screen_text(pos, text, voice.as_ref(), frame);
                Some(Wait::Until {
                    until: frame + n,
                    then: Next::Finish,
                })
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
        let instrs = self.instrs;
        let InstrKind::Layout(l) = &instrs[pos].kind else {
            unreachable!("layout_body on non-layout")
        };
        let seconds = match &l.op {
            LayoutOp::Move { duration, .. } => *duration,
            LayoutOp::Appear {
                costume, motion, ..
            } => {
                let (id, costume, motion) = (l.character, costume.clone(), motion.clone());
                self.appear(id, costume.as_deref(), motion.as_deref(), frame);
                return Wait::Until {
                    until: frame + fade_opacity_frames(self.tb, 0.0),
                    then: Next::ReleaseBusy(l.character),
                };
            }
            LayoutOp::Hide { delay } => {
                if let Some(c) = self.chars.get_mut(&l.character) {
                    c.shown = false;
                }
                return Wait::Until {
                    until: frame + fade_opacity_frames(self.tb, *delay),
                    then: Next::ReleaseBusy(l.character),
                };
            }
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
        // Talk-embedded body motions. `TalkMotionChangeSyncVoiceTime` starts synchronously:
        // iteration j runs at frame + j with `elapsed = (j + 1) * dt` and plays at most one
        // motion per iteration once `elapsed >= TimingSyncValue`.
        let mut j = 0u32;
        let dt = self.tb.delta();
        for m in &t.motions {
            let mut elapsed = (j + 1) as f32 * dt;
            if t.motion_change == MotionChangeFactor::PlayTime {
                while elapsed < m.timing_sync_value {
                    j += 1;
                    elapsed += dt;
                }
            }
            if let Some(name) = &m.motion {
                if j == 0 {
                    self.set_body(m.character, Some(name), frame);
                } else if let Some(c) = self.chars.get_mut(&m.character) {
                    c.pending.push((frame + j, name.clone()));
                }
            }
            j += 1;
        }
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

    /// `ScenarioPlayer.PlayFullScreenText` → `ScenarioFullScreenTextDialog.PlayCore`.
    /// Returns the frames until `onFinished` (→ `FinishSnippet`).
    fn full_screen_text(&mut self, pos: usize, text: &str, voice: Option<&AudioRef>, frame: u32) -> u32 {
        let is_fst = |i: Option<&&Instr>| {
            matches!(
                i.map(|i| &i.kind),
                Some(InstrKind::Effect(Effect { op: EffectOp::FullScreenText { .. }, .. }))
            )
        };
        let index = self.instrs[pos].index;
        let prev = self.instrs.iter().find(|i| i.index + 1 == index);
        let next = self.instrs.iter().find(|i| i.index == index + 1);
        let first = !is_fst(prev);
        let last = !is_fst(next);
        let tb = self.tb;
        let mut t = 0;
        if first {
            t += tb.frames_for(consts::FST_PLAY_DURATION); // PlayCinemascope(true)
            t += tb.frames_for(consts::FST_OPEN_DELAY); // UniTask.Delay(0.5)
        }
        if first {
            // a new dialog: `new TMP_TextInfo()` allocates characterInfo[8]
            self.fst_slots = consts::TMP_CHARACTER_INFO_INITIAL;
        }
        let text_start = t;
        self.fst_slots = tmp_character_info_len(self.fst_slots, text);
        let per_slot = tb.frames_for(consts::FST_TEXT_WAIT);
        t += self.fst_slots * per_slot;
        let text_end = t;
        // do { t += dt; if (t >= 10) break; yield; } while (!isVoiceFinish)
        let voice_end = voice.map(|a| text_start + tb.frames_for(self.durations.of(a)));
        t = match voice_end {
            Some(v) if v > t + 1 => v.min(t + tb.frames_for(consts::FST_VOICE_TIMEOUT)),
            _ => t + 1,
        };
        t += tb.frames_for(consts::FST_PLAY_DURATION); // hold
        let fade_start = last.then_some(frame + t);
        if last {
            t += tb.frames_for(consts::FST_PLAY_DURATION); // FadeOutAll(1) ∥ bars out (1 s)
        }
        self.timings.get_mut(&index).expect("started").full_screen_text = Some(FstTiming {
            first,
            last,
            text_start: frame + text_start,
            frames_per_slot: per_slot,
            slots: self.fst_slots,
            text_end: frame + text_end,
            fade_start,
        });
        t
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
            FullScreenText { .. } => unreachable!("handled by full_screen_text"),
            // `ColorFader.Play(white, delay, Duration, FinishSnippet)` (effect handlers
            // 0x16DD1E8 / 0x16DD4A4, 3anv 0x16DDC10 / 0x16DD11C)
            SekaiTransition { dir, .. } => {
                let delay = match dir {
                    Direction::In => consts::SEKAI_IN_FADE_DELAY,
                    Direction::Out => consts::SEKAI_OUT_FADE_DELAY,
                };
                let n = self.tb.frames_for(delay) + self.tb.frames_for(d);
                Some(n as f32 / self.tb.fps() as f32 - 1e-6)
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

/// `Live2DModelView.FadeOpacityCoroutine`: `yield return new WaitForSeconds(delay)` (at least
/// one frame, even for 0), then `while (time < 0.1) { lerp; time += dt; yield; }`.
fn fade_opacity_frames(tb: TimeBase, delay: f32) -> u32 {
    tb.frames_for(delay).max(1) + tb.frames_for(consts::CHARACTER_FADE_DURATION)
}

/// `TMP_Text.SetArraySizes` (`TextMeshProUGUI` 0x4B52F90): before parsing, `characterInfo`
/// is resized to exactly `m_InternalTextProcessingArraySize` when that is larger (never
/// shrunk). That size is the UTF-32 length of the source text after
/// `TextAppearFade.Initialize` → `TrimStartLine`, rich-text tags included; the block-allocated
/// growth inside the loop cannot trigger because tags only reduce the laid-out count.
fn tmp_character_info_len(current: u32, text: &str) -> u32 {
    let n = text.trim_start_matches(['\n', '\r']).chars().count() as u32;
    current.max(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmp_array_only_grows_to_the_exact_length() {
        assert_eq!(tmp_character_info_len(8, "abc"), 8);
        assert_eq!(tmp_character_info_len(8, "\n0123456789"), 10);
        assert_eq!(tmp_character_info_len(38, "short"), 38);
        assert_eq!(tmp_character_info_len(8, "<b>ab</b>"), 9);
    }
}
