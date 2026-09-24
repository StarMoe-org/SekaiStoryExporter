//! # sse-bake -- Pass 1: state baking
//!
//! Walks the compiled timeline frame by frame, updating state only -- no rendering -- and
//! emits the per-frame parameter table (`sse-params`). Behaviour references are the
//! reverse-engineering documents under `docs/reverse/versions/cn-6.4.0/`; every place that
//! is an approximation pushes a note into the table's report.
//!
//! Per frame the order is: snippet actions that take effect this frame (coroutines, i.e.
//! `Update`), then the animator (motion + facial), eye blink, lip sync, breath, physics
//! (`LateUpdate`), then the snapshot.

mod character;
mod lipsync;
mod shake;

use std::collections::BTreeMap;
use std::sync::Arc;

use sse_assets::Library;
use sse_core::{TimeBase, consts, rng::Rng};
use sse_ir::*;
use sse_params::*;
use sse_timeline::Timeline;

use character::{CharacterRt, ModelInfo};

#[derive(Debug, thiserror::Error)]
pub enum BakeError {
    #[error(transparent)]
    Asset(#[from] sse_assets::AssetError),
    #[error("{0}: {1}")]
    Live2d(String, String),
}

#[derive(Debug, Clone)]
pub struct BakeOptions {
    /// UI content size (`ScreenManager.ContentSize`) for the output aspect ratio.
    pub content_size: [f32; 2],
    /// Seed for the game's `UnityEngine.Random` call sites (breath phase).
    pub seed: u64,
}

pub fn bake(
    lib: &Library,
    ep: &Episode,
    tl: &Timeline,
    opts: &BakeOptions,
) -> Result<ParamTable, BakeError> {
    Baker::new(lib, ep, tl, opts).run()
}

struct Tween {
    from: f32,
    to: f32,
    start: u32,
    frames: u32,
    ease_out_quad: bool,
}

/// A `ScenarioSideFadePlayer.Play` run.
struct SideFadeRt {
    from: [f32; 2],
    to: [f32; 2],
    start: u32,
    frames: u32,
    /// `OnPlayFinished` deactivates the panel for the *In types (mask 0xAA).
    hide: bool,
}

impl SideFadeRt {
    fn at(&self, frame: u32) -> Option<[f32; 2]> {
        if frame >= self.start + self.frames && self.hide {
            return None;
        }
        let p = if self.frames == 0 || frame >= self.start + self.frames {
            1.0
        } else if frame <= self.start {
            0.0
        } else {
            let t = (frame - self.start) as f32 / self.frames as f32;
            -t * (t - 2.0)
        };
        Some([self.from[0] + (self.to[0] - self.from[0]) * p, self.from[1] + (self.to[1] - self.from[1]) * p])
    }
}

impl Tween {
    fn at(&self, frame: u32) -> f32 {
        if self.frames == 0 || frame >= self.start + self.frames {
            return self.to;
        }
        if frame <= self.start {
            return self.from;
        }
        let mut t = (frame - self.start) as f32 / self.frames as f32;
        if self.ease_out_quad {
            t = -t * (t - 2.0);
        }
        self.from + (self.to - self.from) * t
    }
}


#[derive(Debug, Clone, Copy, PartialEq)]
enum PlaceStatus {
    SlideIn,
    Active,
    SlideOut,
}

/// `ScenarioPlaceInfo` (`ScreenSlideInOut`: `DOAnchorPos(to, 0.2)` with `Ease.OutQuart`).
struct PlaceInfoRt {
    text: String,
    from: f32,
    to: f32,
    start: u32,
    status: PlaceStatus,
    reserve_close: bool,
    /// `DefaultPosX` ([`consts::place_info_hidden_x`]).
    hidden_x: f32,
}

impl PlaceInfoRt {
    fn frames(tb: TimeBase) -> u32 {
        tb.frames_for(consts::PLACE_INFO_SLIDE_DURATION).max(1)
    }

    fn x_at(&self, f: u32, tb: TimeBase) -> f32 {
        let t = (f.saturating_sub(self.start) as f32 / Self::frames(tb) as f32).min(1.0);
        let e = 1.0 - (1.0 - t).powi(4); // OutQuart
        self.from + (self.to - self.from) * e
    }

    /// Settles a finished slide-in (and a reserved close) before frame `f`.
    fn advance(&mut self, f: u32, tb: TimeBase) {
        let end = self.start + Self::frames(tb);
        if self.status == PlaceStatus::SlideIn && f >= end {
            self.status = PlaceStatus::Active;
            if self.reserve_close {
                self.reserve_close = false;
                self.start_slide_out(end);
            }
        }
    }

    fn start_slide_out(&mut self, f: u32) {
        self.from = self.to;
        self.to = self.hidden_x;
        self.start = f;
        self.status = PlaceStatus::SlideOut;
    }

    /// `ScenarioPlaceInfo.Hide`: slides out when active, reserves when still sliding in.
    fn hide(&mut self, f: u32, tb: TimeBase) {
        self.advance(f, tb);
        match self.status {
            PlaceStatus::Active => self.start_slide_out(f),
            PlaceStatus::SlideIn => self.reserve_close = true,
            PlaceStatus::SlideOut => {}
        }
    }

    fn state(&mut self, f: u32, tb: TimeBase) -> Option<PlaceInfoState> {
        self.advance(f, tb);
        if self.status == PlaceStatus::SlideOut && f >= self.start + Self::frames(tb) {
            return None; // `SetActive(false)` after the slide-out
        }
        Some(PlaceInfoState { text: self.text.clone(), x: self.x_at(f, tb) })
    }
}

/// One `ScenarioFullScreenTextDialog.PlayCore` run.
struct FstRun {
    text: String,
    end: u32,
    timing: sse_timeline::FstTiming,
}

impl FstRun {
    fn state(&self, f: u32, tb: TimeBase) -> Option<FullScreenTextState> {
        let t = &self.timing;
        if f < t.text_start || f >= self.end {
            return None;
        }
        // TextAppearFade.Play: slot i fades in over frames [start + i·n, start + (i+1)·n),
        // showing alpha = elapsed / textWait before each yield.
        let k = f - t.text_start;
        let n = t.frames_per_slot.max(1);
        let slot = k / n;
        let progress = if slot >= t.slots {
            t.slots as f32
        } else {
            slot as f32 + ((k % n) as f32 * tb.delta() / consts::FST_TEXT_WAIT).min(1.0)
        };
        let alpha = match t.fade_start {
            Some(s) if f >= s => {
                1.0 - ((f - s) as f32 / tb.frames_for(consts::FST_PLAY_DURATION) as f32).min(1.0)
            }
            _ => 1.0,
        };
        Some(FullScreenTextState { text: self.text.clone(), progress, alpha })
    }
}

struct Baker<'a> {
    lib: &'a Library,
    ep: &'a Episode,
    tl: &'a Timeline,
    opts: &'a BakeOptions,
    tb: TimeBase,
    rng: Rng,
    models: Vec<String>,
    model_info: BTreeMap<String, Arc<ModelInfo>>,
    chars: BTreeMap<CharacterId, CharacterRt>,
    /// Sibling order, back to front.
    order: Vec<CharacterId>,
    layout_mode: LayoutMode,
    ambient: [f32; 4],
    background: BackgroundState,
    bg_tween: Option<Tween>,
    fader: [f32; 4],
    fader_from: [f32; 4],
    fader_to: [f32; 4],
    fader_tween: Option<Tween>,
    blur: Option<Tween>,
    blur_value: f32,
    camera_color: Option<CameraColor>,
    talk: Option<(u32, TalkState)>,
    /// Frame the first talk window appeared (auto signal enabled).
    auto_since: Option<u32>,
    talk_window: Option<Tween>,
    /// (text, start, hide clip start, end)
    telop: Vec<(String, u32, u32, u32)>,
    place_info: Option<PlaceInfoRt>,
    full_text: Vec<FstRun>,
    /// `PlayCinemascope(show)` calls: (start frame, show).
    cinemascope: Vec<(u32, bool)>,
    /// (name, video file, start frame, end frame)
    movie: Option<(String, Option<String>, u32, u32)>,
    /// `fx_transition_scenario`: (first frame, frames until `DestroyAtTime`).
    fx: Option<(u32, u32)>,
    /// `screenShakeTweener` on `scenarioRoot` / `TalkWindow.ShakeWindow` on `windowRectTransform`.
    screen_shake: Option<shake::Shake>,
    window_shake: Option<shake::Shake>,
    side_fade: Option<SideFadeRt>,
    audio: Vec<AudioCue>,
    bgm: Option<usize>,
    se_loops: BTreeMap<String, usize>,
    notes: Vec<String>,
}

fn note(notes: &mut Vec<String>, s: &str) {
    if !notes.iter().any(|n| n == s) {
        notes.push(s.to_owned());
    }
}

impl<'a> Baker<'a> {
    fn new(lib: &'a Library, ep: &'a Episode, tl: &'a Timeline, opts: &'a BakeOptions) -> Self {
        Self {
            lib,
            ep,
            tl,
            opts,
            tb: TimeBase::new(tl.fps),
            rng: Rng::new(opts.seed),
            models: Vec::new(),
            model_info: BTreeMap::new(),
            chars: BTreeMap::new(),
            order: Vec::new(),
            layout_mode: ep.initial.layout_mode,
            ambient: consts::MODEL_COLOR_NORMAL,
            background: BackgroundState {
                current: ep.initial.background.as_ref().map(|b| b.0.clone()),
                previous: None,
                mix: 1.0,
            },
            bg_tween: None,
            fader: [0.0; 4],
            fader_from: [0.0; 4],
            fader_to: [0.0; 4],
            fader_tween: None,
            blur: None,
            blur_value: 0.0,
            camera_color: None,
            talk: None,
            talk_window: None,
            telop: Vec::new(),
            place_info: None,
            full_text: Vec::new(),
            cinemascope: Vec::new(),
            auto_since: None,
            movie: None,
            fx: None,
            screen_shake: None,
            window_shake: None,
            side_fade: None,
            audio: Vec::new(),
            bgm: None,
            se_loops: BTreeMap::new(),
            notes: tl.notes.clone(),
        }
    }

    fn run(mut self) -> Result<ParamTable, BakeError> {
        let instrs: BTreeMap<u32, &Instr> =
            self.ep.instrs().into_iter().map(|i| (i.index, i)).collect();
        let mut acts: Vec<(u32, u32)> = self.tl.instrs.values().map(|t| (t.act, t.index)).collect();
        acts.sort();
        let mut finishes: Vec<(u32, u32)> =
            self.tl.instrs.values().map(|t| (t.finish, t.index)).collect();
        finishes.sort();

        if let Some(bgm) = &self.ep.initial.bgm {
            self.play_bgm(bgm, 0, 0.0, 1.0);
        }
        for p in &self.ep.initial.layout {
            self.appear(
                p.character,
                p.side,
                p.offset_x,
                p.costume.as_deref(),
                p.motion.as_deref(),
                p.facial.as_deref(),
                0,
                false,
            )?;
        }

        let dt = self.tb.delta();
        let mut frames = Vec::with_capacity(self.tl.end_frame as usize);
        let (mut ai, mut fi) = (0, 0);
        for f in 0..self.tl.end_frame {
            while ai < acts.len() && acts[ai].0 == f {
                let instr = instrs[&acts[ai].1];
                self.act(instr, f)?;
                ai += 1;
            }
            while fi < finishes.len() && finishes[fi].0 == f {
                let instr = instrs[&finishes[fi].1];
                self.finish(instr, f);
                fi += 1;
            }
            self.scheduled_motions(f)?;
            for c in self.chars.values_mut() {
                c.late_update(f, dt, self.tb);
            }
            frames.push(self.snapshot(f));
        }
        // CleanupSounds(): fade everything out.
        let end = self.tl.end_frame;
        let fade = self.tb.frames_for(consts::CLEANUP_SOUND_FADE_TIME);
        for cue in &mut self.audio {
            if cue.stop_frame.is_none_or(|s| s > end) && cue.looping {
                cue.stop_frame = Some(end);
                cue.fade_out = fade.min(end.saturating_sub(cue.start_frame));
            }
        }
        Ok(ParamTable {
            version: PARAM_TABLE_VERSION,
            fps: self.tl.fps,
            models: self.models,
            frames,
            audio: self.audio,
            notes: self.notes,
        })
    }

    fn model_info(&mut self, bundle: &str) -> Result<(usize, Arc<ModelInfo>), BakeError> {
        if !self.model_info.contains_key(bundle) {
            let info = ModelInfo::load(self.lib, bundle)
                .map_err(|e| BakeError::Live2d(bundle.to_owned(), e))?;
            self.model_info.insert(bundle.to_owned(), Arc::new(info));
            self.models.push(bundle.to_owned());
        }
        let idx = self.models.iter().position(|m| m == bundle).expect("pushed");
        Ok((idx, self.model_info[bundle].clone()))
    }

    fn content(&self) -> [f32; 2] {
        self.opts.content_size
    }

    /// `GetScenarioCharacterTransformData(side, offsetX)` (`layout.yaml`).
    fn side_position(&self, side: Side, offset_x: f32) -> (f32, f32) {
        let x_side = match self.layout_mode {
            LayoutMode::Default => consts::SIDE_X_DEFAULT,
            LayoutMode::Three => consts::SIDE_X_THREE,
        };
        let over_x = self.content()[0] * 0.5 + consts::OVER_POSITION_MARGIN;
        let over_y = self.content()[1] * 0.5 + consts::OVER_POSITION_MARGIN;
        use Side::*;
        let x = match side {
            Left | LeftInside | LeftUnder | LeftInsideUnder => -x_side,
            Right | RightInside | RightUnder | RightInsideUnder => x_side,
            LeftOver => -over_x,
            RightOver => over_x,
            _ => 0.0,
        };
        // `side_resolution`: y is 0 except for the *Under sides (`layout.yaml`).
        let y = if matches!(
            side,
            LeftUnder | LeftInsideUnder | CenterUnder | RightUnder | RightInsideUnder
        ) {
            -over_y
        } else {
            0.0
        };
        (x + offset_x, y)
    }

    fn mode_scale(&self) -> f32 {
        match self.layout_mode {
            LayoutMode::Default => consts::MODE_SCALE_DEFAULT,
            LayoutMode::Three => consts::MODE_SCALE_THREE,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn appear(
        &mut self,
        id: CharacterId,
        side: Side,
        offset_x: f32,
        costume: Option<&str>,
        motion: Option<&str>,
        facial: Option<&str>,
        frame: u32,
        fade: bool,
    ) -> Result<(), BakeError> {
        let cast = self.ep.cast.get(&id);
        let costume = costume
            .map(str::to_owned)
            .or_else(|| self.chars.get(&id).map(|c| c.costume.clone()))
            .or_else(|| cast.and_then(|c| c.initial_costume.clone()));
        let Some(costume) = costume else {
            note(&mut self.notes, &format!("character {id}: no costume; not shown"));
            return Ok(());
        };
        let model_bundle = cast
            .and_then(|c| c.costumes.get(&costume))
            .and_then(|c| c.model.clone());
        let Some(model_bundle) = model_bundle else {
            note(
                &mut self.notes,
                &format!("character {id}: costume {costume} has no model bundle (the game fails to load it too)"),
            );
            return Ok(());
        };
        let (model_index, info) = self.model_info(&model_bundle.0)?;
        let breath_deg = self.rng.range_i32(0, 360) as f32;
        let (x, y) = self.side_position(side, offset_x);
        let scale = self.mode_scale();
        let rt = match self.chars.remove(&id) {
            Some(mut c) if c.costume == costume => {
                c.x = Tween::fixed(x);
                c.y = y;
                c.scale = scale;
                c
            }
            _ => CharacterRt::new(id, costume.clone(), model_index, info, breath_deg, x, y, scale),
        };
        self.chars.insert(id, rt);
        self.order.retain(|c| *c != id);
        self.order.push(id);
        let c = self.chars.get_mut(&id).expect("inserted");
        c.visible = true;
        c.opacity = Tween {
            from: 0.0,
            to: 1.0,
            // FadeOpacityCoroutine starts with WaitForSeconds(0): one frame at opacity 0
            start: if fade { frame + 1 } else { frame },
            frames: if fade {
                self.tb.frames_for(consts::CHARACTER_FADE_DURATION)
            } else {
                0
            },
            ease_out_quad: false,
        };
        self.change_motion(id, motion, facial, true)?;
        Ok(())
    }

    fn change_motion(
        &mut self,
        id: CharacterId,
        motion: Option<&str>,
        facial: Option<&str>,
        first: bool,
    ) -> Result<(), BakeError> {
        let Some(c) = self.chars.get_mut(&id) else {
            return Ok(());
        };
        let table = self
            .ep
            .cast
            .get(&id)
            .and_then(|cast| cast.costumes.get(&c.costume))
            .map(|x| &x.motions);
        for (name, face) in [(motion, false), (facial, true)] {
            let Some(name) = name else { continue };
            let Some(path) = table.and_then(|t| t.get(name)) else {
                // the game keeps the previous motion on this layer
                continue;
            };
            let clip = c.clip(self.lib, &path.0).map_err(|e| BakeError::Live2d(path.0.clone(), e))?;
            c.play(clip, face, first);
        }
        Ok(())
    }

    fn scheduled_motions(&mut self, f: u32) -> Result<(), BakeError> {
        let due: Vec<(CharacterId, Option<String>, Option<String>)> = self
            .chars
            .values_mut()
            .flat_map(|c| {
                let (now, later): (Vec<_>, Vec<_>) =
                    std::mem::take(&mut c.pending).into_iter().partition(|p| p.0 <= f);
                c.pending = later;
                now.into_iter().map(|p| (c.id, p.1, p.2)).collect::<Vec<_>>()
            })
            .collect();
        for (id, m, fa) in due {
            self.change_motion(id, m.as_deref(), fa.as_deref(), false)?;
        }
        Ok(())
    }

    fn fade_color(&mut self, to: [f32; 4], from: Option<[f32; 4]>, frame: u32, seconds: f32) {
        self.fader_from = from.unwrap_or(self.fader);
        self.fader_to = to;
        self.fader_tween = Some(Tween {
            from: 0.0,
            to: 1.0,
            start: frame,
            frames: self.tb.frames_for(seconds),
            ease_out_quad: false,
        });
    }

    /// `TalkWindow.Close` (0.2 s) or `SetVisible(false)` (0.15 s).
    fn hide_talk_window(&mut self, frame: u32, seconds: f32) {
        if self.talk.is_some() {
            let a = self.talk_window.as_ref().map_or(1.0, |t| t.at(frame));
            self.talk_window = Some(Tween {
                from: a,
                to: 0.0,
                start: frame,
                frames: self.tb.frames_for(seconds),
                ease_out_quad: false,
            });
        }
    }

    fn play_bgm(&mut self, a: &AudioRef, frame: u32, fade: f32, volume: f32) {
        let fade_frames = self.tb.frames_for(fade);
        if let Some(prev) = self.bgm {
            let cue = &mut self.audio[prev];
            if cue.stop_frame.is_none() {
                cue.stop_frame = Some(frame + fade_frames);
                cue.fade_out = fade_frames;
            }
        }
        self.audio.push(AudioCue {
            files: a.files.iter().map(|f| f.0.clone()).collect(),
            start_frame: frame,
            stop_frame: None,
            looping: true,
            volume,
            fade_in: fade_frames,
            fade_out: 0,
            kind: AudioKind::Bgm,
        });
        self.bgm = Some(self.audio.len() - 1);
    }

    fn one_shot(&mut self, a: &AudioRef, frame: u32, volume: f32, kind: AudioKind) {
        self.audio.push(AudioCue {
            files: a.files.iter().map(|f| f.0.clone()).collect(),
            start_frame: frame,
            stop_frame: None,
            looping: false,
            volume,
            fade_in: 0,
            fade_out: 0,
            kind,
        });
    }

    fn sound(&mut self, ops: &[SoundOp], f: u32) {
        for op in ops {
            match op {
                SoundOp::Bgm { bgm, fade, volume, name } => match bgm {
                    Some(a) => {
                        let a = a.clone();
                        self.play_bgm(&a, f, *fade, *volume);
                    }
                    None => note(&mut self.notes, &format!("BGM {name} not found")),
                },
                SoundOp::SeOneShot { se, volume, deferred, name } => match se {
                    Some(a) => {
                        let a = a.clone();
                        let at = f + self.tb.frames_for(*deferred);
                        if *deferred > 0.0 {
                            note(&mut self.notes, "SE with Duration > 0: callback not reversed, played after the duration");
                        }
                        self.one_shot(&a, at, *volume, AudioKind::Se);
                    }
                    None => note(&mut self.notes, &format!("SE {name} not found")),
                },
                SoundOp::SeLoop { se, fade, volume, name } => {
                    if let Some(&i) = self.se_loops.get(name) {
                        self.audio[i].volume = *volume;
                        note(&mut self.notes, "SpecialSePlay volume fades are applied as steps");
                    } else if let Some(a) = se {
                        self.audio.push(AudioCue {
                            files: a.files.iter().map(|x| x.0.clone()).collect(),
                            start_frame: f,
                            stop_frame: None,
                            looping: true,
                            volume: *volume,
                            fade_in: self.tb.frames_for(*fade),
                            fade_out: 0,
                            kind: AudioKind::Se,
                        });
                        self.se_loops.insert(name.clone(), self.audio.len() - 1);
                    }
                }
                SoundOp::Stop { se, bgm, fade } => {
                    let frames = self.tb.frames_for(*fade);
                    if let Some(i) = self.se_loops.remove(se) {
                        self.audio[i].stop_frame = Some(f + frames);
                        self.audio[i].fade_out = frames;
                    } else if !bgm.is_empty()
                        && let Some(i) = self.bgm.take() {
                            self.audio[i].stop_frame = Some(f + frames);
                            self.audio[i].fade_out = frames;
                        }
                }
                SoundOp::BgmVolume { .. } | SoundOp::BgmAisacVolume { .. } | SoundOp::BgmBlock { .. } => {
                    note(&mut self.notes, "BGM volume / AISAC / block changes are not applied yet");
                }
                SoundOp::Nothing => {}
            }
        }
    }

    fn act(&mut self, instr: &Instr, f: u32) -> Result<(), BakeError> {
        let timing = &self.tl.instrs[&instr.index];
        match &instr.kind {
            InstrKind::Wait => {}
            InstrKind::Talk(t) => {
                // `TalkWindow.Open`: PlayActive(1, 0.2) from alpha 0, linear
                if timing.talk.as_ref().is_some_and(|tt| tt.opens_window) {
                    self.talk_window = Some(Tween {
                        from: 0.0,
                        to: 1.0,
                        start: f,
                        frames: self.tb.frames_for(consts::TALK_WINDOW_OPEN_CLOSE_DURATION),
                        ease_out_quad: false,
                    });
                }
                self.auto_since.get_or_insert(f);
                self.talk = Some((
                    instr.index,
                    TalkState {
                        name: t.display_name.clone(),
                        body: t.body.clone(),
                        visible: 0,
                        window_alpha: 1.0,
                        auto_time: 0.0,
                    },
                ));
                for s in &t.speakers {
                    if self.chars.contains_key(s) {
                        self.order.retain(|c| c != s);
                        self.order.push(*s);
                    }
                }
                let tt = timing.talk.clone().expect("talk timing");
                for v in &t.voices {
                    if let Some(a) = &v.voice {
                        self.one_shot(a, f, v.volume, AudioKind::Voice);
                        if t.lip_sync == LipSyncMode::Voice {
                            let pcm = lipsync::load_mono(self.lib, a)?;
                            if let Some(c) = self.chars.get_mut(&v.character) {
                                c.lip = lipsync::Lip::voice(pcm, f, v.character, t.target_value_scale);
                            }
                        }
                    }
                }
                if t.lip_sync == LipSyncMode::Text {
                    for s in &t.speakers {
                        if let Some(c) = self.chars.get_mut(s) {
                            c.lip = lipsync::Lip::text(tt.clone());
                        }
                    }
                }
                // the timeline's `talk_motion_schedule`; entries due now play in this frame
                for &(at, k) in &tt.motions {
                    let m = &t.motions[k];
                    if let Some(c) = self.chars.get_mut(&m.character) {
                        c.pending.push((at, m.motion.clone(), m.facial.clone()));
                    }
                }
                if let Some(e) = &t.attached_effect {
                    self.effect(instr.index, e, f);
                }
                if let Some(s) = &t.attached_sound {
                    self.sound(s, f);
                }
            }
            InstrKind::Layout(l) => match &l.op {
                LayoutOp::Appear { from, offset_x, costume, motion, facial, depth } => {
                    self.appear(
                        l.character,
                        *from,
                        *offset_x,
                        costume.as_deref(),
                        motion.as_deref(),
                        facial.as_deref(),
                        f,
                        true,
                    )?;
                    self.depth(l.character, *depth);
                }
                LayoutOp::Move { to, offset_x, duration } => {
                    let (x, y) = self.side_position(*to, *offset_x);
                    let frames = self.tb.frames_for(*duration);
                    if let Some(c) = self.chars.get_mut(&l.character) {
                        let cur = c.x.at(f);
                        c.x = Tween { from: cur, to: x, start: f, frames, ease_out_quad: false };
                        c.y = y;
                    }
                }
                LayoutOp::Hide { delay } => {
                    // FadeOpacityCoroutine: WaitForSeconds(delay) (>= 1 frame), then the fade
                    let start = f + self.tb.frames_for(*delay).max(1);
                    let frames = self.tb.frames_for(consts::CHARACTER_FADE_DURATION);
                    if let Some(c) = self.chars.get_mut(&l.character) {
                        let cur = c.opacity.at(f);
                        c.opacity = Tween { from: cur, to: 0.0, start, frames, ease_out_quad: false };
                        c.hide_at = Some(start + frames);
                    }
                }
                LayoutOp::Shake { .. } => note(&mut self.notes, "character shake not rendered"),
                LayoutOp::Depth { depth } => self.depth(l.character, *depth),
            },
            InstrKind::ChangeMotion { character, motion, facial } => {
                self.change_motion(*character, motion.as_deref(), facial.as_deref(), false)?;
            }
            InstrKind::Effect(e) => self.effect(instr.index, e, f),
            InstrKind::Sound { ops } => self.sound(ops, f),
            InstrKind::SetLayoutMode { mode } => {
                self.layout_mode = *mode;
                note(&mut self.notes, "layout mode changes apply to later placements only");
            }
            InstrKind::Unsupported(u) => {
                if let UnsupportedReason::Movie { name, files } = &u.reason {
                    // PlayMovie → SetHideUI(true) → RefreshTalkWindow → SetVisible(false)
                    self.hide_talk_window(f, consts::TALK_WINDOW_FADE_DURATION);
                    let video = files.iter().find(|x| x.0.ends_with(".m2v")).map(|x| x.0.clone());
                    self.movie = Some((name.clone(), video, f, timing.finish));
                    if let Some(w) = files.iter().find(|x| x.0.ends_with(".wav")) {
                        let a = AudioRef { cue: name.clone(), files: vec![w.clone()] };
                        self.one_shot(&a, f, 1.0, AudioKind::Movie);
                    }
                    // `PlayMovie` hides every appearing character first.
                    for c in self.chars.values_mut() {
                        c.visible = false;
                    }
                    note(&mut self.notes, "movies: video decoded with ffmpeg into the 2338×1080 movie rect (CRI Mana decoder not used)");
                } else {
                    note(&mut self.notes, &format!("unsupported snippet: {:?}", u.reason));
                }
            }
        }
        Ok(())
    }

    fn depth(&mut self, id: CharacterId, depth: DepthType) {
        match depth {
            DepthType::Front => {
                self.order.retain(|c| *c != id);
                self.order.push(id);
            }
            DepthType::Back => {
                self.order.retain(|c| *c != id);
                self.order.insert(0, id);
            }
            _ => {}
        }
    }

    fn finish(&mut self, instr: &Instr, f: u32) {
        if let InstrKind::Talk(t) = &instr.kind {
            // `OnFinishTalkWindow`: `if (placeInfo.IsActive) placeInfo.Hide()`
            let tb = self.tb;
            if let Some(p) = &mut self.place_info {
                p.hide(f, tb);
            }
            for c in self.chars.values_mut() {
                c.lip.end_text(f);
            }
            if t.close_window_on_finish {
                self.hide_talk_window(f, consts::TALK_WINDOW_OPEN_CLOSE_DURATION);
            }
        }
    }

    fn effect(&mut self, index: u32, e: &Effect, f: u32) {
        let timing = &self.tl.instrs[&index];
        let d = e.duration;
        match &e.op {
            EffectOp::Fade { color, dir } => {
                let rgb = match color {
                    FadeColor::Black => [0.0, 0.0, 0.0],
                    FadeColor::White => [1.0, 1.0, 1.0],
                };
                match dir {
                    // In: reveal from an opaque colour
                    Direction::In => self.fade_color(
                        [rgb[0], rgb[1], rgb[2], 0.0],
                        Some([rgb[0], rgb[1], rgb[2], 1.0]),
                        f,
                        d,
                    ),
                    // Out: from the current colour to opaque
                    Direction::Out => {
                        let from = if self.fader[3] == 0.0 {
                            [rgb[0], rgb[1], rgb[2], 0.0]
                        } else {
                            self.fader
                        };
                        self.fade_color([rgb[0], rgb[1], rgb[2], 1.0], Some(from), f, d);
                    }
                }
            }
            EffectOp::ChangeBackground { background, .. } => {
                let new = background.as_ref().map(|b| b.0.clone());
                self.background.previous = self.background.current.take();
                self.background.current = new;
                let frames = self.tb.frames_for(d);
                self.bg_tween = Some(Tween { from: 0.0, to: 1.0, start: f, frames, ease_out_quad: false });
            }
            EffectOp::Telop { text } => {
                let clip = self.tb.frames_for(consts::TELOP_ANIM_CLIP_LENGTH);
                self.telop.push((text.clone(), f, timing.finish.saturating_sub(clip), timing.finish));
            }
            EffectOp::PlaceInfo { text } => {
                // `ScenarioPlaceInfo.Show`: from the current x (after `Reset`, DefaultPosX) to 0
                let hidden_x = consts::place_info_hidden_x(self.opts.content_size);
                let from = self.place_info.as_ref().map_or(hidden_x, |p| p.x_at(f, self.tb));
                self.place_info = Some(PlaceInfoRt {
                    text: text.clone(),
                    from,
                    to: 0.0,
                    start: f,
                    status: PlaceStatus::SlideIn,
                    reserve_close: false,
                    hidden_x,
                });
            }
            EffectOp::FullScreenText { text, voice, .. } => {
                let Some(t) = timing.full_screen_text.clone() else { return };
                if t.first {
                    self.cinemascope.push((f, true));
                }
                if let Some(fade) = t.fade_start {
                    self.cinemascope.push((fade, false));
                }
                if let Some(a) = voice {
                    let a = a.clone();
                    self.one_shot(&a, t.text_start, 1.0, AudioKind::Voice);
                }
                self.full_text.push(FstRun { text: text.clone(), end: timing.finish, timing: t });
            }
            EffectOp::Blur { dir } => {
                let (from, to) = match dir {
                    Direction::In => (self.blur_value, 1.0),
                    Direction::Out => (self.blur_value, 0.0),
                };
                self.blur = Some(Tween { from, to, start: f, frames: self.tb.frames_for(d), ease_out_quad: true });
            }
            EffectOp::CameraColor { effect } => {
                self.camera_color = Some(match effect {
                    CameraColorEffect::Sepia => CameraColor {
                        mono: [0.298_912, 0.586_611, 0.114_478, 1.0],
                        tone: [1.07, 0.74, 0.43, 1.0],
                        influence: 0.75,
                    },
                    CameraColorEffect::Flashback => CameraColor {
                        mono: [0.5, 0.5, 0.5, 1.0],
                        tone: [0.5, 0.5, 0.5, 1.0],
                        influence: 0.5,
                    },
                });
            }
            EffectOp::CameraColorOff => self.camera_color = None,
            EffectOp::Ambient { color } => {
                self.ambient = match color {
                    AmbientColor::Afternoon => consts::MODEL_COLOR_NORMAL,
                    AmbientColor::Evening => consts::MODEL_COLOR_EVENING,
                    AmbientColor::Night => consts::MODEL_COLOR_NIGHT,
                };
            }
            EffectOp::SekaiTransition { dir, .. } => {
                let (delay, from, to) = match dir {
                    Direction::In => (consts::SEKAI_IN_FADE_DELAY, [1.0, 1.0, 1.0, 1.0], [1.0, 1.0, 1.0, 0.0]),
                    Direction::Out => (consts::SEKAI_OUT_FADE_DELAY, self.fader, [1.0, 1.0, 1.0, 1.0]),
                };
                if *dir == Direction::In {
                    self.fader = from;
                    self.fader_tween = None;
                }
                let start = f + self.tb.frames_for(delay);
                self.fade_color(to, Some(if *dir == Direction::Out && from[3] == 0.0 { [1.0, 1.0, 1.0, 0.0] } else { from }), start, d);
                // Case 21 / 41 instantiate `fx_transition_scenario` under `effectLayer`;
                // case 20 / 40 only drive the fader. The copy is destroyed after 5 s.
                if *dir == Direction::Out {
                    self.fx = Some((f, self.tb.frames_for(consts::FX_LIFETIME)));
                }
                note(&mut self.notes, "Sekai transition particles: fx_transition_scenario simulated from the prefab modules (noise approximated)");
            }
            // `SnippetActionSpecialEffect` case 5: `scenarioRoot.DOShakePosition(Duration, 10, 16,
            // 90, false, true)`; `Duration == INFINITY_DURATION` goes to `ScreenShakeInfinity`
            // (3600 s, no fade-out) until StopShakeScreen kills it.
            EffectOp::ShakeScreen => {
                let fade = d != consts::INFINITY_DURATION;
                let fps = self.tb.fps() as f32;
                self.screen_shake = Some(shake::Shake::new(&mut self.rng, f, fps, d, 10.0, 16, 90.0, false, fade));
            }
            // case 6: `TalkWindow.ShakeWindow` = `windowRectTransform.DOShakeAnchorPos(Duration,
            // 10, 16, 90, false, true)`
            EffectOp::ShakeWindow => {
                let fps = self.tb.fps() as f32;
                self.window_shake = Some(shake::Shake::new(&mut self.rng, f, fps, d, 10.0, 16, 90.0, true, true));
            }
            // cases 25 / 26: kill the tween; `OnFinishShake*` puts the base position back
            // cases 29-36 → `ScenarioSideFadePlayer.Play(FadeType, Duration)` (JP 6.8.1 switch:
            // 29→LeftIn 1, 30→LeftOut 0, 31→RightIn 3, 32→RightOut 2, 33→TopIn 5, 34→TopOut 4,
            // 35→BottomIn 7, 36→BottomOut 6). `Play` sets `anchoredPosition` to `from` and
            // `DOAnchorPos(to, Duration)` (OutQuad); the *In types deactivate it at the end.
            // W/H = `ScreenManager.contentSize`.
            EffectOp::SideFade { effect_type } => {
                let [w, h] = self.opts.content_size;
                let (ox, oy) = (w + 512.0, h + 512.0);
                let (from, to, hide) = match effect_type {
                    29 => ([0.0, 0.0], [ox, 0.0], true),
                    30 => ([-ox, 0.0], [0.0, 0.0], false),
                    31 => ([0.0, 0.0], [-ox, 0.0], true),
                    32 => ([ox, 0.0], [0.0, 0.0], false),
                    33 => ([0.0, 0.0], [0.0, -oy], true),
                    34 => ([0.0, oy], [0.0, 0.0], false),
                    35 => ([0.0, 0.0], [0.0, oy], true),
                    _ => ([0.0, -oy], [0.0, 0.0], false),
                };
                self.side_fade = Some(SideFadeRt { from, to, start: f, frames: self.tb.frames_for(d), hide });
            }
            EffectOp::StopShakeScreen => self.screen_shake = None,
            EffectOp::StopShakeWindow => self.window_shake = None,
            EffectOp::Noop => {}
            other => note(&mut self.notes, &format!("effect not rendered: {other:?}")),
        }
    }

    /// `PlayCinemascope`: DOTween (default ease OutQuad) over `playDuration`.
    fn cinemascope_at(&self, f: u32) -> f32 {
        let Some(&(start, show)) = self.cinemascope.iter().rev().find(|(s, _)| *s <= f) else {
            return 0.0;
        };
        let n = self.tb.frames_for(consts::FST_PLAY_DURATION).max(1);
        let t = ((f - start) as f32 / n as f32).min(1.0);
        let e = -t * (t - 2.0);
        if show { e } else { 1.0 - e }
    }

    fn snapshot(&mut self, f: u32) -> FrameState {
        if let Some(t) = &self.bg_tween {
            self.background.mix = t.at(f);
            if f >= t.start + t.frames {
                self.background.previous = None;
                self.bg_tween = None;
            }
        }
        if let Some(t) = &self.fader_tween {
            let k = t.at(f);
            for i in 0..4 {
                self.fader[i] = self.fader_from[i] + (self.fader_to[i] - self.fader_from[i]) * k;
            }
        }
        if let Some(b) = &self.blur {
            self.blur_value = b.at(f);
        }
        let mut characters = Vec::new();
        for id in &self.order {
            let Some(c) = self.chars.get_mut(id) else { continue };
            if c.hide_at.is_some_and(|h| f >= h) {
                c.visible = false;
                c.hide_at = None;
            }
            if !c.visible {
                continue;
            }
            characters.push(CharacterState {
                character: c.id,
                model: c.model,
                opacity: c.opacity.at(f),
                x: c.x.at(f),
                y: c.y,
                scale: c.scale,
                color: self.ambient,
                params: c.values.clone(),
            });
        }
        let talk = self.talk.as_ref().map(|(index, t)| {
            let visible = self.tl.instrs[index].talk.as_ref().map_or(0, |tt| tt.visible_at(f));
            TalkState {
                name: t.name.clone(),
                body: t.body.clone(),
                visible,
                window_alpha: self.talk_window.as_ref().map_or(1.0, |w| w.at(f)),
                auto_time: self.auto_since.map_or(0.0, |s| f.saturating_sub(s) as f32 * self.tb.delta()),
            }
        });
        let movie = match &self.movie {
            Some((name, file, start, end)) if f < *end => Some(MovieState {
                name: name.clone(),
                file: file.clone(),
                time: (f - start) as f32 * self.tb.delta(),
            }),
            _ => None,
        };
        FrameState {
            background: self.background.clone(),
            characters,
            fader: self.fader,
            blur: self.blur_value,
            camera_color: self.camera_color,
            talk: talk.filter(|t| t.window_alpha > 0.0),
            telop: self.telop.iter().find(|t| f >= t.1 && f < t.3).map(|(text, start, hide, _)| TelopState {
                text: text.clone(),
                show: (f - start) as f32 * self.tb.delta(),
                hide: (f >= *hide).then(|| (f - hide) as f32 * self.tb.delta()),
            }),
            place_info: self.place_info.as_mut().and_then(|p| p.state(f, self.tb)),
            full_screen_text: self.full_text.iter().find_map(|b| b.state(f, self.tb)),
            cinemascope: self.cinemascope_at(f),
            menu_alpha: if movie.is_some() { 0.0 } else { 1.0 },
            movie,
            fx: self.fx.and_then(|(start, frames)| {
                (f >= start && f < start + frames)
                    .then_some(sse_params::FxState { age_frames: f - start, seed: start.wrapping_mul(2654435761) })
            }),
            scenario_shake: self.screen_shake.as_ref().map_or([0.0, 0.0], |s| s.at(f)),
            window_shake: self.window_shake.as_ref().map_or([0.0, 0.0], |s| s.at(f)),
            side_fade: self.side_fade.as_ref().and_then(|s| s.at(f)),
        }
    }
}

impl Tween {
    fn fixed(v: f32) -> Self {
        Tween { from: v, to: v, start: 0, frames: 0, ease_out_quad: false }
    }
}
