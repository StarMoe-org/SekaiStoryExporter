//! `ScenarioSceneData` as serialised by the ripper (typetree JSON). Field names are the
//! game's C# names, typos included (`docs/reverse/versions/cn-6.4.0/data-model.md`).
//! Unity serialises `bool` as `0/1`, so booleans accept either form.

use serde::{Deserialize, Deserializer};

fn flag<'de, D: Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        B(bool),
        I(i64),
    }
    Ok(match Flag::deserialize(d)? {
        Flag::B(b) => b,
        Flag::I(i) => i != 0,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SceneData {
    #[serde(default)]
    pub scenario_id: String,
    #[serde(default)]
    pub appear_characters: Vec<ResourceSet>,
    #[serde(default)]
    pub first_layout: Vec<FirstLayout>,
    #[serde(default)]
    pub first_bgm: String,
    #[serde(default)]
    pub episode_music_video_id: String,
    #[serde(default)]
    pub first_background: String,
    #[serde(default)]
    pub first_aisac_value: String,
    #[serde(default)]
    pub first_character_layout_mode: i32,
    #[serde(default)]
    pub snippets: Vec<Snippet>,
    #[serde(default)]
    pub talk_data: Vec<TalkData>,
    #[serde(default)]
    pub layout_data: Vec<LayoutData>,
    #[serde(default)]
    pub special_effect_data: Vec<SpecialEffect>,
    #[serde(default)]
    pub sound_data: Vec<SoundData>,
    #[serde(default)]
    pub scenario_snippet_character_layout_modes: Vec<LayoutModeData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ResourceSet {
    pub character2d_id: i32,
    #[serde(default)]
    pub costume_type: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct FirstLayout {
    #[serde(default)]
    pub position_side: i32,
    pub character2d_id: i32,
    #[serde(default)]
    pub costume_type: String,
    #[serde(default)]
    pub motion_name: String,
    #[serde(default)]
    pub facial_name: String,
    #[serde(default)]
    pub offset_x: f32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Snippet {
    #[serde(default)]
    pub index: i64,
    pub action: i32,
    #[serde(default)]
    pub progress_behavior: i32,
    #[serde(default)]
    pub reference_index: i64,
    #[serde(default)]
    pub delay: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TalkData {
    #[serde(default)]
    pub talk_characters: Vec<TalkCharacter>,
    #[serde(default)]
    pub window_display_name: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub talk_tention: i32,
    #[serde(default)]
    pub lip_sync: i32,
    #[serde(default)]
    pub motion_change_from: i32,
    #[serde(default)]
    pub motions: Vec<TalkMotion>,
    #[serde(default)]
    pub voices: Vec<TalkVoice>,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub font_size: i32,
    #[serde(default, deserialize_with = "flag")]
    pub when_finish_close_window: bool,
    #[serde(default, deserialize_with = "flag")]
    pub require_play_effect: bool,
    #[serde(default)]
    pub effect_reference_idx: i64,
    #[serde(default, deserialize_with = "flag")]
    pub require_play_sound: bool,
    #[serde(default)]
    pub sound_reference_idx: i64,
    #[serde(default)]
    pub target_value_scale: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TalkCharacter {
    pub character2d_id: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TalkMotion {
    pub character2d_id: i32,
    #[serde(default)]
    pub motion_name: String,
    #[serde(default)]
    pub facial_name: String,
    #[serde(default)]
    pub timing_sync_value: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TalkVoice {
    pub character2d_id: i32,
    #[serde(default)]
    pub voice_id: String,
    #[serde(default)]
    pub volume: f32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutData {
    #[serde(default, rename = "Type")]
    pub kind: i32,
    #[serde(default)]
    pub side_from: i32,
    #[serde(default)]
    pub side_from_offset_x: f32,
    #[serde(default)]
    pub side_to: i32,
    #[serde(default)]
    pub side_to_offset_x: f32,
    #[serde(default)]
    pub depth_type: i32,
    pub character2d_id: i32,
    #[serde(default)]
    pub costume_type: String,
    #[serde(default)]
    pub motion_name: String,
    #[serde(default)]
    pub facial_name: String,
    #[serde(default)]
    pub move_speed_type: i32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SpecialEffect {
    pub effect_type: i32,
    #[serde(default)]
    pub string_val: String,
    #[serde(default)]
    pub string_val_sub: String,
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub int_val: i32,
}

#[derive(Debug, Deserialize, serde::Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct SoundData {
    #[serde(default)]
    pub play_mode: i32,
    #[serde(default)]
    pub bgm: String,
    #[serde(default)]
    pub se: String,
    #[serde(default)]
    pub volume: f32,
    #[serde(default)]
    pub se_bundle_name: String,
    #[serde(default)]
    pub duration: f32,
    #[serde(default)]
    pub bgm_block_index: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LayoutModeData {
    #[serde(default)]
    pub character_layout_mode: i32,
}
