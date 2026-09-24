//! # sse-params -- the parameter table (Pass 1 → Pass 2 seam, decision Q14)
//!
//! Plain data describing the complete visual and audio state of every frame. `sse-bake`
//! writes it; `sse-render` and `sse-export` read it. Living in its own crate keeps the
//! iron rule structural: the renderer depends on this data, never on the timeline or the
//! scenario. Format notes: `docs/spec/param-table.md`.
//!
//! ## Allowed dependencies
//! `sse-core` only.

use serde::{Deserialize, Serialize};

pub const PARAM_TABLE_VERSION: u32 = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamTable {
    pub version: u32,
    pub fps: u32,
    /// Models referenced by `CharacterState::model` (library-relative bundle dirs).
    pub models: Vec<String>,
    pub frames: Vec<FrameState>,
    pub audio: Vec<AudioCue>,
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
