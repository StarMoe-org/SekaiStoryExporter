//! # sse-scenario -- scenario parsing
//!
//! ## Responsibilities
//! - `ScenarioSceneData` (+ the ripper episode index) -> IR (`docs/spec/ir.md`)
//! - Version adaptation of enums and fields (CN 6.4.0)
//!
//! ## Not responsible for
//! - Timeline computation or branch flattening
//!
//! ## Allowed dependencies
//! `sse-core`, `sse-ir`, `sse-assets`.

mod dotnet;
pub mod raw;

use std::collections::BTreeMap;

use sse_assets::{EpisodeIndex, Library};
use sse_core::consts;
use sse_ir::*;

pub use dotnet::{bool_try_parse, single_try_parse};

#[derive(Debug, thiserror::Error)]
pub enum ScenarioError {
    #[error(transparent)]
    Asset(#[from] sse_assets::AssetError),
    #[error("snippet {index}: {what} reference {reference} is out of range")]
    BadReference {
        index: u32,
        what: &'static str,
        reference: i64,
    },
}

pub type Result<T> = std::result::Result<T, ScenarioError>;

#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Replacement for `{{playerName}}` (decision Q39).
    pub player_name: String,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            player_name: "「世界」的居民".to_owned(),
        }
    }
}

/// Parses the episode's scenario into IR.
pub fn parse_episode(lib: &Library, index: &EpisodeIndex, opts: &ParseOptions) -> Result<Episode> {
    let scene: raw::SceneData = lib.read_json(&index.scenario.path)?;
    Parser {
        lib,
        index,
        opts,
        diags: Vec::new(),
    }
    .run(scene)
}

struct Parser<'a> {
    lib: &'a Library,
    index: &'a EpisodeIndex,
    opts: &'a ParseOptions,
    diags: Vec<Diagnostic>,
}

fn opt(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

impl Parser<'_> {
    fn run(mut self, scene: raw::SceneData) -> Result<Episode> {
        let index = self.index;
        if !scene.scenario_id.is_empty() && scene.scenario_id != index.story.scenario_id {
            self.diags.push(Diagnostic::ScenarioIdMismatch {
                json: scene.scenario_id.clone(),
                masterdata: index.story.scenario_id.clone(),
            });
        }
        for w in &index.warnings {
            self.diags.push(Diagnostic::RipperWarning {
                kind: format!("{:?}", w.kind),
                detail: w.detail.clone(),
            });
        }
        if !scene.first_aisac_value.is_empty() {
            self.diags.push(Diagnostic::IgnoredField {
                index: 0,
                field: "FirstAisacValue".into(),
                value: scene.first_aisac_value.clone(),
            });
        }

        let cast = self.cast(&scene);
        let initial = InitialState {
            background: self.background(&scene.first_background, ""),
            bgm: self.audio_bgm(&scene.first_bgm),
            layout_mode: layout_mode(scene.first_character_layout_mode),
            layout: scene
                .first_layout
                .iter()
                .map(|l| InitialPlacement {
                    character: l.character2d_id,
                    side: Side::from_i32(l.position_side).unwrap_or(Side::None),
                    offset_x: l.offset_x,
                    costume: opt(&l.costume_type),
                    motion: opt(&l.motion_name),
                    facial: opt(&l.facial_name),
                })
                .collect(),
            music_video: opt(&scene.episode_music_video_id),
        };

        let mut nodes = Vec::with_capacity(scene.snippets.len());
        for (pos, snippet) in scene.snippets.iter().enumerate() {
            let pos = pos as u32;
            if snippet.index != i64::from(pos) {
                self.diags.push(Diagnostic::IndexMismatch {
                    index: pos,
                    snippet_index: snippet.index,
                });
            }
            nodes.push(self.snippet(&scene, pos, snippet)?);
        }

        Ok(Episode {
            ir_version: IR_VERSION,
            source: EpisodeSource {
                selector: index.story.selector.clone(),
                title: index.story.title.clone(),
                scenario_id: index.story.scenario_id.clone(),
            },
            cast,
            initial,
            root: Block { nodes },
            diagnostics: self.diags,
        })
    }

    fn cast(&self, scene: &raw::SceneData) -> BTreeMap<CharacterId, CastEntry> {
        let mut cast = BTreeMap::new();
        for (id, c) in &self.index.characters {
            let costumes = c
                .costumes
                .iter()
                .map(|costume| {
                    (
                        costume.costume_type.clone(),
                        CostumeEntry {
                            model: costume.model_bundle.clone().map(LibPath),
                            motions: costume
                                .motions
                                .iter()
                                .map(|(k, v)| (k.clone(), LibPath(v.path.clone())))
                                .collect(),
                        },
                    )
                })
                .collect();
            let first = scene
                .appear_characters
                .iter()
                .find(|a| i64::from(a.character2d_id) == *id)
                .map(|a| a.costume_type.clone())
                .or_else(|| c.costumes.first().map(|x| x.costume_type.clone()));
            cast.insert(
                *id as CharacterId,
                CastEntry {
                    asset_name: c.asset_name.clone(),
                    initial_costume: first,
                    costumes,
                },
            );
        }
        cast
    }

    fn background(&self, name: &str, file: &str) -> Option<LibPath> {
        if name.is_empty() {
            return None;
        }
        let key = if file.is_empty() || file == name {
            name.to_owned()
        } else {
            format!("{name}/{file}")
        };
        self.index
            .backgrounds
            .get(&key)
            .or_else(|| self.index.backgrounds.get(name))
            .map(|f| LibPath(f.path.clone()))
    }

    fn audio(map: &BTreeMap<String, sse_assets::episode::AudioRef>, name: &str) -> Option<AudioRef> {
        map.get(name).map(|a| AudioRef {
            cue: a.cue.clone(),
            files: a.files.iter().cloned().map(LibPath).collect(),
        })
    }

    fn audio_bgm(&self, name: &str) -> Option<AudioRef> {
        Self::audio(&self.index.bgm, name)
    }

    fn snippet(&mut self, scene: &raw::SceneData, pos: u32, s: &raw::Snippet) -> Result<Node> {
        let progress = if s.progress_behavior == 1 {
            Progress::WaitFinished
        } else {
            Progress::Now
        };
        let r = s.reference_index;
        let get = |len: usize, what: &'static str| -> Result<usize> {
            usize::try_from(r)
                .ok()
                .filter(|&i| i < len)
                .ok_or(ScenarioError::BadReference {
                    index: pos,
                    what,
                    reference: r,
                })
        };
        let raw_snippet = serde_json::to_value(s).unwrap_or_default();
        let kind = match s.action {
            0 => InstrKind::Wait,
            1 => {
                let t = &scene.talk_data[get(scene.talk_data.len(), "TalkData")?];
                InstrKind::Talk(self.talk(scene, pos, t))
            }
            2 | 4 => {
                let l = &scene.layout_data[get(scene.layout_data.len(), "LayoutData")?];
                self.layout(pos, s.action, l)
            }
            3 => InstrKind::Unsupported(Unsupported {
                reason: UnsupportedReason::InputName,
                finish: FinishRule::Immediate,
                raw: raw_snippet,
            }),
            5 => {
                self.diags.push(Diagnostic::ApproximateTiming {
                    index: pos,
                    reason: "Action=5 (Selectable) semantics not reversed (open question #42)".into(),
                });
                let instr = Instr {
                    index: pos,
                    progress,
                    delay: s.delay,
                    delay_tap_skippable: false,
                    kind: InstrKind::Unsupported(Unsupported {
                        reason: UnsupportedReason::Selectable,
                        finish: FinishRule::Immediate,
                        raw: raw_snippet.clone(),
                    }),
                };
                return Ok(Node::Branch(Branch {
                    index: pos,
                    arms: vec![Block {
                        nodes: vec![Node::Instr(instr)],
                    }],
                    raw: raw_snippet,
                }));
            }
            6 => {
                let e = &scene.special_effect_data
                    [get(scene.special_effect_data.len(), "SpecialEffectData")?];
                self.effect(pos, e)
            }
            7 => {
                let d = &scene.sound_data[get(scene.sound_data.len(), "SoundData")?];
                InstrKind::Sound {
                    ops: self.sound(d),
                }
            }
            8 => {
                let m = &scene.scenario_snippet_character_layout_modes[get(
                    scene.scenario_snippet_character_layout_modes.len(),
                    "ScenarioSnippetCharacterLayoutModes",
                )?];
                InstrKind::SetLayoutMode {
                    mode: layout_mode(m.character_layout_mode),
                }
            }
            other => InstrKind::Unsupported(Unsupported {
                reason: UnsupportedReason::UnknownAction { value: other },
                finish: FinishRule::Immediate,
                raw: raw_snippet,
            }),
        };
        Ok(Node::Instr(Instr {
            index: pos,
            progress,
            delay: s.delay,
            delay_tap_skippable: s.action == 7,
            kind,
        }))
    }

    fn talk(&mut self, scene: &raw::SceneData, pos: u32, t: &raw::TalkData) -> Talk {
        for (field, value) in [
            ("Speed", t.speed != 0.0, t.speed.to_string()),
            ("FontSize", t.font_size != 0, t.font_size.to_string()),
            ("TalkTention", t.talk_tention != 0, t.talk_tention.to_string()),
        ]
        .map(|(f, bad, v)| (f, (bad, v)))
        {
            if value.0 {
                self.diags.push(Diagnostic::IgnoredField {
                    index: pos,
                    field: field.into(),
                    value: value.1,
                });
            }
        }
        let voices = t
            .voices
            .iter()
            .map(|v| {
                let voice = Self::audio(&self.index.voices, &v.voice_id);
                if voice.is_none() && !v.voice_id.is_empty() {
                    self.diags.push(Diagnostic::MissingAsset {
                        index: pos,
                        what: format!("voice {}", v.voice_id),
                    });
                }
                TalkVoice {
                    character: v.character2d_id,
                    voice_id: v.voice_id.clone(),
                    voice,
                    volume: v.volume,
                }
            })
            .collect();
        let attached_effect = if t.require_play_effect {
            usize::try_from(t.effect_reference_idx)
                .ok()
                .and_then(|i| scene.special_effect_data.get(i))
                .and_then(|e| match self.effect(pos, e) {
                    InstrKind::Effect(e) => Some(Box::new(e)),
                    _ => None,
                })
        } else {
            None
        };
        let attached_sound = if t.require_play_sound {
            usize::try_from(t.sound_reference_idx)
                .ok()
                .and_then(|i| scene.sound_data.get(i))
                .map(|d| self.sound(d))
        } else {
            None
        };
        Talk {
            speakers: t.talk_characters.iter().map(|c| c.character2d_id).collect(),
            display_name: t.window_display_name.clone(),
            body: t
                .body
                .replace(consts::PLAYER_NAME_PLACEHOLDER, &self.opts.player_name),
            lip_sync: match t.lip_sync {
                0 => LipSyncMode::Text,
                1 => LipSyncMode::Voice,
                _ => LipSyncMode::Close,
            },
            motion_change: if t.motion_change_from == 1 {
                MotionChangeFactor::PlayTime
            } else {
                MotionChangeFactor::Text
            },
            motions: t
                .motions
                .iter()
                .map(|m| TalkMotion {
                    character: m.character2d_id,
                    motion: opt(&m.motion_name),
                    facial: opt(&m.facial_name),
                    timing_sync_value: m.timing_sync_value,
                })
                .collect(),
            voices,
            close_window_on_finish: t.when_finish_close_window,
            target_value_scale: t.target_value_scale,
            attached_effect,
            attached_sound,
        }
    }

    fn layout(&mut self, pos: u32, action: i32, l: &raw::LayoutData) -> InstrKind {
        let character = l.character2d_id;
        let side = |v: i32, diags: &mut Vec<Diagnostic>, field: &str| {
            Side::from_i32(v).unwrap_or_else(|| {
                diags.push(Diagnostic::UnknownEnumValue {
                    index: pos,
                    field: field.into(),
                    value: i64::from(v),
                });
                Side::None
            })
        };
        let depth = match l.depth_type {
            1 => DepthType::Front,
            2 => DepthType::Back,
            3 => DepthType::Reset,
            _ => DepthType::None,
        };
        if action == 4 || l.kind == 0 {
            return InstrKind::ChangeMotion {
                character,
                motion: opt(&l.motion_name),
                facial: opt(&l.facial_name),
            };
        }
        let op = match l.kind {
            1 => {
                if l.side_from != 0 {
                    self.diags.push(Diagnostic::IgnoredField {
                        index: pos,
                        field: "LayoutData.SideFrom (Move)".into(),
                        value: l.side_from.to_string(),
                    });
                }
                LayoutOp::Move {
                    to: side(l.side_to, &mut self.diags, "SideTo"),
                    offset_x: l.side_to_offset_x,
                    duration: match l.move_speed_type {
                        1 => consts::MOVE_DURATION_FAST,
                        2 => consts::MOVE_DURATION_SLOW,
                        _ => consts::MOVE_DURATION_NORMAL,
                    },
                }
            }
            2 => LayoutOp::Appear {
                from: side(l.side_from, &mut self.diags, "SideFrom"),
                offset_x: l.side_from_offset_x,
                costume: opt(&l.costume_type),
                motion: opt(&l.motion_name),
                facial: opt(&l.facial_name),
                depth,
            },
            3 => LayoutOp::Hide {
                // `ScenarioPlayer` +0x200 (speed scale) is 1 in normal playback.
                delay: if l.side_from == l.side_to {
                    consts::HIDE_DELAY_IN_PLACE
                } else {
                    let duration = match l.move_speed_type {
                        1 => consts::MOVE_DURATION_FAST,
                        2 => consts::MOVE_DURATION_SLOW,
                        _ => consts::MOVE_DURATION_NORMAL,
                    };
                    duration + consts::HIDE_SLIDE_FADE_OFFSET
                },
            },
            4 | 5 => LayoutOp::Shake {
                axis: if l.kind == 4 { Axis::X } else { Axis::Y },
                raw: serde_json::to_value(l).unwrap_or_default(),
            },
            6 => LayoutOp::Depth { depth },
            other => {
                self.diags.push(Diagnostic::UnknownEnumValue {
                    index: pos,
                    field: "LayoutData.Type".into(),
                    value: i64::from(other),
                });
                return InstrKind::Unsupported(Unsupported {
                    reason: UnsupportedReason::UnknownAction { value: 2 },
                    finish: FinishRule::Immediate,
                    raw: serde_json::to_value(l).unwrap_or_default(),
                });
            }
        };
        InstrKind::Layout(Layout { character, op })
    }

    fn effect(&mut self, pos: u32, e: &raw::SpecialEffect) -> InstrKind {
        use EffectOp::*;
        let raw = || serde_json::to_value(e).unwrap_or_default();
        let op = match e.effect_type {
            0 | 11 | 17 => Noop,
            1 => Fade {
                color: FadeColor::Black,
                dir: Direction::In,
            },
            2 => Fade {
                color: FadeColor::Black,
                dir: Direction::Out,
            },
            3 => Fade {
                color: FadeColor::White,
                dir: Direction::In,
            },
            4 => Fade {
                color: FadeColor::White,
                dir: Direction::Out,
            },
            5 => ShakeScreen,
            6 => ShakeWindow,
            7 => {
                let background = self.background(&e.string_val, &e.string_val_sub);
                if background.is_none() {
                    self.diags.push(Diagnostic::MissingAsset {
                        index: pos,
                        what: format!("background {}", e.string_val),
                    });
                }
                ChangeBackground {
                    background,
                    name: e.string_val.clone(),
                }
            }
            8 => Telop {
                text: e.string_val.clone(),
            },
            9 => CameraColor {
                effect: CameraColorEffect::Flashback,
            },
            10 | 28 => CameraColorOff,
            12 => Ambient {
                color: AmbientColor::Afternoon,
            },
            13 => Ambient {
                color: AmbientColor::Evening,
            },
            14 => Ambient {
                color: AmbientColor::Night,
            },
            15 => PlayScenarioEffect {
                name: e.string_val.clone(),
                bundle: e.string_val_sub.clone(),
            },
            16 => StopScenarioEffect {
                name: e.string_val.clone(),
            },
            18 => PlaceInfo {
                text: e.string_val.clone(),
            },
            19 => {
                let movie = self.index.movies.get(&e.string_val);
                let files: Vec<LibPath> = movie
                    .map(|m| m.files.iter().cloned().map(LibPath).collect())
                    .unwrap_or_default();
                let seconds = files
                    .iter()
                    .find(|f| f.0.ends_with(".wav"))
                    .and_then(|f| self.lib.load_wav(&f.0).ok())
                    .map(|p| p.duration() as f32);
                self.diags.push(Diagnostic::ApproximateTiming {
                    index: pos,
                    reason: format!(
                        "Movie {}: finishes after the movie's audio length ({})",
                        e.string_val,
                        seconds.map_or("unknown".into(), |s| format!("{s:.3} s"))
                    ),
                });
                return InstrKind::Unsupported(Unsupported {
                    reason: UnsupportedReason::Movie {
                        name: e.string_val.clone(),
                        files,
                    },
                    finish: seconds.map_or(FinishRule::Immediate, |seconds| FinishRule::After {
                        seconds,
                    }),
                    raw: raw(),
                });
            }
            20 | 21 | 40 | 41 => SekaiTransition {
                anniversary: e.effect_type >= 40,
                dir: if e.effect_type % 2 == 0 {
                    Direction::In
                } else {
                    Direction::Out
                },
            },
            22 => CharacterShader {
                character: e.int_val,
                shader: e.string_val.clone(),
                bundle: e.string_val_sub.clone(),
            },
            23 => SimpleSelectable {
                options: e.string_val.split('/').map(str::to_owned).collect(),
            },
            24 => {
                let voice = Self::audio(&self.index.voices, &e.string_val_sub);
                FullScreenText {
                    text: e.string_val.clone(),
                    voice_id: e.string_val_sub.clone(),
                    voice,
                }
            }
            25 => StopShakeScreen,
            26 => StopShakeWindow,
            27 => CameraColor {
                effect: CameraColorEffect::Sepia,
            },
            29..=36 => SideFade {
                effect_type: e.effect_type,
            },
            37 => {
                return InstrKind::Unsupported(Unsupported {
                    reason: UnsupportedReason::MusicVideo,
                    finish: FinishRule::Immediate,
                    raw: raw(),
                });
            }
            38 => Blur { dir: Direction::In },
            39 => Blur {
                dir: Direction::Out,
            },
            42 => {
                let parts: Vec<&str> = e.string_val.split(',').collect();
                let valid = parts.len() == 2;
                CameraMove {
                    x: parts.first().map_or(0.0, |p| single_try_parse(p)),
                    y: parts.get(1).map_or(0.0, |p| single_try_parse(p)),
                    valid,
                }
            }
            43 => CameraZoom {
                scale: single_try_parse(&e.string_val),
            },
            44 => BackgroundBlur {
                on: bool_try_parse(&e.string_val),
            },
            other => {
                return InstrKind::Unsupported(Unsupported {
                    reason: UnsupportedReason::UnknownEffectType { value: other },
                    finish: FinishRule::Immediate,
                    raw: raw(),
                });
            }
        };
        InstrKind::Effect(Effect {
            effect_type: e.effect_type,
            duration: e.duration,
            op,
        })
    }

    fn sound(&self, d: &raw::SoundData) -> Vec<SoundOp> {
        let se_ref = |name: &str| Self::audio(&self.index.se, name);
        let mut ops = Vec::new();
        match d.play_mode {
            0 | 1 => {
                if !d.bgm.is_empty() {
                    ops.push(SoundOp::Bgm {
                        name: d.bgm.clone(),
                        bgm: self.audio_bgm(&d.bgm),
                        fade: if d.duration != 0.0 {
                            d.duration
                        } else {
                            consts::DEFAULT_BGM_FADE
                        },
                        volume: d.volume,
                    });
                }
                if !d.se.is_empty() {
                    ops.push(SoundOp::SeOneShot {
                        name: d.se.clone(),
                        se: se_ref(&d.se),
                        volume: d.volume,
                        deferred: d.duration.max(0.0),
                    });
                }
            }
            2 => ops.push(SoundOp::SeLoop {
                name: d.se.clone(),
                se: se_ref(&d.se),
                fade: d.duration,
                volume: d.volume,
            }),
            3 => ops.push(SoundOp::Stop {
                se: d.se.clone(),
                bgm: d.bgm.clone(),
                fade: d.duration,
            }),
            4 => ops.push(SoundOp::BgmVolume {
                volume: d.volume,
                duration: d.duration,
            }),
            5 => ops.push(SoundOp::BgmAisacVolume { value: d.volume }),
            6 => ops.push(SoundOp::BgmBlock {
                index: d.bgm_block_index,
            }),
            _ => {}
        }
        if ops.is_empty() {
            ops.push(SoundOp::Nothing);
        }
        ops
    }
}

fn layout_mode(v: i32) -> LayoutMode {
    if v == 3 {
        LayoutMode::Three
    } else {
        LayoutMode::Default
    }
}
