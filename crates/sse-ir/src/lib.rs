//! # sse-ir -- intermediate representation
//!
//! The first seam of the project: the contract between the parsing layer and the
//! compilation layer. Format spec: `docs/spec/ir.md` (ADR-0008).
//!
//! ## Responsibilities
//! - IR data structures, JSON debug serialisation, and the version field
//! - Representation of unsupported content that still takes part in scheduling (ADR-0008)
//!
//! ## Not responsible for
//! - Parsing `ScenarioSceneData` (that is `sse-scenario`)
//! - Any time computation (that is `sse-timeline`)
//!
//! ## Allowed dependencies
//! `sse-core` only.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const IR_VERSION: u32 = 4;

pub type CharacterId = i32;

/// A `library/`-relative path from the ripper episode index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LibPath(pub String);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub ir_version: u32,
    pub source: EpisodeSource,
    pub cast: BTreeMap<CharacterId, CastEntry>,
    pub initial: InitialState,
    pub root: Block,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeSource {
    pub selector: String,
    pub title: String,
    /// masterdata `scenarioId` (ADR-0007), never the JSON's own `ScenarioId`.
    pub scenario_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CastEntry {
    pub asset_name: Option<String>,
    /// Costume the character starts in (`AppearCharacters`, first appearance).
    pub initial_costume: Option<String>,
    pub costumes: BTreeMap<String, CostumeEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostumeEntry {
    /// `library/<bundle>` of the model; `None` = the game fails to load it too (P6).
    pub model: Option<LibPath>,
    /// Motion / facial name as written in the scenario → `sse-motion` file.
    pub motions: BTreeMap<String, LibPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitialState {
    pub background: Option<LibPath>,
    pub bgm: Option<AudioRef>,
    pub layout_mode: LayoutMode,
    pub layout: Vec<InitialPlacement>,
    pub music_video: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitialPlacement {
    pub character: CharacterId,
    pub side: Side,
    pub offset_x: f32,
    pub costume: Option<String>,
    pub motion: Option<String>,
    pub facial: Option<String>,
}

/// One ACB cue, decoded to waveforms by the ripper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioRef {
    pub cue: String,
    pub files: Vec<LibPath>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Block {
    pub nodes: Vec<Node>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum Node {
    Instr(Instr),
    /// Only `Action = 5` produces a branch (ADR-0008); its semantics are not implemented yet.
    Branch(Branch),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub index: u32,
    pub arms: Vec<Block>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Progress {
    Now,
    WaitFinished,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instr {
    /// `Snippets[]` position.
    pub index: u32,
    pub progress: Progress,
    /// Seconds, before the action takes effect; `<= 0` costs no frame.
    pub delay: f32,
    pub delay_tap_skippable: bool,
    pub kind: InstrKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstrKind {
    Wait,
    Talk(Talk),
    Layout(Layout),
    ChangeMotion {
        character: CharacterId,
        motion: Option<String>,
        facial: Option<String>,
    },
    Effect(Effect),
    /// One `SoundData` entry; CrossFade/Stack may start a BGM and an SE at once.
    Sound {
        ops: Vec<SoundOp>,
    },
    SetLayoutMode {
        mode: LayoutMode,
    },
    Unsupported(Unsupported),
}

// ---- talk --------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LipSyncMode {
    Text,
    Voice,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionChangeFactor {
    Text,
    PlayTime,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Talk {
    pub speakers: Vec<CharacterId>,
    pub display_name: String,
    /// Final text: `{{playerName}}` already replaced (ADR-0008); rich-text tags kept.
    pub body: String,
    pub lip_sync: LipSyncMode,
    pub motion_change: MotionChangeFactor,
    pub motions: Vec<TalkMotion>,
    pub voices: Vec<TalkVoice>,
    pub close_window_on_finish: bool,
    pub target_value_scale: f32,
    pub attached_effect: Option<Box<Effect>>,
    pub attached_sound: Option<Vec<SoundOp>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalkMotion {
    pub character: CharacterId,
    pub motion: Option<String>,
    pub facial: Option<String>,
    pub timing_sync_value: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TalkVoice {
    pub character: CharacterId,
    pub voice_id: String,
    /// `None` when no ACB carries the cue (the game plays nothing).
    pub voice: Option<AudioRef>,
    pub volume: f32,
}

// ---- layout ------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    Default,
    Three,
}

/// `ScenarioCharacterLayout.Side`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    None,
    Left,
    LeftOver,
    LeftInside,
    Center,
    Right,
    RightOver,
    RightInside,
    LeftUnder,
    LeftInsideUnder,
    CenterUnder,
    RightUnder,
    RightInsideUnder,
}

impl Side {
    pub fn from_i32(v: i32) -> Option<Self> {
        use Side::*;
        Some(match v {
            0 => None,
            1 => Left,
            2 => LeftOver,
            3 => LeftInside,
            4 => Center,
            5 => Right,
            6 => RightOver,
            7 => RightInside,
            8 => LeftUnder,
            9 => LeftInsideUnder,
            10 => CenterUnder,
            11 => RightUnder,
            12 => RightInsideUnder,
            _ => return Option::None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepthType {
    None,
    Front,
    Back,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub character: CharacterId,
    pub op: LayoutOp,
    /// `SnippetActionCharacterLayout` changes the body motion and the facial before it branches
    /// on the layout type, so every type can carry them. `Appear` keeps its own in the op.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facial: Option<String>,
    /// `CheckAndChangeCostume` (every type but `Appear`, which keeps its own): a different
    /// costume hides the character and swaps its model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub costume: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LayoutOp {
    /// Only `SideTo` is used; the game ignores `SideFrom` when moving.
    Move {
        to: Side,
        offset_x: f32,
        duration: f32,
    },
    Appear {
        from: Side,
        offset_x: f32,
        costume: Option<String>,
        motion: Option<String>,
        facial: Option<String>,
        depth: DepthType,
    },
    /// `HideCharacter(id, delay)`: the 0.1 s fade starts after `delay` seconds
    /// (`SnippetActionCharacterLayout`). In place (`SideFrom == SideTo`) the delay is
    /// 0.15 s; otherwise it is the move duration minus 0.1 s.
    Hide {
        delay: f32,
    },
    /// Types 4 / 5 (`SetCharacterShake`): `DOShakePosition` on the model view's
    /// `shakeTargetObject`, an empty child ("shake") that nothing renders, so nothing moves on
    /// screen; the snippet finishes after `duration` (MoveSpeedType 0: 0.5 s, 1: 0.75 s,
    /// else 0.25 s).
    Shake {
        axis: Axis,
        duration: f32,
        raw: serde_json::Value,
    },
    Depth {
        depth: DepthType,
    },
}

// ---- effects -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FadeColor {
    Black,
    White,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmbientColor {
    Afternoon,
    Evening,
    Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraColorEffect {
    Sepia,
    Flashback,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Raw `EffectType`.
    pub effect_type: i32,
    pub duration: f32,
    pub op: EffectOp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EffectOp {
    Fade {
        color: FadeColor,
        dir: Direction,
    },
    ShakeScreen,
    ShakeWindow,
    StopShakeScreen,
    StopShakeWindow,
    ChangeBackground {
        background: Option<LibPath>,
        name: String,
    },
    Telop {
        text: String,
    },
    PlaceInfo {
        text: String,
    },
    FullScreenText {
        text: String,
        voice_id: String,
        voice: Option<AudioRef>,
    },
    CameraColor {
        effect: CameraColorEffect,
    },
    CameraColorOff,
    Blur {
        dir: Direction,
    },
    /// Effect 45 (JP): `DollyZoomParams` from "Zoom: z, Blur: b, Dist: d" and the DOTween
    /// ease name in `StringValSub`. Without `zoom` the game logs an error and does nothing.
    DollyZoom {
        zoom: Option<f32>,
        blur: Option<f32>,
        dist: Option<f32>,
        ease: String,
    },
    BackgroundBlur {
        on: bool,
    },
    CameraMove {
        x: f32,
        y: f32,
        valid: bool,
    },
    CameraZoom {
        scale: f32,
    },
    Ambient {
        color: AmbientColor,
    },
    PlayScenarioEffect {
        name: String,
        bundle: String,
    },
    StopScenarioEffect {
        name: String,
    },
    CharacterShader {
        character: CharacterId,
        shader: String,
        bundle: String,
    },
    SekaiTransition {
        anniversary: bool,
        dir: Direction,
    },
    SideFade {
        effect_type: i32,
    },
    SimpleSelectable {
        options: Vec<String>,
    },
    /// Empty handler in 6.4.0 (EffectType 0 / 11 / 17): only `FinishSnippet`.
    Noop,
}

// ---- sound -------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum SoundOp {
    /// PlayMode CrossFade / Stack with a BGM: crossfade (`Duration` or 0.25 s).
    Bgm {
        name: String,
        bgm: Option<AudioRef>,
        fade: f32,
        volume: f32,
    },
    SeOneShot {
        name: String,
        se: Option<AudioRef>,
        volume: f32,
        /// `Duration > 0` path of the game (6 instances in the corpus): callback not reversed.
        deferred: f32,
    },
    SeLoop {
        name: String,
        se: Option<AudioRef>,
        fade: f32,
        volume: f32,
    },
    Stop {
        se: String,
        bgm: String,
        fade: f32,
    },
    BgmVolume {
        volume: f32,
        duration: f32,
    },
    BgmAisacVolume {
        value: f32,
    },
    BgmBlock {
        index: i32,
    },
    /// Nothing to do (e.g. CrossFade with neither BGM nor SE).
    Nothing,
}

// ---- unsupported / diagnostics -----------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum UnsupportedReason {
    InputName,
    Selectable,
    Movie { name: String, files: Vec<LibPath> },
    MusicVideo,
    UnknownEffectType { value: i32 },
    UnknownAction { value: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum FinishRule {
    Immediate,
    /// Wait this many seconds after the delay (best available approximation).
    After {
        seconds: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Unsupported {
    #[serde(flatten)]
    pub reason: UnsupportedReason,
    pub finish: FinishRule,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "diagnostic", rename_all = "snake_case")]
pub enum Diagnostic {
    IgnoredField {
        index: u32,
        field: String,
        value: String,
    },
    IndexMismatch {
        index: u32,
        snippet_index: i64,
    },
    ScenarioIdMismatch {
        json: String,
        masterdata: String,
    },
    RipperWarning {
        kind: String,
        detail: String,
    },
    ApproximateTiming {
        index: u32,
        reason: String,
    },
    UnknownEnumValue {
        index: u32,
        field: String,
        value: i64,
    },
    MissingAsset {
        index: u32,
        what: String,
    },
}

impl Episode {
    /// All instructions in document order, descending into the first arm of branches
    /// (the default flattening, ADR-0008).
    pub fn instrs(&self) -> Vec<&Instr> {
        fn walk<'a>(b: &'a Block, out: &mut Vec<&'a Instr>) {
            for n in &b.nodes {
                match n {
                    Node::Instr(i) => out.push(i),
                    Node::Branch(br) => {
                        if let Some(arm) = br.arms.first() {
                            walk(arm, out);
                        }
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }
}
