//! Scenario effect prefabs (`SnippetActionSpecialEffect` cases 15 / 16).
//!
//! JP 6.8.1 `ScenarioPlayer.PlayScenarioEffect`: load bundle `StringValSub`, instantiate the
//! prefab `StringVal` under `effectLayer` (sorting order 260), `ScenarioEffector.Setup`, `Play`.
//! `StopScenarioEffect` calls `Stop` on every effector under `effectLayer` whose effect name
//! matches. Both run after the snippet's `Duration` (a delay; the snippet itself finishes at
//! once). `CommandAnimator` (the effector's base):
//! - `Play`: enable the root `Animator` (it plays its default state); `playOnEnable` does this
//!   on instantiation; particle systems with `playOnAwake` start by themselves.
//! - `Stop`: `ParticleSystem.Stop()` (stop emitting) on every child system, then play the
//!   Animator state `"Stop"` if the controller has such a clip (else disable the Animator),
//!   then `WaitAllStop`: once no particle is alive and the Stop clip's length has passed, the
//!   state is Stopped and `stopBehaviour == DestroyMySelf` destroys the object.
//!
//! This module reads a prefab from the SSR library (`_objects.json`, the clips as
//! `.sse-motion.json`), and evaluates one instance per frame into sorted quads.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::Value;
use sse_assets::SseMotion;
use sse_assets::motion::CurveData;
use sse_core::det_math::{cosf, sinf};

use crate::particle::{SystemDef, SystemState};

/// `EffectLayer`'s canvas sorting order (nested canvases without their own order use it).
const EFFECT_LAYER_ORDER: i32 = 260;

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn v2(v: &Value) -> [f32; 2] {
    [f(&v["x"]), f(&v["y"])]
}

fn v3(v: &Value) -> [f32; 3] {
    [f(&v["x"]), f(&v["y"]), f(&v["z"])]
}

fn pid(v: &Value) -> i64 {
    v["m_PathID"].as_i64().unwrap_or(0)
}

/// Where a texture came from: a library PNG and the sub-rect in it (u0, v0, u1, v1, v down).
#[derive(Debug, Clone)]
pub struct TexRef {
    pub png: String,
    pub uv: [f32; 4],
}

#[derive(Debug, Clone)]
struct Sprite {
    tex: TexRef,
    /// sprite rect size in pixels, pixels per unit, pivot (0..1)
    size: [f32; 2],
    ppu: f32,
    pivot: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Alpha,
    Additive,
}

#[derive(Debug, Clone)]
struct Material {
    tex: Option<TexRef>,
    blend: Blend,
}

#[derive(Debug, Clone)]
enum Xf {
    Rect {
        anchor_min: [f32; 2],
        anchor_max: [f32; 2],
        anchored: [f32; 2],
        size_delta: [f32; 2],
        pivot: [f32; 2],
    },
    Plain,
}

#[derive(Debug, Clone)]
struct Node {
    name: String,
    parent: Option<usize>,
    children: Vec<usize>,
    active: bool,
    xf: Xf,
    pos: [f32; 3],
    rot: [f32; 4],
    scale: [f32; 3],
    /// UI `Image` (sprite or plain colour) and its colour
    image: Option<(Option<Sprite>, [f32; 4])>,
    sprite_renderer: Option<(Sprite, [f32; 4], i32)>,
    /// `Canvas` with `overrideSorting`
    canvas_order: Option<i32>,
    /// (system index, material)
    system: Option<(usize, Option<Material>)>,
}

#[derive(Debug, Clone)]
struct Clip {
    length: f32,
    looping: bool,
    start: f32,
    /// (node, property, curve)
    curves: Vec<(usize, Prop, CurveData)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prop {
    Active,
    Color(usize),
    AnchoredX,
    AnchoredY,
    /// `Transform` localScale / localPosition component (the SSR converter names them)
    Scale(usize),
    Position(usize),
}

#[derive(Debug, Clone)]
struct State {
    name_hash: u32,
    clip: Option<usize>,
    speed: f32,
    /// exit-time transition: (destination state, normalized exit time, duration, fixed)
    exit: Option<(usize, f32, f32, bool)>,
}

#[derive(Debug, Clone)]
struct Animator {
    clips: Vec<Clip>,
    states: Vec<State>,
    default_state: usize,
}

/// A loaded prefab.
#[derive(Debug, Clone)]
pub struct Prefab {
    nodes: Vec<Node>,
    systems: Vec<SystemDef>,
    animator: Option<Animator>,
    stop_destroys: bool,
    play_on_enable: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("{0}: {1}")]
    Load(String, String),
}

fn load_json(path: &Path) -> Result<Value, EffectError> {
    let s = std::fs::read(path)
        .map_err(|e| EffectError::Load(path.display().to_string(), e.to_string()))?;
    serde_json::from_slice(&s)
        .map_err(|e| EffectError::Load(path.display().to_string(), e.to_string()))
}

/// Blend of a particle material: the built-in legacy particle shaders by file id
/// (10720 `Mobile/Particles/Additive`, 10721 `…/Alpha Blended`), the `shader/particles`
/// bundle's by name, else alpha blending.
fn blend_of(mat: &Value, shader_names: &BTreeMap<i64, String>) -> Blend {
    let id = pid(&mat["m_Shader"]);
    if id == 10720 {
        return Blend::Additive;
    }
    if let Some(n) = shader_names.get(&id) {
        let n = n.to_ascii_lowercase();
        if n.contains("add") && !n.contains("alpha") {
            return Blend::Additive;
        }
    }
    Blend::Alpha
}

impl Prefab {
    /// `dir` = the bundle's library directory, `name` = the prefab (`StringVal`).
    pub fn load(lib: &sse_assets::Library, bundle: &str, name: &str) -> Result<Self, EffectError> {
        let dir = lib.path(bundle);
        let objs = load_json(&dir.join("_objects.json"))?;
        let by: BTreeMap<i64, &Value> = objs
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| Some((k.parse::<i64>().ok()?, v)))
                    .collect()
            })
            .unwrap_or_default();
        let class = |id: i64| {
            by.get(&id)
                .and_then(|o| o["classId"].as_i64())
                .unwrap_or(-1)
        };
        let tree = |id: i64| by.get(&id).map(|o| &o["tree"]);
        // textures: pathId → png in this bundle
        let ripper = load_json(&dir.join("_ripper.json"))?;
        let mut pngs: BTreeMap<i64, String> = BTreeMap::new();
        for fl in ripper["files"].as_array().into_iter().flatten() {
            if fl["kind"] == "png"
                && let Some(p) = fl["path"].as_str()
            {
                pngs.insert(fl["pathId"].as_i64().unwrap_or(0), format!("{bundle}/{p}"));
            }
        }
        let shader_names = shader_names(lib);
        // a material may use a texture of a dependency bundle (e.g. `spot_light` → `snow`):
        // path ids are unique, so look through the sibling effect bundles
        let foreign = sibling_pngs(lib, bundle);
        let tex_ref = |tex_id: i64| -> Option<TexRef> {
            pngs.get(&tex_id)
                .or_else(|| foreign.get(&tex_id))
                .map(|p| TexRef {
                    png: p.clone(),
                    uv: [0.0, 0.0, 1.0, 1.0],
                })
        };
        let sprite = |sid: i64| -> Option<Sprite> {
            let s = tree(sid)?;
            let rd = &s["m_RD"];
            let tex_id = pid(&rd["texture"]);
            let t = tree(tex_id)?;
            let (tw, th) = (f(&t["m_Width"]).max(1.0), f(&t["m_Height"]).max(1.0));
            let r = &s["m_Rect"];
            let (x, y, w, h) = (f(&r["x"]), f(&r["y"]), f(&r["width"]), f(&r["height"]));
            let mut tex = tex_ref(tex_id)?;
            tex.uv = [x / tw, 1.0 - (y + h) / th, (x + w) / tw, 1.0 - y / th];
            Some(Sprite {
                tex,
                size: [w, h],
                ppu: f(&s["m_PixelsToUnits"]).max(1e-3),
                pivot: v2(&s["m_Pivot"]),
            })
        };
        let material = |mid: i64| -> Option<Material> {
            let m = tree(mid)?;
            let tex = m["m_SavedProperties"]["m_TexEnvs"]
                .as_array()
                .and_then(|envs| {
                    envs.iter()
                        .find(|e| e["key"] == "_MainTex")
                        .and_then(|e| tex_ref(pid(&e["value"]["m_Texture"])))
                });
            Some(Material {
                tex,
                blend: blend_of(m, &shader_names),
            })
        };

        // transforms, depth first from the root
        let tfs: Vec<i64> = by
            .keys()
            .copied()
            .filter(|&id| matches!(class(id), 4 | 224))
            .collect();
        let root_tf = tfs
            .iter()
            .copied()
            .find(|&id| tree(id).is_some_and(|t| pid(&t["m_Father"]) == 0))
            .ok_or_else(|| EffectError::Load(name.into(), "no root transform".into()))?;
        let mut nodes: Vec<Node> = Vec::new();
        let mut systems = Vec::new();
        let mut animator_ctrl = None;
        let mut stop_destroys = true;
        let mut play_on_enable = true;
        let mut stack = vec![(root_tf, None::<usize>)];
        while let Some((tf, parent)) = stack.pop() {
            let t = tree(tf).expect("transform");
            let go = tree(pid(&t["m_GameObject"])).expect("game object");
            let xf = if class(tf) == 224 {
                Xf::Rect {
                    anchor_min: v2(&t["m_AnchorMin"]),
                    anchor_max: v2(&t["m_AnchorMax"]),
                    anchored: v2(&t["m_AnchoredPosition"]),
                    size_delta: v2(&t["m_SizeDelta"]),
                    pivot: v2(&t["m_Pivot"]),
                }
            } else {
                Xf::Plain
            };
            let q = &t["m_LocalRotation"];
            let mut node = Node {
                name: go["m_Name"].as_str().unwrap_or("").to_owned(),
                parent,
                children: Vec::new(),
                active: go["m_IsActive"].as_bool().unwrap_or(true)
                    || go["m_IsActive"].as_i64() == Some(1),
                xf,
                pos: v3(&t["m_LocalPosition"]),
                rot: [f(&q["x"]), f(&q["y"]), f(&q["z"]), f(&q["w"])],
                scale: v3(&t["m_LocalScale"]),
                image: None,
                sprite_renderer: None,
                canvas_order: None,
                system: None,
            };
            let comps: Vec<i64> = go["m_Component"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| pid(&c["component"]))
                .collect();
            let mut ps = None;
            let mut psr = None;
            for c in comps {
                let Some(ct) = tree(c) else { continue };
                match class(c) {
                    198 => ps = Some(ct),
                    199 => psr = Some(ct),
                    223 => {
                        if ct["m_OverrideSorting"].as_bool().unwrap_or(false)
                            || ct["m_OverrideSorting"].as_i64() == Some(1)
                        {
                            node.canvas_order =
                                Some(ct["m_SortingOrder"].as_i64().unwrap_or(0) as i32);
                        }
                    }
                    212 => {
                        if ct["m_Enabled"].as_i64().unwrap_or(1) != 0
                            && let Some(s) = sprite(pid(&ct["m_Sprite"]))
                        {
                            let c4 = &ct["m_Color"];
                            node.sprite_renderer = Some((
                                s,
                                [f(&c4["r"]), f(&c4["g"]), f(&c4["b"]), f(&c4["a"])],
                                ct["m_SortingOrder"].as_i64().unwrap_or(0) as i32,
                            ));
                        }
                    }
                    95 => animator_ctrl = Some(pid(&ct["m_Controller"])),
                    114 => {
                        let script = tree(pid(&ct["m_Script"]))
                            .and_then(|s| s["m_ClassName"].as_str())
                            .unwrap_or("");
                        match script {
                            "Image" | "CustomImage" | "AtlasImage"
                                if ct["m_Enabled"].as_i64().unwrap_or(1) != 0 =>
                            {
                                let c4 = &ct["m_Color"];
                                node.image = Some((
                                    sprite(pid(&ct["m_Sprite"])),
                                    [f(&c4["r"]), f(&c4["g"]), f(&c4["b"]), f(&c4["a"])],
                                ));
                            }
                            "ScenarioEffector" => {
                                stop_destroys = ct["stopBehaviour"].as_i64().unwrap_or(1) == 1;
                                play_on_enable = ct["playOnEnable"].as_i64().unwrap_or(1) != 0;
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
            if let (Some(ps), Some(r)) = (ps, psr)
                && r["m_Enabled"].as_i64().unwrap_or(1) != 0
            {
                let seed = crc32fast::hash(node.name.as_bytes())
                    ^ (systems.len() as u32).wrapping_mul(0x9E37_79B9);
                let def = SystemDef::parse(ps, r, seed);
                let mat = r["m_Materials"]
                    .as_array()
                    .and_then(|m| m.first())
                    .and_then(|m| material(pid(m)));
                node.system = Some((systems.len(), mat));
                systems.push(def);
            }
            let me = nodes.len();
            if let Some(p) = parent {
                nodes[p].children.push(me);
            }
            nodes.push(node);
            // children in sibling order (stack: push reversed)
            let kids: Vec<i64> = t["m_Children"]
                .as_array()
                .into_iter()
                .flatten()
                .map(pid)
                .collect();
            for k in kids.into_iter().rev() {
                stack.push((k, Some(me)));
            }
        }
        // depth-first push order gives sibling order already (children pushed after parent)

        let animator = match animator_ctrl {
            Some(ctrl) => Some(load_animator(&dir, &by, ctrl, &nodes)?),
            None => None,
        };
        Ok(Prefab {
            nodes,
            systems,
            animator,
            stop_destroys,
            play_on_enable,
        })
    }
}

/// pathId → png of every bundle next to `bundle` (same parent directory in the library).
fn sibling_pngs(lib: &sse_assets::Library, bundle: &str) -> BTreeMap<i64, String> {
    let mut out = BTreeMap::new();
    let parent = bundle.rsplit_once('/').map_or("", |(p, _)| p);
    let Ok(entries) = sse_core::fs::read_dir_sorted(&lib.path(parent)) else {
        return out;
    };
    for dir in entries {
        let Some(name) = dir.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        let Ok(r) = load_json(&dir.join("_ripper.json")) else {
            continue;
        };
        for fl in r["files"].as_array().into_iter().flatten() {
            if fl["kind"] == "png"
                && let (Some(id), Some(p)) = (fl["pathId"].as_i64(), fl["path"].as_str())
            {
                out.insert(id, format!("{parent}/{name}/{p}"));
            }
        }
    }
    out
}

fn shader_names(lib: &sse_assets::Library) -> BTreeMap<i64, String> {
    let mut out = BTreeMap::new();
    if let Ok(r) = load_json(&lib.path("shader/particles").join("_ripper.json")) {
        for fl in r["files"].as_array().into_iter().flatten() {
            if let (Some(id), Some(p)) = (fl["pathId"].as_i64(), fl["path"].as_str()) {
                out.insert(id, p.to_owned());
            }
        }
    }
    out
}

/// Relative path of each node from the root (`Animator` binding paths).
fn node_paths(nodes: &[Node]) -> Vec<String> {
    let mut out = vec![String::new(); nodes.len()];
    for i in 1..nodes.len() {
        let p = nodes[i].parent.unwrap_or(0);
        out[i] = if out[p].is_empty() {
            nodes[i].name.clone()
        } else {
            format!("{}/{}", out[p], nodes[i].name)
        };
    }
    out
}

fn load_animator(
    dir: &Path,
    by: &BTreeMap<i64, &Value>,
    ctrl: i64,
    nodes: &[Node],
) -> Result<Animator, EffectError> {
    let c = &by
        .get(&ctrl)
        .ok_or_else(|| EffectError::Load(dir.display().to_string(), "controller".into()))?["tree"];
    // clips: pathId → sse-motion file (`source.pathId`)
    let mut motions: BTreeMap<i64, SseMotion> = BTreeMap::new();
    if let Ok(files) = sse_core::fs::read_dir_sorted(dir) {
        for p in files {
            if p.to_string_lossy().ends_with(".sse-motion.json")
                && let Ok(m) = sse_assets::read_json::<SseMotion>(&p)
            {
                motions.insert(m.source.path_id, m);
            }
        }
    }
    let paths = node_paths(nodes);
    let by_hash: BTreeMap<u32, usize> = paths
        .iter()
        .enumerate()
        .map(|(i, p)| (crc32fast::hash(p.as_bytes()), i))
        .collect();
    let prop_of = |type_id: i64, attr: u32| -> Option<Prop> {
        let h = |s: &str| crc32fast::hash(s.as_bytes());
        match type_id {
            1 if attr == h("m_IsActive") => Some(Prop::Active),
            114 if attr == h("m_Color.r") => Some(Prop::Color(0)),
            114 if attr == h("m_Color.g") => Some(Prop::Color(1)),
            114 if attr == h("m_Color.b") => Some(Prop::Color(2)),
            114 if attr == h("m_Color.a") => Some(Prop::Color(3)),
            224 if attr == h("m_AnchoredPosition.x") => Some(Prop::AnchoredX),
            224 if attr == h("m_AnchoredPosition.y") => Some(Prop::AnchoredY),
            4 => ["m_LocalScale.x", "m_LocalScale.y", "m_LocalScale.z"]
                .iter()
                .position(|n| attr == h(n))
                .map(Prop::Scale)
                .or_else(|| {
                    [
                        "m_LocalPosition.x",
                        "m_LocalPosition.y",
                        "m_LocalPosition.z",
                    ]
                    .iter()
                    .position(|n| attr == h(n))
                    .map(Prop::Position)
                }),
            _ => None,
        }
    };
    let clip_ids: Vec<i64> = c["m_AnimationClips"]
        .as_array()
        .into_iter()
        .flatten()
        .map(pid)
        .collect();
    let clips: Vec<Clip> = clip_ids
        .iter()
        .map(|id| match motions.get(id) {
            Some(m) => Clip {
                length: m.stop_time - m.start_time,
                looping: m.loop_time,
                start: m.start_time,
                curves: m
                    .curves
                    .iter()
                    .filter_map(|cv| {
                        let node = *by_hash.get(&cv.binding.path_hash)?;
                        let prop = prop_of(cv.binding.type_id as i64, cv.binding.attr_hash)?;
                        Some((node, prop, cv.data.clone()))
                    })
                    .collect(),
            },
            None => Clip {
                length: 0.0,
                looping: false,
                start: 0.0,
                curves: Vec::new(),
            },
        })
        .collect();
    let sm = &c["m_Controller"]["m_StateMachineArray"][0]["data"];
    let states = sm["m_StateConstantArray"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let d = &s["data"];
            let clip = d["m_BlendTreeConstantArray"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|bt| bt["data"]["m_NodeArray"].as_array()?.first().cloned())
                .and_then(|n| n["data"]["m_ClipID"].as_u64())
                .map(|i| i as usize)
                .filter(|&i| i < clips.len());
            let exit = d["m_TransitionConstantArray"].as_array().and_then(|ts| {
                ts.iter().find_map(|t| {
                    let t = &t["data"];
                    let has_exit = t["m_HasExitTime"].as_bool().unwrap_or(false)
                        || t["m_HasExitTime"].as_i64() == Some(1);
                    let conds = t["m_ConditionConstantArray"]
                        .as_array()
                        .map_or(0, |a| a.len());
                    (has_exit && conds == 0).then(|| {
                        (
                            t["m_DestinationState"].as_u64().unwrap_or(0) as usize,
                            f(&t["m_ExitTime"]),
                            f(&t["m_TransitionDuration"]),
                            t["m_HasFixedDuration"].as_bool().unwrap_or(false)
                                || t["m_HasFixedDuration"].as_i64() == Some(1),
                        )
                    })
                })
            });
            State {
                name_hash: d["m_NameID"].as_u64().unwrap_or(0) as u32,
                clip,
                speed: d.get("m_Speed").map_or(1.0, f),
                exit,
            }
        })
        .collect();
    Ok(Animator {
        clips,
        states,
        default_state: sm["m_DefaultState"].as_u64().unwrap_or(0) as usize,
    })
}

// ------------------------------------------------------------------ instance

/// One drawn quad of an effect, in reference-canvas pixels (origin at the canvas centre,
/// y up), with its sort key.
#[derive(Debug, Clone)]
pub struct EffectQuad {
    pub order: i32,
    pub corners: [[f32; 2]; 4],
    /// u0, v0, u1, v1 with v down (image rows)
    pub uv: [f32; 4],
    pub color: [f32; 4],
    pub tex: Option<String>,
    pub blend: Blend,
}

/// The Animator's playing state: current state, time in it, and an optional crossfade.
#[derive(Debug, Clone)]
struct AnimPlay {
    state: usize,
    time: f32,
    /// (next state, time in next, fade elapsed, fade length)
    fade: Option<(usize, f32, f32, f32)>,
}

pub struct EffectInstance {
    prefab: std::sync::Arc<Prefab>,
    systems: Vec<SystemState>,
    /// Node activity after the Animator.
    active: Vec<bool>,
    color: Vec<[f32; 4]>,
    anchored: Vec<[f32; 2]>,
    scale: Vec<[f32; 3]>,
    pos: Vec<[f32; 3]>,
    anim: Option<AnimPlay>,
    time: f32,
    stopped_at: Option<f32>,
    stop_wait: f32,
    pub finished: bool,
    dt: f32,
}

impl EffectInstance {
    pub fn new(prefab: std::sync::Arc<Prefab>, seed: u32, dt: f32) -> Self {
        let n = prefab.nodes.len();
        let mut inst = EffectInstance {
            systems: prefab
                .systems
                .iter()
                .map(|d| SystemState::new(d, seed))
                .collect(),
            active: prefab.nodes.iter().map(|n| n.active).collect(),
            color: prefab
                .nodes
                .iter()
                .map(|n| n.image.as_ref().map_or([1.0; 4], |i| i.1))
                .collect(),
            anchored: prefab
                .nodes
                .iter()
                .map(|n| match n.xf {
                    Xf::Rect { anchored, .. } => anchored,
                    Xf::Plain => [0.0; 2],
                })
                .collect(),
            scale: prefab.nodes.iter().map(|n| n.scale).collect(),
            pos: prefab.nodes.iter().map(|n| n.pos).collect(),
            anim: prefab
                .play_on_enable
                .then_some(())
                .and(prefab.animator.as_ref())
                .map(|a| AnimPlay {
                    state: a.default_state,
                    time: 0.0,
                    fade: None,
                }),
            prefab,
            time: 0.0,
            stopped_at: None,
            stop_wait: 0.0,
            finished: false,
            dt,
        };
        let _ = n;
        inst.apply_animator();
        // `playOnAwake` systems in the active hierarchy start on instantiation
        for i in 0..inst.prefab.nodes.len() {
            if let Some((s, _)) = inst.prefab.nodes[i].system
                && inst.effective_active(i)
                && inst.prefab.systems[s].play_on_awake
            {
                let def = inst.prefab.systems[s].clone();
                inst.systems[s].play(&def, dt);
            }
        }
        inst
    }

    fn effective_active(&self, mut i: usize) -> bool {
        loop {
            if !self.active[i] {
                return false;
            }
            match self.prefab.nodes[i].parent {
                Some(p) => i = p,
                None => return true,
            }
        }
    }

    /// `Stop()`: systems stop emitting, the Animator switches to "Stop".
    pub fn stop(&mut self) {
        if self.stopped_at.is_some() {
            return;
        }
        self.stopped_at = Some(self.time);
        for s in &mut self.systems {
            s.stop_emitting();
        }
        let stop_hash = crc32fast::hash(b"Stop");
        if let Some(a) = &self.prefab.animator {
            match a
                .states
                .iter()
                .position(|s| s.name_hash == stop_hash && s.clip.is_some())
            {
                Some(st) => {
                    self.stop_wait = a.clips[a.states[st].clip.unwrap_or(0)].length;
                    self.anim = Some(AnimPlay {
                        state: st,
                        time: 0.0,
                        fade: None,
                    });
                }
                // no Stop clip: the Animator is disabled, its last pose stays
                None => self.anim = None,
            }
        }
    }

    /// Advances one frame.
    #[allow(clippy::needless_range_loop)] // nodes, their systems and `before` in parallel
    pub fn step(&mut self) {
        if self.finished {
            return;
        }
        let dt = self.dt;
        self.time += dt;
        let before: Vec<bool> = (0..self.prefab.nodes.len())
            .map(|i| self.effective_active(i))
            .collect();
        if let (Some(play), Some(a)) = (self.anim.as_mut(), self.prefab.animator.as_ref()) {
            advance(play, a, dt);
        }
        self.apply_animator();
        for i in 0..self.prefab.nodes.len() {
            let Some((s, _)) = self.prefab.nodes[i].system else {
                continue;
            };
            let now = self.effective_active(i);
            let def = &self.prefab.systems[s];
            if now && !before[i] {
                if def.play_on_awake {
                    self.systems[s].play(def, dt);
                    if self.stopped_at.is_some() {
                        self.systems[s].stop_emitting();
                    }
                }
            } else if !now && before[i] {
                self.systems[s].clear();
            }
            if now {
                self.systems[s].step(def, dt);
            }
        }
        if let Some(at) = self.stopped_at {
            let alive = self
                .systems
                .iter()
                .zip(&self.prefab.systems)
                .any(|(s, d)| s.is_alive(d));
            if !alive && self.time - at >= self.stop_wait && self.prefab.stop_destroys {
                self.finished = true;
            }
        }
    }

    fn apply_animator(&mut self) {
        let (Some(play), Some(a)) = (self.anim.as_ref(), self.prefab.animator.as_ref()) else {
            return;
        };
        let mut vals: BTreeMap<(usize, Prop), f32> = BTreeMap::new();
        let sample = |state: usize, t: f32, w: f32, vals: &mut BTreeMap<(usize, Prop), f32>| {
            let Some(ci) = a.states[state].clip else {
                return;
            };
            let c = &a.clips[ci];
            let lt = if c.looping && c.length > 0.0 {
                t % c.length
            } else {
                t.min(c.length)
            };
            for (node, prop, curve) in &c.curves {
                let v = curve.evaluate(c.start + lt);
                let e = vals.entry((*node, *prop)).or_insert(0.0);
                *e += v * w;
            }
        };
        match play.fade {
            None => sample(play.state, play.time, 1.0, &mut vals),
            Some((next, nt, el, len)) => {
                let w = if len > 0.0 {
                    (el / len).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                sample(play.state, play.time, 1.0 - w, &mut vals);
                sample(next, nt, w, &mut vals);
            }
        }
        for ((node, prop), v) in vals {
            match prop {
                Prop::Active => self.active[node] = v > 0.5,
                Prop::Color(c) => self.color[node][c] = v,
                Prop::AnchoredX => self.anchored[node][0] = v,
                Prop::AnchoredY => self.anchored[node][1] = v,
                Prop::Scale(k) => self.scale[node][k] = v,
                Prop::Position(k) => self.pos[node][k] = v,
            }
        }
    }

    /// World (canvas-pixel) matrices of every node: 2×3 affine [a b tx; c d ty] plus each
    /// node's rect in its own space. `parent` is the root's parent: its matrix and its rect
    /// in its own space.
    fn layout(&self, parent: &Parent) -> (Vec<[f32; 6]>, Vec<[f32; 4]>) {
        let n = self.prefab.nodes.len();
        let mut world = vec![[1.0, 0.0, 0.0, 0.0, 1.0, 0.0]; n];
        let mut rects = vec![[0.0; 4]; n];
        for i in 0..n {
            let node = &self.prefab.nodes[i];
            let (pw, prect) = match node.parent {
                Some(p) => (world[p], rects[p]),
                None => (parent.matrix, parent.rect),
            };
            let (local_pos, rect) = match &node.xf {
                Xf::Rect {
                    anchor_min,
                    anchor_max,
                    size_delta,
                    pivot,
                    ..
                } => {
                    let a = self.anchored[i];
                    let size = [
                        (anchor_max[0] - anchor_min[0]) * prect[2] + size_delta[0],
                        (anchor_max[1] - anchor_min[1]) * prect[3] + size_delta[1],
                    ];
                    let refp = [
                        prect[0]
                            + prect[2]
                                * (anchor_min[0] + (anchor_max[0] - anchor_min[0]) * pivot[0]),
                        prect[1]
                            + prect[3]
                                * (anchor_min[1] + (anchor_max[1] - anchor_min[1]) * pivot[1]),
                    ];
                    (
                        [refp[0] + a[0], refp[1] + a[1]],
                        [-pivot[0] * size[0], -pivot[1] * size[1], size[0], size[1]],
                    )
                }
                Xf::Plain => ([self.pos[i][0], self.pos[i][1]], [0.0; 4]),
            };
            // z rotation of the quaternion (the prefabs only turn about z)
            let q = node.rot;
            let ang = 2.0 * q[2].atan2(q[3]);
            let (s, c) = (sinf(ang), cosf(ang));
            let (sx, sy) = (self.scale[i][0], self.scale[i][1]);
            let l = [c * sx, -s * sy, local_pos[0], s * sx, c * sy, local_pos[1]];
            world[i] = mul(pw, l);
            rects[i] = rect;
        }
        (world, rects)
    }

    /// Quads to draw this frame under `effectLayer` (the whole canvas, pivot at its centre),
    /// sorted back to front.
    pub fn quads(&self, canvas: [f32; 2]) -> Vec<EffectQuad> {
        let root = Parent {
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            rect: [-canvas[0] * 0.5, -canvas[1] * 0.5, canvas[0], canvas[1]],
        };
        self.quads_under(canvas, &root)
    }

    /// Quads to draw this frame with the prefab root under `parent`, sorted back to front.
    pub fn quads_under(&self, canvas: [f32; 2], parent: &Parent) -> Vec<EffectQuad> {
        if self.finished {
            return Vec::new();
        }
        let (world, rects) = self.layout(parent);
        let wu = canvas[1] * 0.5; // canvas pixels per scenario-camera world unit
        let mut out: Vec<(i32, f32, usize, EffectQuad)> = Vec::new();
        let mut seq = 0usize;
        for i in 0..self.prefab.nodes.len() {
            if !self.effective_active(i) {
                continue;
            }
            let node = &self.prefab.nodes[i];
            let canvas_order = self.canvas_order(i);
            let w = world[i];
            let tf = |p: [f32; 2]| {
                [
                    w[0] * p[0] + w[1] * p[1] + w[2],
                    w[3] * p[0] + w[4] * p[1] + w[5],
                ]
            };
            if let Some((sprite, _)) = &node.image {
                let r = rects[i];
                let corners = [
                    tf([r[0], r[1]]),
                    tf([r[0] + r[2], r[1]]),
                    tf([r[0] + r[2], r[1] + r[3]]),
                    tf([r[0], r[1] + r[3]]),
                ];
                let (tex, uv) = match sprite {
                    Some(s) => (Some(s.tex.png.clone()), s.tex.uv),
                    None => (None, [0.0, 0.0, 1.0, 1.0]),
                };
                out.push((
                    canvas_order,
                    0.0,
                    seq,
                    EffectQuad {
                        order: canvas_order,
                        corners,
                        uv,
                        color: self.color[i],
                        tex,
                        blend: Blend::Alpha,
                    },
                ));
                seq += 1;
            }
            if let Some((s, col, order)) = &node.sprite_renderer {
                let (sw, sh) = (s.size[0] / s.ppu, s.size[1] / s.ppu);
                let (x0, y0) = (-s.pivot[0] * sw, -s.pivot[1] * sh);
                let corners = [
                    tf([x0, y0]),
                    tf([x0 + sw, y0]),
                    tf([x0 + sw, y0 + sh]),
                    tf([x0, y0 + sh]),
                ];
                out.push((
                    *order,
                    -node.pos[2],
                    seq,
                    EffectQuad {
                        order: *order,
                        corners,
                        uv: s.tex.uv,
                        color: *col,
                        tex: Some(s.tex.png.clone()),
                        blend: Blend::Alpha,
                    },
                ));
                seq += 1;
            }
            if let Some((si, mat)) = &node.system {
                let def = &self.prefab.systems[*si];
                // `ScalingMode.Local`: position from the hierarchy, the system's own scale only
                let origin = [w[2], w[5]];
                let (sx, sy) = (self.scale[i][0], self.scale[i][1]);
                let tex = mat.as_ref().and_then(|m| m.tex.clone());
                let blend = mat.as_ref().map_or(Blend::Alpha, |m| m.blend);
                for (corners, uv, color) in self.systems[*si].quads(def) {
                    let px =
                        corners.map(|c| [origin[0] + c[0] * sx * wu, origin[1] + c[1] * sy * wu]);
                    let base = tex.as_ref().map_or([0.0, 0.0, 1.0, 1.0], |t| t.uv);
                    // particle UVs have v up; convert into the texture's (v down) sub-rect
                    let bu = |u: f32| base[0] + (base[2] - base[0]) * u;
                    let bv = |v: f32| base[1] + (base[3] - base[1]) * (1.0 - v);
                    let quad_uv = [bu(uv[0]), bv(uv[3]), bu(uv[2]), bv(uv[1])];
                    out.push((
                        def.sorting_order,
                        -node.pos[2],
                        seq,
                        EffectQuad {
                            order: def.sorting_order,
                            corners: px,
                            uv: quad_uv,
                            color,
                            tex: tex.as_ref().map(|t| t.png.clone()),
                            blend,
                        },
                    ));
                }
                seq += 1;
            }
        }
        // sorting order, then farther (larger z) first, then hierarchy order
        out.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2)));
        out.into_iter().map(|q| q.3).collect()
    }

    fn canvas_order(&self, mut i: usize) -> i32 {
        loop {
            if let Some(o) = self.prefab.nodes[i].canvas_order {
                return o;
            }
            match self.prefab.nodes[i].parent {
                Some(p) => i = p,
                None => return EFFECT_LAYER_ORDER,
            }
        }
    }
}

/// The transform an instance hangs off, in canvas pixels (origin at the canvas centre, y up).
#[derive(Debug, Clone, Copy)]
pub struct Parent {
    /// 2×3 affine [a b tx; c d ty] of the parent.
    pub matrix: [f32; 6],
    /// The parent's rect in its own space (x, y, w, h), for the root's anchors.
    pub rect: [f32; 4],
}

fn mul(a: [f32; 6], b: [f32; 6]) -> [f32; 6] {
    [
        a[0] * b[0] + a[1] * b[3],
        a[0] * b[1] + a[1] * b[4],
        a[0] * b[2] + a[1] * b[5] + a[2],
        a[3] * b[0] + a[4] * b[3],
        a[3] * b[1] + a[4] * b[4],
        a[3] * b[2] + a[4] * b[5] + a[5],
    ]
}

/// One Animator frame: state time, exit-time transitions with their crossfade.
fn advance(play: &mut AnimPlay, a: &Animator, dt: f32) {
    let len = |s: usize| {
        a.states[s]
            .clip
            .map_or(0.0, |c| a.clips[c].length)
            .max(1e-4)
    };
    play.time += dt * a.states[play.state].speed;
    if let Some((next, nt, el, fl)) = play.fade.as_mut() {
        *nt += dt * a.states[*next].speed;
        *el += dt;
        if *el >= *fl {
            let (n, t) = (*next, *nt);
            play.state = n;
            play.time = t;
            play.fade = None;
        }
        return;
    }
    if let Some((dest, exit, dur, fixed)) = a.states[play.state].exit
        && play.time / len(play.state) >= exit
    {
        let fl = if fixed { dur } else { dur * len(play.state) };
        play.fade = Some((dest, 0.0, 0.0, fl));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_composition_applies_the_child_after_the_parent() {
        let parent = [2.0, 0.0, 10.0, 0.0, 2.0, 20.0];
        let child = [1.0, 0.0, 1.0, 0.0, 1.0, 1.0];
        let m = mul(parent, child);
        assert_eq!([m[2], m[5]], [12.0, 22.0]);
    }
}
