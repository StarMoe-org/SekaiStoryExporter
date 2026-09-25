//! The game's constants (CN 6.4.0; JP 6.8.1 uses the same values).
//!
//! Every item names its key (`group.name`) and, where it helps, the game class or method it
//! comes from. Literals keep the full f32 precision of the game's values on purpose.
#![allow(clippy::excessive_precision)]

// ---- screen / ugui -------------------------------------------------------------------

/// `screen.base_screen_size`: CanvasScaler reference resolution.
pub const BASE_SCREEN_SIZE: [f32; 2] = [1920.0, 1080.0];
/// `screen.background_base_size`.
pub const BACKGROUND_BASE_SIZE: [f32; 2] = [2338.0, 1440.0];
/// `screen.outside_fill_offset_x`.
pub const OUTSIDE_FILL_OFFSET_X: f32 = 1169.0;
/// `layout.over_position`.
pub const OVER_POSITION_MARGIN: f32 = 819.2;

/// `frame.story_target_frame_rate`: the story inherits the UI `targetFrameRate` (ADR-0009).
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
/// `Live2DRenderStudio.fixedStagePosition.y`. In landscape `UpdateRenderOrientation` sets
/// `stageRoot.localPosition =
/// fixedStagePosition × orthographicSize`, and the model hangs below `stageRoot` at
/// `standPositionLandscape` — so the model stands at y = −1.5 + 0.383 in RT world units.
pub const LIVE2D_FIXED_STAGE_Y: f32 = -1.0;
/// `Live2DRenderStudio` landscape `orthographicSize`.
pub const LIVE2D_ORTHO_SIZE: f32 = 1.5;
/// `standPositionLandscape.y`.
pub const LIVE2D_STAND_Y: f32 = 0.383;
/// `ScenarioStudioCamera.BlurIn/BlurOut` → `SetBlurEffect(iteration: 3, size, downSample: 2)`
/// with `size` from 0 to 3; `RenderBlur` uses them as below.
pub const BLUR_ITERATIONS: u32 = 3;
pub const BLUR_DOWN_SAMPLE: u32 = 2;
pub const BLUR_MAX_SPREAD: f32 = 3.0;
/// `ScenarioPlayer.movieResolution`: size of the centred movie `RawImage`.
pub const MOVIE_RESOLUTION: [f32; 2] = [2338.0, 1080.0];
/// `ScreenSlideInOut.<Play>d__7`: `DOAnchorPos(to, 0.2)` + `SetEase(OutQuart)`.
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
/// `SnippetActionCharacterLayout`).
pub const HIDE_DELAY_IN_PLACE: f32 = 0.15;
/// Added to the move duration for a sliding hide (`0xBDCCCCCD` = -0.1).
pub const HIDE_SLIDE_FADE_OFFSET: f32 = -0.1;
/// `layout.character_fade_duration`.
pub const CHARACTER_FADE_DURATION: f32 = 0.1;
/// `layout.transform_map`: side X for DefaultMode / ThreeMode.
pub const SIDE_X_DEFAULT: f32 = 350.0;
pub const SIDE_X_THREE: f32 = 550.0;
/// `layout.mode_scale`.
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

/// `ScenarioFullScreenTextDialog.playDuration` (static, `.cctor`): cinemascope
/// tween, hold after the voice, and `FadeOutAll` duration.
pub const FST_PLAY_DURATION: f32 = 1.0;
/// `ScenarioFullScreenTextDialog.cinemascopeHeight` (static): bar height in reference pixels.
pub const FST_CINEMASCOPE_HEIGHT: f32 = 240.0;
/// `baseCinemascope` alpha while the bars are shown (`PlayCinemascope`).
pub const FST_BASE_ALPHA: f32 = 0.5;
/// `UniTask.Delay(0.5 s)` after the bars on `ViewType.First` (`PlayCore`).
pub const FST_OPEN_DELAY: f32 = 0.5;
/// `TextAppearFade.textWait` (prefab `resources.assets|572433`): per-slot fade-in time.
pub const FST_TEXT_WAIT: f32 = 0.125;
/// Voice wait cap in `PlayCore` (`t >= 10`).
pub const FST_VOICE_TIMEOUT: f32 = 10.0;
/// `new TMP_TextInfo()` allocates `characterInfo[8]`.
pub const TMP_CHARACTER_INFO_INITIAL: u32 = 8;
/// `talk.window_fade_duration`.
pub const TALK_WINDOW_FADE_DURATION: f32 = 0.15;
/// `TalkWindow.Open` / `Close`: `PlayActive(1 | 0, 0.2)`, linear.
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
/// Default for `OnLive2DInvokeUserData("eyeblink,…")` arguments.
pub const EYEBLINK_COMMAND_DEFAULT: f32 = 0.100_000_001_490_116_12;

// ---- lip sync --------------------------------------------------------------------------

/// `Live2DVoice` defaults.
pub const LIPSYNC_DEFAULT_TARGET_VALUE_SCALE: f32 = 1.0;
pub const LIPSYNC_DEFAULT_POW_K: f32 = 1.75;
/// Characters that get `(1.25, 1.75)`.
pub const LIPSYNC_LOUD_CHARACTERS: [i32; 6] = [1, 4, 8, 12, 17, 18];
pub const LIPSYNC_LOUD_TARGET_VALUE_SCALE: f32 = 1.25;
/// `lipsync.lip_levels`.
pub const LIP_LEVELS: [f32; 38] = [
    0.12, 0.24, 0.9, 1.0, 0.5, 0.24, 0.1, 0.24, 0.45, 0.8, 0.8, 0.96, 0.8, 0.4, 0.24, 0.1, 0.06,
    0.1, 0.24, 0.24, 0.5, 0.7, 0.96, 0.9, 0.8, 0.24, 0.24, 0.1, 0.06, 0.1, 0.24, 0.4, 0.5, 0.9,
    0.8, 0.45, 0.3, 0.24,
];

// ---- post effects ----------------------------------------------------------------------

/// `ScenarioGuassianBlur` 5-tap weights (`shader.blur_weights`).
pub const BLUR_WEIGHTS: [f32; 5] = [0.402_6, 0.244_2, 0.244_2, 0.054_5, 0.054_5];

// ---- character shader (hologram) -------------------------------------------------------

/// `Resources/Live2D/Materials/Live2DHologram` (shader `Sekai/Live2D/Live2DHologram`),
/// identical in CN 6.4.0 and JP 6.8.1: initial `_Line`, `_SubColor.a`, and the constants.
pub const HOLOGRAM_LINE: f32 = 0.86;
pub const HOLOGRAM_SUB_ALPHA: f32 = 0.776_470_6;
pub const HOLOGRAM_INFLUENCE: f32 = 0.6;
pub const HOLOGRAM_MONOCHROME: [f32; 3] = [0.2, 0.45, 0.35];
pub const HOLOGRAM_TONE_RATIO: [f32; 3] = [0.9, 1.2, 1.15];
/// Fragment: `rgb += (_Line >= line) ? line × 0.07 : 0` with `line = _SubTex.r`.
pub const HOLOGRAM_LINE_GAIN: f32 = 0.07;
/// Vertex: `TEXCOORD2.y += _Time.x × 0.1`, `_Time.x = t / 20`.
pub const HOLOGRAM_SCROLL_PER_SECOND: f32 = 0.1 / 20.0;
/// `Live2DHologramController.Update`: re-roll when `Random.Range(0, 10000) < 150` or the
/// countdown ran out: `_Line ∈ [0.6, 0.8]`, `_SubColor.a ∈ [0.85, 0.9]`, countdown ∈ [0, 0.5].
pub const HOLOGRAM_REROLL_CHANCE: (i32, i32) = (150, 10_000);
pub const HOLOGRAM_LINE_RANGE: [f32; 2] = [0.6, 0.8];
pub const HOLOGRAM_ALPHA_RANGE: [f32; 2] = [0.85, 0.9];
pub const HOLOGRAM_COUNTDOWN_RANGE: [f32; 2] = [0.0, 0.5];
/// `Setup`: countdown = `Random.Range(0, 0.2) + 0.05`.
pub const HOLOGRAM_FIRST_COUNTDOWN: [f32; 2] = [0.05, 0.25];
