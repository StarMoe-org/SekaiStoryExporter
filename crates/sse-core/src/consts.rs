//! Reverse-engineered constants for CN 6.4.0.
//!
//! Source of truth: `docs/reverse/versions/cn-6.4.0/constants.yaml` (and `layout.yaml`).
//! Every item names its YAML key. Until `cargo xtask codegen` exists, this file is kept in
//! sync by hand and the `matches_constants_yaml` test fails if a scalar drifts.
//! Literals keep the full precision written in the YAML on purpose.
#![allow(clippy::excessive_precision)]

// ---- screen / ugui -------------------------------------------------------------------

/// `screen.base_screen_size`: CanvasScaler reference resolution.
pub const BASE_SCREEN_SIZE: [f32; 2] = [1920.0, 1080.0];
/// `screen.background_base_size`.
pub const BACKGROUND_BASE_SIZE: [f32; 2] = [2338.0, 1440.0];
/// `screen.outside_fill_offset_x`.
pub const OUTSIDE_FILL_OFFSET_X: f32 = 1169.0;
/// `layout.yaml` `over_position.literal`.
pub const OVER_POSITION_MARGIN: f32 = 819.2;

/// `frame.story_target_frame_rate`: the story inherits the UI `targetFrameRate` (decision Q34).
pub const STORY_TARGET_FRAME_RATE: u32 = 60;

// ---- scenario player -----------------------------------------------------------------

/// `scenario.playcore_warmup_frames`.
pub const PLAYCORE_WARMUP_FRAMES: u32 = 10;
/// `scenario.played_wait_time`.
pub const PLAYED_WAIT_TIME: f32 = 0.25;
/// `scenario.cleanup_sound_fade_time`.
pub const CLEANUP_SOUND_FADE_TIME: f32 = 1.0;
/// `scenario.infinity_duration`.
pub const INFINITY_DURATION: f32 = 3600.0;
/// `scenario.fade_time`.
pub const SCENARIO_FADE_TIME: f32 = 0.15;
/// `scenario.model_color_normal` / `_evening` / `_night`.
pub const MODEL_COLOR_NORMAL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const MODEL_COLOR_EVENING: [f32; 4] = [0.990_000_01, 0.899_999_98, 0.800_000_01, 1.0];
pub const MODEL_COLOR_NIGHT: [f32; 4] = [0.829_999_98, 0.860_000_01, 0.910_000_03, 1.0];
/// `scenario.player_name_placeholder`.
pub const PLAYER_NAME_PLACEHOLDER: &str = "{{playerName}}";

// ---- layout ----------------------------------------------------------------------------

/// `layout.move_duration_normal` / `_fast` / `_slow`.
pub const MOVE_DURATION_NORMAL: f32 = 0.5;
pub const MOVE_DURATION_FAST: f32 = 0.33;
pub const MOVE_DURATION_SLOW: f32 = 0.75;
/// `Live2DRenderStudio.fixedStagePosition.y` (`.ctor` 0x205E9B8, literal 0x529E798). In
/// landscape `UpdateRenderOrientation` (0x205E77C) sets `stageRoot.localPosition =
/// fixedStagePosition × orthographicSize`, and the model hangs below `stageRoot` at
/// `standPositionLandscape` — so the model stands at y = −1.5 + 0.383 in RT world units.
pub const LIVE2D_FIXED_STAGE_Y: f32 = -1.0;
/// `Live2DRenderStudio` landscape `orthographicSize`.
pub const LIVE2D_ORTHO_SIZE: f32 = 1.5;
/// `standPositionLandscape.y`.
pub const LIVE2D_STAND_Y: f32 = 0.383;
/// `ScenarioStudioCamera.BlurIn/BlurOut` → `SetBlurEffect(iteration: 3, size, downSample: 2)`
/// with `size` from 0 to 3 (`rendering.md`); `RenderBlur` 0x4A15AC4 uses them as below.
pub const BLUR_ITERATIONS: u32 = 3;
pub const BLUR_DOWN_SAMPLE: u32 = 2;
pub const BLUR_MAX_SPREAD: f32 = 3.0;
/// `ScenarioPlayer.movieResolution` (`.ctor` 0x16B9B04, literal 0x529D340): size of the
/// centred movie `RawImage`.
pub const MOVIE_RESOLUTION: [f32; 2] = [2338.0, 1080.0];
/// `ScreenSlideInOut.<Play>d__7` (0x16EB2B4): `DOAnchorPos(to, 0.2)` + `SetEase(OutQuart)`.
pub const PLACE_INFO_SLIDE_DURATION: f32 = 0.2;
/// `UILayer/TopLeft2/PlaceInfo` width (`resources.assets|60657`, pivot (0, 1), anchored x 0).
pub const PLACE_INFO_WIDTH: f32 = 580.0;
/// `CanvasRoot/UICamera` (`level*|Camera`): orthographic, size 1; the root canvas is
/// Screen Space - Camera, centred on the camera at world x 0.
pub const UI_CAMERA_ORTHO_SIZE: f32 = 1.0;

/// `ScenarioPlaceInfo.DefaultPosX = −(rt.position.x + rect.width)`: a *world* x plus a
/// reference-pixel width. `Reset` and `Hide` both evaluate it while the panel sits at
/// anchored x 0, i.e. with its pivot on the canvas' left edge: world x
/// `−ortho × content_w / content_h` (canvas height spans `2 × ortho` world units).
/// 16:9 → −(580 − 1.778) = −578.22.
pub fn place_info_hidden_x(content_size: [f32; 2]) -> f32 {
    let left = -UI_CAMERA_ORTHO_SIZE * content_size[0] / content_size[1];
    -(left + PLACE_INFO_WIDTH)
}
/// SekaiIn (20 / 40): `ColorFader.Set(white)` then fades to transparent after this delay.
pub const SEKAI_IN_FADE_DELAY: f32 = 0.25;
/// `DestroyAtTime.deleteAtTime` on `fx_transition_scenario` (`resources.assets|15600`).
pub const FX_LIFETIME: f32 = 5.0;
/// SekaiOut (21 / 41): fades to opaque white after this delay (with the particle prefab).
pub const SEKAI_OUT_FADE_DELAY: f32 = 0.5;
/// Delay before a hide fade when the character stays in place (`0x3E19999A`,
/// `SnippetActionCharacterLayout` 0x16DB774).
pub const HIDE_DELAY_IN_PLACE: f32 = 0.15;
/// Added to the move duration for a sliding hide (`0xBDCCCCCD` = -0.1, 0x16DB838).
pub const HIDE_SLIDE_FADE_OFFSET: f32 = -0.1;
/// `layout.character_fade_duration`.
pub const CHARACTER_FADE_DURATION: f32 = 0.1;
/// `layout.yaml` `transform_map`: side X for DefaultMode / ThreeMode.
pub const SIDE_X_DEFAULT: f32 = 350.0;
pub const SIDE_X_THREE: f32 = 550.0;
/// `layout.yaml` `mode_scale`.
pub const MODE_SCALE_DEFAULT: f32 = 1.0;
pub const MODE_SCALE_THREE: f32 = 0.9;

// ---- talk window -----------------------------------------------------------------------

/// `talk.word_interval`.
pub const WORD_INTERVAL: f32 = 0.059_999_998_658_895_49;
/// `talk.auto_next_page_delay`.
pub const AUTO_NEXT_PAGE_DELAY: f32 = 2.0;
/// `talk.auto_wait_after_voice`.
pub const AUTO_WAIT_AFTER_VOICE: f32 = 0.5;
/// `talk.auto_voice_timeout`.
pub const AUTO_VOICE_TIMEOUT: f32 = 30.0;
/// `sound.voice_end_latency`: a voice counts as finished when `CriAtomExPlayback.GetStatus()`
/// reaches Removed; with Sonic Sync on (`CriWareInitializer.atomConfig.iosEnableSonicSync`)
/// that trails the last mixed sample by the output buffer, `iosBufferingTime` = 50 ms.
pub const VOICE_END_LATENCY: f32 = 0.05;

/// `ScenarioFullScreenTextDialog.playDuration` (static, `.cctor` 0x169DB80): cinemascope
/// tween, hold after the voice, and `FadeOutAll` duration.
pub const FST_PLAY_DURATION: f32 = 1.0;
/// `ScenarioFullScreenTextDialog.cinemascopeHeight` (static): bar height in reference pixels.
pub const FST_CINEMASCOPE_HEIGHT: f32 = 240.0;
/// `baseCinemascope` alpha while the bars are shown (`PlayCinemascope` 0x169DC00).
pub const FST_BASE_ALPHA: f32 = 0.5;
/// `UniTask.Delay(0.5 s)` after the bars on `ViewType.First` (`PlayCore` 0x169E140).
pub const FST_OPEN_DELAY: f32 = 0.5;
/// `TextAppearFade.textWait` (prefab `resources.assets|572433`): per-slot fade-in time.
pub const FST_TEXT_WAIT: f32 = 0.125;
/// Voice wait cap in `PlayCore` (`t >= 10`).
pub const FST_VOICE_TIMEOUT: f32 = 10.0;
/// `new TMP_TextInfo()` allocates `characterInfo[8]`.
pub const TMP_CHARACTER_INFO_INITIAL: u32 = 8;
/// `talk.window_fade_duration`.
pub const TALK_WINDOW_FADE_DURATION: f32 = 0.15;
/// `TalkWindow.Open` / `Close` (0x16E57FC / 0x16E5B98): `PlayActive(1 | 0, 0.2)`, linear.
/// Typing starts from `OnCompleteOpen`, i.e. after the fade-in.
pub const TALK_WINDOW_OPEN_CLOSE_DURATION: f32 = 0.2;
/// `talk.line_advance_px`.
pub const TALK_LINE_ADVANCE_PX: f32 = 48.0;

// ---- sound / telop / transitions -------------------------------------------------------

/// `sound.default_bgm_fade`.
pub const DEFAULT_BGM_FADE: f32 = 0.25;
/// `telop.auto_hold`.
pub const TELOP_AUTO_HOLD: f32 = 2.0;
/// `telop.anim_clip_length`.
pub const TELOP_ANIM_CLIP_LENGTH: f32 = 0.333_333_34;
/// `sekai_transition.lifetime`.
pub const SEKAI_TRANSITION_LIFETIME: f32 = 5.0;

// ---- live2d ----------------------------------------------------------------------------

/// `live2d.body_motion_fade`.
pub const BODY_MOTION_FADE: f32 = 0.5;
/// `live2d.facial_fade`.
pub const FACIAL_FADE: f32 = 0.25;
/// `live2d.same_category_blend`.
pub const SAME_CATEGORY_BLEND: f32 = 0.125;
/// `live2d.scale_reference_height`.
pub const LIVE2D_SCALE_REFERENCE_HEIGHT: f32 = 1024.0;
/// `live2d.render_texture_size`.
pub const LIVE2D_RT_SIZE: [u32; 2] = [2304, 1536];
/// `live2d.mask_texture`.
pub const MASK_TEXTURE_SIZE: u32 = 1024;
/// `live2d.eyeblink`.
pub const EYEBLINK_MEAN: f32 = 3.0;
pub const EYEBLINK_MAX_DEV: f32 = 2.0;
pub const EYEBLINK_SPEED: f32 = 10.0;
/// Default for `OnLive2DInvokeUserData("eyeblink,…")` arguments (`live2d.md` §1).
pub const EYEBLINK_COMMAND_DEFAULT: f32 = 0.100_000_001_490_116_12;

// ---- lip sync --------------------------------------------------------------------------

/// `Live2DVoice` defaults (`live2d.md` §8).
pub const LIPSYNC_DEFAULT_TARGET_VALUE_SCALE: f32 = 1.0;
pub const LIPSYNC_DEFAULT_POW_K: f32 = 1.75;
/// Characters that get `(1.25, 1.75)` (`live2d.md` §8).
pub const LIPSYNC_LOUD_CHARACTERS: [i32; 6] = [1, 4, 8, 12, 17, 18];
pub const LIPSYNC_LOUD_TARGET_VALUE_SCALE: f32 = 1.25;
/// `lipsync.lip_levels`.
pub const LIP_LEVELS: [f32; 38] = [
    0.12, 0.24, 0.9, 1.0, 0.5, 0.24, 0.1, 0.24, 0.45, 0.8, 0.8, 0.96, 0.8, 0.4, 0.24, 0.1, 0.06,
    0.1, 0.24, 0.24, 0.5, 0.7, 0.96, 0.9, 0.8, 0.24, 0.24, 0.1, 0.06, 0.1, 0.24, 0.4, 0.5, 0.9,
    0.8, 0.45, 0.3, 0.24,
];

// ---- post effects ----------------------------------------------------------------------

/// `rendering.md` §6 `ScenarioGuassianBlur` 5-tap weights (`shader.blur_weights`).
pub const BLUR_WEIGHTS: [f32; 5] = [0.402_6, 0.244_2, 0.244_2, 0.054_5, 0.054_5];

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust2::{Yaml, YamlLoader};

    fn load() -> Vec<Yaml> {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/reverse/versions/cn-6.4.0/constants.yaml"
        );
        let text = std::fs::read_to_string(path).unwrap();
        let docs = YamlLoader::load_from_str(&text).unwrap();
        docs[0].as_vec().unwrap().clone()
    }

    fn scalar(entries: &[Yaml], key: &str) -> f64 {
        let entry = entries
            .iter()
            .find(|e| e["key"].as_str() == Some(key))
            .unwrap_or_else(|| panic!("{key} missing from constants.yaml"));
        match &entry["value"] {
            Yaml::Real(s) => s.parse().unwrap(),
            Yaml::Integer(i) => *i as f64,
            other => panic!("{key}: not a scalar: {other:?}"),
        }
    }

    #[test]
    fn matches_constants_yaml() {
        let e = load();
        let check = |key: &str, value: f32| {
            let expected = scalar(&e, key) as f32;
            assert_eq!(value.to_bits(), expected.to_bits(), "{key}");
        };
        check("scenario.played_wait_time", PLAYED_WAIT_TIME);
        check("scenario.cleanup_sound_fade_time", CLEANUP_SOUND_FADE_TIME);
        check("scenario.infinity_duration", INFINITY_DURATION);
        check("scenario.fade_time", SCENARIO_FADE_TIME);
        check("scenario.playcore_warmup_frames", PLAYCORE_WARMUP_FRAMES as f32);
        check("layout.move_duration_normal", MOVE_DURATION_NORMAL);
        check("layout.move_duration_fast", MOVE_DURATION_FAST);
        check("layout.move_duration_slow", MOVE_DURATION_SLOW);
        check("layout.character_fade_duration", CHARACTER_FADE_DURATION);
        check("talk.word_interval", WORD_INTERVAL);
        check("talk.auto_next_page_delay", AUTO_NEXT_PAGE_DELAY);
        check("talk.auto_wait_after_voice", AUTO_WAIT_AFTER_VOICE);
        check("talk.auto_voice_timeout", AUTO_VOICE_TIMEOUT);
        check("sound.voice_end_latency", VOICE_END_LATENCY);
        check("talk.window_fade_duration", TALK_WINDOW_FADE_DURATION);
        check("talk.line_advance_px", TALK_LINE_ADVANCE_PX);
        check("sound.default_bgm_fade", DEFAULT_BGM_FADE);
        check("telop.auto_hold", TELOP_AUTO_HOLD);
        check("telop.anim_clip_length", TELOP_ANIM_CLIP_LENGTH);
        check("fst.play_duration", FST_PLAY_DURATION);
        check("fst.cinemascope_height", FST_CINEMASCOPE_HEIGHT);
        check("fst.base_alpha", FST_BASE_ALPHA);
        check("fst.open_delay", FST_OPEN_DELAY);
        check("fst.text_wait", FST_TEXT_WAIT);
        check("fst.voice_timeout", FST_VOICE_TIMEOUT);
        check("layout.hide_delay_in_place", HIDE_DELAY_IN_PLACE);
        check("layout.hide_slide_fade_offset", HIDE_SLIDE_FADE_OFFSET);
        check("sekai.in_fade_delay", SEKAI_IN_FADE_DELAY);
        check("sekai.out_fade_delay", SEKAI_OUT_FADE_DELAY);
        check("talk.window_open_close_duration", TALK_WINDOW_OPEN_CLOSE_DURATION);
        check("place_info.width", PLACE_INFO_WIDTH);
        check("fx.transition_lifetime", FX_LIFETIME);
        check("ui.camera_ortho_size", UI_CAMERA_ORTHO_SIZE);
        check("sekai_transition.lifetime", SEKAI_TRANSITION_LIFETIME);
        check("live2d.body_motion_fade", BODY_MOTION_FADE);
        check("live2d.facial_fade", FACIAL_FADE);
        check("live2d.same_category_blend", SAME_CATEGORY_BLEND);
        check("live2d.scale_reference_height", LIVE2D_SCALE_REFERENCE_HEIGHT);
        check("screen.outside_fill_offset_x", OUTSIDE_FILL_OFFSET_X);
        check("frame.story_target_frame_rate", STORY_TARGET_FRAME_RATE as f32);
    }

    #[test]
    fn place_info_hidden_x_uses_the_canvas_world_edge() {
        let x = place_info_hidden_x([1920.0, 1080.0]);
        assert!((x - (-(580.0 - 16.0 / 9.0))).abs() < 1e-4, "{x}");
    }

    #[test]
    fn lip_levels_match_constants_yaml() {
        let e = load();
        let entry = e
            .iter()
            .find(|x| x["key"].as_str() == Some("lipsync.lip_levels"))
            .unwrap();
        let values: Vec<f32> = entry["value"]
            .as_vec()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        assert_eq!(values, LIP_LEVELS);
    }
}
