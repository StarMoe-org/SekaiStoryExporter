//! # sse-params -- the parameter table (Pass 1 → Pass 2 seam, ADR-0004)
//!
//! Plain data describing the complete visual and audio state of every frame. `sse-bake`
//! writes it; `sse-render` and `sse-export` read it. Living in its own crate keeps the
//! iron rule structural: the renderer depends on this data, never on the timeline or the
//! scenario. Format notes: `docs/spec/param-table.md`.
//!
//! ## Allowed dependencies
//! `sse-core` only.

use serde::{Deserialize, Serialize};

pub const PARAM_TABLE_VERSION: u32 = 5;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamTable {
    pub version: u32,
    pub fps: u32,
    /// Models referenced by `CharacterState::model` (library-relative bundle dirs).
    pub models: Vec<String>,
    pub frames: Vec<FrameState>,
    pub audio: Vec<AudioCue>,
    /// `SoundData` PlayMode 4: tweens of the `BGM` category AISAC `VOL_BGM_SCE` (a linear
    /// 0–1 volume graph in the client's ACF, default 1), in start order.
    #[serde(default)]
    pub bgm_volume: Vec<VolumeTween>,
    /// Approximations and unsupported content, for the export report.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrameState {
    pub background: BackgroundState,
    /// In draw order (back to front).
    pub characters: Vec<CharacterState>,
    /// `ColorFader` overlay colour, straight alpha.
    pub fader: [f32; 4],
    /// Camera blur amount in [0, 1].
    pub blur: f32,
    /// Monotone post effect (`influence`, `tone`, `mono`) when attached.
    pub camera_color: Option<CameraColor>,
    pub talk: Option<TalkState>,
    pub telop: Option<TelopState>,
    pub place_info: Option<PlaceInfoState>,
    pub full_screen_text: Option<FullScreenTextState>,
    /// `ScenarioFullScreenTextDialog` cinemascope: bar height and `Base` alpha, as the
    /// eased fraction of the shown state (0 = hidden).
    pub cinemascope: f32,
    /// Scenario menu button (`UIPartsMenuButton`) opacity.
    pub menu_alpha: f32,
    /// `PlayMovie`: the movie layer covers the scenario.
    pub movie: Option<MovieState>,
    /// `fx_transition_scenario` instance (`SnippetActionSpecialEffect` cases 21 / 41).
    pub fx: Option<FxState>,
    /// ShakeScreen offset of `ScenarioRoot`, reference-canvas pixels, +y up.
    #[serde(default)]
    pub scenario_shake: [f32; 2],
    /// ShakeWindow offset of the talk window, reference-canvas pixels, +y up.
    #[serde(default)]
    pub window_shake: [f32; 2],
    /// Scenario effect prefabs (`PlayScenarioEffect`) alive this frame, oldest first.
    #[serde(default)]
    pub effects: Vec<EffectState>,
    /// `ScenarioStudio` camera and `scenarioRoot` scale (effects 42 / 43): the view is
    /// offset by `[x, y]` reference-canvas pixels (+y up) and `scenarioRoot` (background,
    /// characters, effects) scaled by `zoom` about the screen centre. `None` = identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraView>,
    /// `ScenarioSideFadePlayer` while active: its `anchoredPosition` (reference-canvas pixels,
    /// +y up; zero = covering the screen).
    #[serde(default)]
    pub side_fade: Option<[f32; 2]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraView {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl CameraView {
    /// A point of `scenarioRoot` (reference-canvas pixels from the centre, +y up) on screen,
    /// given the root's shake offset.
    pub fn apply(view: Option<&CameraView>, shake: [f32; 2], p: [f32; 2]) -> [f32; 2] {
        match view {
            Some(v) => [
                shake[0] + v.zoom * p[0] - v.x,
                shake[1] + v.zoom * p[1] - v.y,
            ],
            None => [shake[0] + p[0], shake[1] + p[1]],
        }
    }
}

/// One `PlayScenarioEffect` instance: the prefab `name` from `bundle`, instantiated
/// `age_frames` ago; `stop_age` = age at which `StopScenarioEffect` stopped it. The renderer
/// re-simulates (or steps its cached copy) up to `age_frames`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectState {
    pub bundle: String,
    pub name: String,
    pub age_frames: u32,
    pub stop_age: Option<u32>,
    /// Instance identity and random seed (the instantiation frame).
    pub seed: u32,
    /// `ScenarioModelView.AttachModelScenarioEffect`: the prefab hangs off this character's
    /// model view (and moves and scales with it) instead of `effectLayer`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<i32>,
}

/// One live `fx_transition_scenario` copy. The prefab hangs off `ScenarioPlayer.effectLayer`
/// for `DestroyAtTime.deleteAtTime` = 5 s; the renderer re-simulates `age_frames` steps from
/// the start, so its output stays a pure function of the frame.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FxState {
    /// Frames since the prefab was instantiated.
    pub age_frames: u32,
    /// Seeds the emitters with `autoRandomSeed`; the game randomises those per instance, so
    /// ours is derived from the instance and only has to be reproducible.
    pub seed: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackgroundState {
    pub current: Option<String>,
    /// Crossfade source and the weight of `current` in [0, 1].
    pub previous: Option<String>,
    pub mix: f32,
    /// `backgroundImage.material` = `Materials/UI/UIGaussianBlur` (effect 44 "true").
    #[serde(default)]
    pub blur: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterState {
    pub character: i32,
    /// Index into `ParamTable::models`.
    pub model: usize,
    pub opacity: f32,
    /// Bottom-centre anchor offset in UI units (reference 1920×1080 space).
    pub x: f32,
    pub y: f32,
    /// Layout-mode scale (`baseScale.x`).
    pub scale: f32,
    /// Ambient model colour (multiplied in).
    pub color: [f32; 4],
    /// Final Cubism parameter values, in the model's parameter order.
    pub params: Vec<f32>,
    /// `Live2DHologramController` on the model's `RawImage` (character shader "hologram" /
    /// "monitor").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hologram: Option<HologramState>,
    /// `Live2DBlurController` on the `RawImage` (character shader "blur"): `_Blur`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blur: Option<f32>,
}

/// The `Live2D/Materials/Live2DHologram` instance's animated values this frame.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HologramState {
    /// `_Line`: scan-line threshold.
    pub line: f32,
    /// `_SubColor.a`: output alpha factor.
    pub alpha: f32,
    /// Shader `_Time.y` (seconds since the scene loaded; scrolls `_SubTex`).
    pub time: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CameraColor {
    pub mono: [f32; 4],
    pub tone: [f32; 4],
    pub influence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TalkState {
    pub name: String,
    pub body: String,
    /// UTF-16 units of `body` visible (`Substring(0, n)`).
    pub visible: u32,
    pub window_alpha: f32,
    /// Seconds since the auto signal was enabled (first talk of the episode); drives the
    /// `TweenAlpha` blink of its icon.
    pub auto_time: f32,
}

/// `ScenarioTelopPlayer`: seconds into the show clip, and into the hide clip once it runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelopState {
    pub text: String,
    pub show: f32,
    pub hide: Option<f32>,
}

/// `ScenarioPlaceInfo`: `anchoredPosition.x` of the panel (reference pixels).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaceInfoState {
    pub text: String,
    pub x: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovieState {
    pub name: String,
    /// Library path of the video elementary stream (`.m2v`), when the ripper produced one.
    pub file: Option<String>,
    /// Seconds since playback started.
    pub time: f32,
}

/// `TextAppearFade`: character `i` (index in TMP's character list, line feeds included) has
/// alpha `clamp(progress - i, 0, 1) × alpha`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullScreenTextState {
    pub text: String,
    pub progress: f32,
    /// `FadeOutAll` multiplier.
    pub alpha: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerState {
    pub text: String,
    pub alpha: f32,
}

/// A DOTween (ease OutQuad) of a volume from `from` to `to` over `frames`, from `start`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VolumeTween {
    pub start: u32,
    pub frames: u32,
    pub from: f32,
    pub to: f32,
}

impl VolumeTween {
    fn value(&self, frame: f64) -> f32 {
        if self.frames == 0 {
            return self.to;
        }
        let t = ((frame - f64::from(self.start)) / f64::from(self.frames)).clamp(0.0, 1.0) as f32;
        self.from + (self.to - self.from) * (t * (2.0 - t))
    }

    /// The volume at `frame` (fractional) given the tweens in start order; 1 before any.
    pub fn at(tweens: &[VolumeTween], frame: f64) -> f32 {
        tweens
            .iter()
            .rev()
            .find(|t| f64::from(t.start) <= frame)
            .map_or(1.0, |t| t.value(frame))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioCue {
    /// Library-relative waveform files (played together).
    pub files: Vec<String>,
    pub start_frame: u32,
    /// Exclusive; `None` = until the file ends (or loop end).
    pub stop_frame: Option<u32>,
    pub looping: bool,
    pub volume: f32,
    /// Linear fades, in frames.
    pub fade_in: u32,
    pub fade_out: u32,
    pub kind: AudioKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioKind {
    Bgm,
    Se,
    Voice,
    Movie,
}

#[cfg(test)]
mod volume_tests {
    use super::*;

    #[test]
    fn bgm_volume_tweens_ease_out_quad_from_the_current_value() {
        let t = [
            VolumeTween {
                start: 10,
                frames: 20,
                from: 1.0,
                to: 0.0,
            },
            VolumeTween {
                start: 20,
                frames: 0,
                from: 0.75,
                to: 0.5,
            },
        ];
        assert_eq!(VolumeTween::at(&t, 0.0), 1.0);
        assert!((VolumeTween::at(&t, 15.0) - (1.0 - 0.4375)).abs() < 1e-6);
        assert_eq!(VolumeTween::at(&t, 25.0), 0.5);
    }
}
