//! Per-character runtime state (`ScenarioModel` + `Live2DModel` components).

use std::collections::BTreeMap;
use std::sync::Arc;

use sse_assets::Library;
use sse_core::{TimeBase, consts, det_math};
use sse_ir::CharacterId;
use sse_live2d::blink::EyeBlink;
use sse_live2d::motion::{Animator, BoundClip, LAYER_BODY, LAYER_FACE};
use sse_live2d::physics::Physics;

use crate::Tween;
use crate::lipsync::Lip;

/// Static model data the bake needs (parameter table and physics rig).
pub struct ModelInfo {
    pub ids: Vec<String>,
    pub min: Vec<f32>,
    pub max: Vec<f32>,
    pub defaults: Vec<f32>,
    pub physics: Option<Physics>,
    pub mouth: Option<usize>,
    pub eyes: [Option<usize>; 2],
    pub breath: Option<usize>,
}

impl ModelInfo {
    pub fn load(lib: &Library, bundle: &str) -> Result<Self, String> {
        let m3 = lib.load_model3(bundle).map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&m3.moc).map_err(|e| e.to_string())?;
        let moc = sse_live2d::Moc::new(&bytes).map_err(|e| e.to_string())?;
        let model = sse_live2d::Model::new(moc).map_err(|e| e.to_string())?;
        let ids = model.parameter_ids.clone();
        let physics = match &m3.physics {
            Some(p) => {
                let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
                Some(Physics::from_json(&bytes, &ids).map_err(|e| e.to_string())?)
            }
            None => None,
        };
        Ok(Self {
            mouth: sse_live2d::find_param(&ids, sse_live2d::PARAM_MOUTH_OPEN_Y),
            eyes: [
                sse_live2d::find_param(&ids, sse_live2d::PARAM_EYE_L_OPEN),
                sse_live2d::find_param(&ids, sse_live2d::PARAM_EYE_R_OPEN),
            ],
            breath: sse_live2d::find_param(&ids, sse_live2d::PARAM_BREATH),
            min: model.parameter_min.clone(),
            max: model.parameter_max.clone(),
            defaults: model.parameter_default.clone(),
            ids,
            physics,
        })
    }
}

pub struct CharacterRt {
    pub id: CharacterId,
    pub costume: String,
    pub model: usize,
    pub info: Arc<ModelInfo>,
    pub animator: Animator,
    pub physics: Option<Physics>,
    pub blink: EyeBlink,
    pub lip: Lip,
    pub breath_value: f32,
    pub clips: BTreeMap<String, Arc<BoundClip>>,
    pub fades: BTreeMap<String, f32>,
    pub values: Vec<f32>,
    pub visible: bool,
    pub opacity: Tween,
    pub hide_at: Option<u32>,
    pub x: Tween,
    pub y: f32,
    pub scale: f32,
    /// Talk-embedded motion changes: (frame, motion, facial).
    pub pending: Vec<(u32, Option<String>, Option<String>)>,
}

impl CharacterRt {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: CharacterId,
        costume: String,
        model: usize,
        info: Arc<ModelInfo>,
        breath_deg: f32,
        x: f32,
        y: f32,
        scale: f32,
    ) -> Self {
        // breathVector = 0 in stories: a constant written every frame (`live2d.md` §8).
        let breath_value =
            det_math::sinf(breath_deg * std::f32::consts::PI / 180.0) * 0.5 + 0.5;
        Self {
            id,
            costume,
            model,
            physics: info.physics.clone(),
            values: info.defaults.clone(),
            info,
            animator: Animator::default(),
            blink: EyeBlink::default(),
            lip: Lip::None,
            breath_value,
            clips: BTreeMap::new(),
            fades: BTreeMap::new(),
            visible: false,
            opacity: Tween::fixed(0.0),
            hide_at: None,
            x: Tween::fixed(x),
            y,
            scale,
            pending: Vec::new(),
        }
    }

    pub fn clip(&mut self, lib: &Library, path: &str) -> Result<Arc<BoundClip>, String> {
        if let Some(c) = self.clips.get(path) {
            return Ok(c.clone());
        }
        let m = lib.load_motion(path).map_err(|e| e.to_string())?;
        let fade = m.fade.as_ref().map(|f| f.fade_in);
        let bound = Arc::new(BoundClip::bind(&m, &self.info.ids));
        if let Some(f) = fade {
            self.fades.insert(bound.name.clone(), f);
        }
        self.clips.insert(path.to_owned(), bound.clone());
        Ok(bound)
    }

    /// `ScenarioModel.ChangeMotionCore` / facial change (`live2d.md` §3).
    pub fn play(&mut self, clip: Arc<BoundClip>, face: bool, first: bool) {
        let layer = if face { LAYER_FACE } else { LAYER_BODY };
        let blend = if first && self.animator.current(layer).is_none() {
            0.0
        } else if face {
            self.fades.get(&clip.name).copied().unwrap_or(consts::FACIAL_FADE)
        } else {
            let same_category = self
                .animator
                .current(layer)
                .is_some_and(|cur| category(cur) == category(&clip.name));
            if same_category {
                consts::SAME_CATEGORY_BLEND
            } else {
                self.fades
                    .get(&clip.name)
                    .copied()
                    .unwrap_or(consts::BODY_MOTION_FADE)
            }
        };
        self.animator.change(layer, clip, blend);
    }

    pub fn late_update(&mut self, frame: u32, dt: f32, tb: TimeBase) {
        let info = &self.info;
        for e in self.animator.advance(dt) {
            if e.function == "OnLive2DInvokeUserData" {
                self.blink.invoke_user_data(&e.data);
            }
        }
        let mut v = info.defaults.clone();
        self.animator.evaluate(&mut v, &info.defaults);
        self.blink.update(dt);
        for p in info.eyes.iter().flatten() {
            v[*p] *= self.blink.opening;
        }
        if let (Some(m), Some(value)) = (info.mouth, self.lip.value(frame, tb)) {
            v[m] = value;
        }
        if let Some(b) = info.breath {
            v[b] = self.breath_value;
        }
        for (i, x) in v.iter_mut().enumerate() {
            *x = x.clamp(info.min[i], info.max[i]);
        }
        if let Some(p) = &mut self.physics {
            p.evaluate(&mut v, &info.min, &info.max, dt);
        }
        self.values = v;
    }
}

/// `GetCategoryName`: not reversed; approximated as the second `-`-separated token of the
/// motion name (`w-<category>-<name>`).
fn category(name: &str) -> &str {
    name.split('-').nth(1).unwrap_or(name)
}
