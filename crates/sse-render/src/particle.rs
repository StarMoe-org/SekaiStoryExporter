//! A Unity `ParticleSystem` read from its serialized modules (the SSR typetree of a prefab)
//! and simulated on the CPU, for the scenario effect prefabs (`PlayScenarioEffect`).
//!
//! Aim: the game's look — counts, timing, motion, colours, sizes, textures, render modes —
//! not Unity's exact particles (its PRNG cannot be replayed). Modules implemented: initial,
//! emission (rate + bursts), shape (sphere / hemisphere / cone / box / circle / edge /
//! rectangle), velocity / clamp velocity / size / colour / rotation over lifetime, noise (value
//! noise, see `fx.rs`), texture sheet animation; renderer billboard / stretched / mesh (quad).
//!
//! Units: every prefab here uses `ScalingMode.Local` and the local simulation space, so a
//! particle lives in the emitter's local frame in world units of the scenario camera
//! (orthographic size 1: the screen is 2 units tall), ignoring the canvas scale above it.

use serde_json::Value;
use sse_core::det_math::{cosf, sinf};

// ------------------------------------------------------------------ serialized value types

type Key = (f32, f32, f32, f32);

fn f(v: &Value) -> f32 {
    v.as_f64().unwrap_or(0.0) as f32
}

fn fv(v: &Value, k: &str) -> f32 {
    f(&v[k])
}

fn bv(v: &Value, k: &str) -> bool {
    match &v[k] {
        Value::Bool(b) => *b,
        x => x.as_i64().unwrap_or(0) != 0,
    }
}

fn v3(v: &Value) -> [f32; 3] {
    [fv(v, "x"), fv(v, "y"), fv(v, "z")]
}

fn keys(curve: &Value, scale: f32) -> Vec<Key> {
    curve["m_Curve"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|k| {
                    (
                        fv(k, "time"),
                        fv(k, "value") * scale,
                        fv(k, "inSlope") * scale,
                        fv(k, "outSlope") * scale,
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Cubic Hermite between keys, clamped outside the range.
fn hermite(keys: &[Key], t: f32) -> f32 {
    match keys {
        [] => 0.0,
        [k] => k.1,
        _ => {
            if t <= keys[0].0 {
                return keys[0].1;
            }
            for w in keys.windows(2) {
                let (t0, v0, _, s0) = w[0];
                let (t1, v1, s1, _) = w[1];
                if t <= t1 {
                    let dt = t1 - t0;
                    if dt <= 1e-9 {
                        return v1;
                    }
                    // Unity marks stepped keys with infinite tangents
                    if !s0.is_finite() || !s1.is_finite() {
                        return v0;
                    }
                    let u = (t - t0) / dt;
                    let (u2, u3) = (u * u, u * u * u);
                    return (2.0 * u3 - 3.0 * u2 + 1.0) * v0
                        + (u3 - 2.0 * u2 + u) * dt * s0
                        + (-2.0 * u3 + 3.0 * u2) * v1
                        + (u3 - u2) * dt * s1;
                }
            }
            keys[keys.len() - 1].1
        }
    }
}

/// `MinMaxCurve`.
#[derive(Debug, Clone)]
pub enum Curve {
    Const(f32),
    Keys(Vec<Key>),
    TwoCurves(Vec<Key>, Vec<Key>),
    TwoConsts(f32, f32),
}

impl Curve {
    pub fn parse(v: &Value) -> Self {
        let scalar = fv(v, "scalar");
        match v["minMaxState"].as_i64().unwrap_or(0) {
            1 => Curve::Keys(keys(&v["maxCurve"], scalar)),
            2 => Curve::TwoCurves(keys(&v["minCurve"], scalar), keys(&v["maxCurve"], scalar)),
            3 => Curve::TwoConsts(fv(v, "minScalar"), scalar),
            _ => Curve::Const(scalar),
        }
    }

    /// `rnd` is the particle's fixed random for this property; `t` the normalized time.
    pub fn eval(&self, rnd: f32, t: f32) -> f32 {
        match self {
            Curve::Const(c) => *c,
            Curve::Keys(k) => hermite(k, t),
            Curve::TwoCurves(a, b) => {
                let (x, y) = (hermite(a, t), hermite(b, t));
                x + (y - x) * rnd
            }
            Curve::TwoConsts(a, b) => a + (b - a) * rnd,
        }
    }
}

/// `Gradient`: up to 8 colour and 8 alpha keys (times as u16 / 65535), blend or fixed mode.
#[derive(Debug, Clone)]
pub struct Gradient {
    colors: Vec<(f32, [f32; 3])>,
    alphas: Vec<(f32, f32)>,
    fixed: bool,
}

impl Gradient {
    fn parse(g: &Value) -> Self {
        let nc = g["m_NumColorKeys"].as_u64().unwrap_or(0) as usize;
        let na = g["m_NumAlphaKeys"].as_u64().unwrap_or(0) as usize;
        let key = |i: usize| &g[format!("key{i}")];
        Gradient {
            colors: (0..nc)
                .map(|i| {
                    (
                        fv(g, &format!("ctime{i}")) / 65535.0,
                        [fv(key(i), "r"), fv(key(i), "g"), fv(key(i), "b")],
                    )
                })
                .collect(),
            alphas: (0..na)
                .map(|i| (fv(g, &format!("atime{i}")) / 65535.0, fv(key(i), "a")))
                .collect(),
            fixed: g["m_Mode"].as_i64() == Some(1),
        }
    }

    fn at(&self, t: f32) -> [f32; 4] {
        fn track<const N: usize>(keys: &[(f32, [f32; N])], t: f32, fixed: bool) -> [f32; N] {
            match keys {
                [] => [1.0; N],
                [k] => k.1,
                _ => {
                    if t <= keys[0].0 {
                        return keys[0].1;
                    }
                    for w in keys.windows(2) {
                        if t <= w[1].0 {
                            if fixed {
                                return w[1].1;
                            }
                            let u = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-6);
                            let mut out = w[0].1;
                            for (o, (a, b)) in out.iter_mut().zip(w[0].1.iter().zip(w[1].1.iter()))
                            {
                                *o = a + (b - a) * u;
                            }
                            return out;
                        }
                    }
                    keys[keys.len() - 1].1
                }
            }
        }
        let c = track(&self.colors, t, self.fixed);
        let al: Vec<(f32, [f32; 1])> = self.alphas.iter().map(|&(t, a)| (t, [a])).collect();
        let a = track(&al, t, self.fixed);
        [c[0], c[1], c[2], a[0]]
    }
}

/// `MinMaxGradient`.
#[derive(Debug, Clone)]
pub enum ColorSpec {
    Color([f32; 4]),
    Gradient(Gradient),
    TwoColors([f32; 4], [f32; 4]),
    TwoGradients(Gradient, Gradient),
    /// `ParticleSystemGradientMode.RandomColor`: the gradient at a random point.
    RandomColor(Gradient),
}

fn rgba(v: &Value) -> [f32; 4] {
    [fv(v, "r"), fv(v, "g"), fv(v, "b"), fv(v, "a")]
}

impl ColorSpec {
    fn parse(v: &Value) -> Self {
        match v["minMaxState"].as_i64().unwrap_or(0) {
            1 => ColorSpec::Gradient(Gradient::parse(&v["maxGradient"])),
            4 => ColorSpec::RandomColor(Gradient::parse(&v["maxGradient"])),
            2 => ColorSpec::TwoColors(rgba(&v["minColor"]), rgba(&v["maxColor"])),
            3 => ColorSpec::TwoGradients(
                Gradient::parse(&v["minGradient"]),
                Gradient::parse(&v["maxGradient"]),
            ),
            _ => ColorSpec::Color(rgba(&v["maxColor"])),
        }
    }

    fn eval(&self, rnd: f32, t: f32) -> [f32; 4] {
        let mix = |a: [f32; 4], b: [f32; 4]| {
            let mut o = a;
            for i in 0..4 {
                o[i] = a[i] + (b[i] - a[i]) * rnd;
            }
            o
        };
        match self {
            ColorSpec::Color(c) => *c,
            ColorSpec::Gradient(g) => g.at(t),
            ColorSpec::TwoColors(a, b) => mix(*a, *b),
            ColorSpec::TwoGradients(a, b) => mix(a.at(t), b.at(t)),
            ColorSpec::RandomColor(g) => g.at(rnd),
        }
    }
}

// ------------------------------------------------------------------ system definition

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RenderMode {
    Billboard,
    Stretch,
    Mesh,
}

#[derive(Debug, Clone)]
struct Burst {
    time: f32,
    count: Curve,
    cycles: u32,
    interval: f32,
    probability: f32,
}

#[derive(Debug, Clone)]
struct Shape {
    enabled: bool,
    kind: i64,
    radius: f32,
    radius_thickness: f32,
    angle: f32,
    arc: f32,
    box_thickness: [f32; 3],
    position: [f32; 3],
    rotation: [f32; 3],
    scale: [f32; 3],
    random_direction: f32,
    spherical_direction: f32,
}

#[derive(Debug, Clone)]
pub struct SystemDef {
    pub duration: f32,
    pub looping: bool,
    pub prewarm: bool,
    pub play_on_awake: bool,
    start_delay: Curve,
    sim_speed: f32,
    lifetime: Curve,
    speed: Curve,
    size: Curve,
    size3d: Option<(Curve, Curve)>,
    rotation: Curve,
    /// `rotation3D`: start rotation about x and y (radians).
    rotation_xy: Option<[Curve; 2]>,
    color: ColorSpec,
    gravity: Curve,
    max_particles: usize,
    rate: Curve,
    bursts: Vec<Burst>,
    shape: Shape,
    velocity: Option<[Curve; 4]>,
    clamp: Option<(Curve, f32)>,
    size_ol: Option<(Curve, Option<Curve>)>,
    color_ol: Option<ColorSpec>,
    rotation_ol: Option<Curve>,
    /// `RotationModule.separateAxes`: angular velocity about x and y.
    rotation_ol_xy: Option<[Curve; 2]>,
    /// Mesh render mode geometry (the built-in quad unless the prefab loader sets one).
    pub mesh: Option<std::sync::Arc<Mesh>>,
    /// `SubModule` Birth sub-emitters: their `ParticleSystem` path ids.
    pub sub_birth: Vec<i64>,
    noise: Option<(Curve, f32, bool)>,
    /// tiles x, tiles y, frame over time, start frame, cycles, single row
    sheet: Option<(u32, u32, Curve, Curve, f32, Option<u32>)>,
    pub render_mode: RenderMode,
    length_scale: f32,
    velocity_scale: f32,
    max_particle_size: f32,
    pub sorting_order: i32,
    /// `Custom1.x` as the renderer streams it (`CustomDataModule` vector 0, x), when the
    /// renderer's vertex streams include `Custom1X`; see [`SystemDef::set_custom1x`].
    custom1x: Option<Curve>,
    seed: u32,
}

/// `ParticleSystemVertexStream.Custom1X`.
const CUSTOM1X_STREAM: i64 = 31;

impl SystemDef {
    /// `ParticleShaderSettings.UpdateMode` (on `Awake`): adds the `Custom1X` stream and sets
    /// custom data vector 0 to the constant 1 (`Mode.Additive`) or 0 (`Mode.AlphaBlend`).
    pub fn set_custom1x(&mut self, value: f32) {
        self.custom1x = Some(Curve::Const(value));
    }

    /// `ps` = the ParticleSystem typetree, `r` = its ParticleSystemRenderer's.
    pub fn parse(ps: &Value, r: &Value, seed: u32) -> Self {
        let i = &ps["InitialModule"];
        let em = &ps["EmissionModule"];
        let sh = &ps["ShapeModule"];
        let on = |m: &str| bv(&ps[m], "enabled");
        let bursts = if bv(em, "enabled") {
            em["m_Bursts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|b| Burst {
                            time: fv(b, "time"),
                            count: Curve::parse(&b["countCurve"]),
                            cycles: b["cycleCount"].as_u64().unwrap_or(1) as u32,
                            interval: fv(b, "repeatInterval"),
                            probability: b.get("probability").map_or(1.0, f),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let render_mode = match r["m_RenderMode"].as_i64().unwrap_or(0) {
            1 => RenderMode::Stretch,
            4 => RenderMode::Mesh,
            _ => RenderMode::Billboard,
        };
        SystemDef {
            duration: fv(ps, "lengthInSec"),
            looping: bv(ps, "looping"),
            prewarm: bv(ps, "prewarm"),
            play_on_awake: bv(ps, "playOnAwake"),
            start_delay: Curve::parse(&ps["startDelay"]),
            sim_speed: ps.get("simulationSpeed").map_or(1.0, f),
            lifetime: Curve::parse(&i["startLifetime"]),
            speed: Curve::parse(&i["startSpeed"]),
            size: Curve::parse(&i["startSize"]),
            size3d: bv(i, "size3D").then(|| {
                (
                    Curve::parse(&i["startSizeY"]),
                    Curve::parse(&i["startSizeZ"]),
                )
            }),
            rotation: Curve::parse(&i["startRotation"]),
            rotation_xy: bv(i, "rotation3D").then(|| {
                [
                    Curve::parse(&i["startRotationX"]),
                    Curve::parse(&i["startRotationY"]),
                ]
            }),
            color: ColorSpec::parse(&i["startColor"]),
            gravity: Curve::parse(&i["gravityModifier"]),
            max_particles: i["maxNumParticles"].as_u64().unwrap_or(1000) as usize,
            rate: if bv(em, "enabled") {
                Curve::parse(&em["rateOverTime"])
            } else {
                Curve::Const(0.0)
            },
            bursts,
            shape: Shape {
                enabled: bv(sh, "enabled"),
                kind: sh["type"].as_i64().unwrap_or(0),
                radius: fv(&sh["radius"], "value"),
                radius_thickness: sh.get("radiusThickness").map_or(1.0, f),
                angle: fv(sh, "angle"),
                arc: sh.get("arc").map_or(360.0, |a| fv(a, "value")),
                box_thickness: v3(&sh["boxThickness"]),
                position: v3(&sh["m_Position"]),
                rotation: v3(&sh["m_Rotation"]),
                scale: v3(&sh["m_Scale"]),
                random_direction: fv(sh, "randomDirectionAmount"),
                spherical_direction: fv(sh, "sphericalDirectionAmount"),
            },
            velocity: on("VelocityModule").then(|| {
                let v = &ps["VelocityModule"];
                [
                    Curve::parse(&v["x"]),
                    Curve::parse(&v["y"]),
                    Curve::parse(&v["z"]),
                    Curve::parse(&v["speedModifier"]),
                ]
            }),
            clamp: on("ClampVelocityModule").then(|| {
                let c = &ps["ClampVelocityModule"];
                (Curve::parse(&c["magnitude"]), fv(c, "dampen"))
            }),
            size_ol: on("SizeModule").then(|| {
                let s = &ps["SizeModule"];
                let y = bv(s, "separateAxes").then(|| Curve::parse(&s["y"]));
                (Curve::parse(&s["curve"]), y)
            }),
            color_ol: on("ColorModule").then(|| ColorSpec::parse(&ps["ColorModule"]["gradient"])),
            rotation_ol: on("RotationModule").then(|| Curve::parse(&ps["RotationModule"]["curve"])),
            rotation_ol_xy: (on("RotationModule") && bv(&ps["RotationModule"], "separateAxes"))
                .then(|| {
                    [
                        Curve::parse(&ps["RotationModule"]["x"]),
                        Curve::parse(&ps["RotationModule"]["y"]),
                    ]
                }),
            mesh: None,
            sub_birth: if bv(&ps["SubModule"], "enabled") {
                ps["SubModule"]["subEmitters"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["type"].as_i64().unwrap_or(0) == 0)
                    .filter_map(|e| e["emitter"]["m_PathID"].as_i64().filter(|&id| id != 0))
                    .collect()
            } else {
                Vec::new()
            },
            noise: on("NoiseModule").then(|| {
                let n = &ps["NoiseModule"];
                (
                    Curve::parse(&n["strength"]),
                    fv(n, "frequency").max(1e-4),
                    bv(n, "damping"),
                )
            }),
            sheet: on("UVModule").then(|| {
                let u = &ps["UVModule"];
                let row = (u["animationType"].as_i64() == Some(1))
                    .then(|| u["rowIndex"].as_u64().unwrap_or(0) as u32);
                (
                    u["tilesX"].as_u64().unwrap_or(1).max(1) as u32,
                    u["tilesY"].as_u64().unwrap_or(1).max(1) as u32,
                    Curve::parse(&u["frameOverTime"]),
                    Curve::parse(&u["startFrame"]),
                    u.get("cycles").map_or(1.0, f),
                    row,
                )
            }),
            render_mode,
            length_scale: r.get("m_LengthScale").map_or(1.0, f),
            velocity_scale: r.get("m_VelocityScale").map_or(0.0, f),
            max_particle_size: r.get("m_MaxParticleSize").map_or(0.5, f),
            sorting_order: r["m_SortingOrder"].as_i64().unwrap_or(0) as i32,
            custom1x: {
                let cd = &ps["CustomDataModule"];
                let streamed = r["m_VertexStreams"]
                    .as_array()
                    .is_some_and(|a| a.iter().any(|v| v.as_i64() == Some(CUSTOM1X_STREAM)));
                (streamed
                    && bv(cd, "enabled")
                    && cd["mode0"].as_i64() == Some(1)
                    && cd["vectorComponentCount0"].as_i64().unwrap_or(0) >= 1)
                    .then(|| Curve::parse(&cd["vector0_0"]))
            },
            seed,
        }
    }
}

// ------------------------------------------------------------------ runtime

/// A particle quad: corners (local frame, world units), UV rect (v up), colour.
/// Corners (a degenerate fourth corner for a mesh triangle), per-corner UVs (v up, in the
/// texture sheet), colour, and the particle's `Custom1.x` vertex stream (0 without one).
pub type Quad = ([[f32; 2]; 4], [[f32; 2]; 4], [f32; 4], f32);

/// Particle mesh geometry (`ParticleSystemRenderer.m_Mesh` in Mesh render mode).
#[derive(Debug, Clone)]
pub struct Mesh {
    pub verts: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub tris: Vec<[u32; 3]>,
}

impl Mesh {
    /// Unity's built-in Quad (`unity default resources` 10210): 1×1 in XY, facing −z.
    pub fn quad() -> Self {
        Mesh {
            verts: vec![
                [-0.5, -0.5, 0.0],
                [0.5, -0.5, 0.0],
                [0.5, 0.5, 0.0],
                [-0.5, 0.5, 0.0],
            ],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            tris: vec![[0, 1, 2], [0, 2, 3]],
        }
    }

    /// Unity's built-in Cube (10202): the unit cube, each face UV-mapped 0–1.
    pub fn cube() -> Self {
        let mut m = Mesh {
            verts: Vec::new(),
            uvs: Vec::new(),
            tris: Vec::new(),
        };
        // (normal axis, sign)
        for (axis, sign) in [
            (0, 1.0),
            (0, -1.0),
            (1, 1.0),
            (1, -1.0),
            (2, 1.0),
            (2, -1.0),
        ] {
            let base = m.verts.len() as u32;
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for (a, b) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
                let mut p = [0.0f32; 3];
                p[axis] = 0.5 * sign;
                p[u] = a;
                p[v] = b;
                m.verts.push(p);
                m.uvs.push([a + 0.5, b + 0.5]);
            }
            m.tris.push([base, base + 1, base + 2]);
            m.tris.push([base, base + 2, base + 3]);
        }
        m
    }

    /// A serialized Unity `Mesh` (uncompressed, float32 / float16 positions and UV0 in stream 0,
    /// 16- or 32-bit indices). `None` for layouts this reader does not handle.
    pub fn from_unity(m: &Value) -> Option<Self> {
        if m["m_MeshCompression"].as_i64().unwrap_or(0) != 0 {
            return None;
        }
        let vd = &m["m_VertexData"];
        let count = vd["m_VertexCount"].as_u64()? as usize;
        let data: Vec<u8> = vd["m_DataSize"]
            .as_array()?
            .iter()
            .map(|b| b.as_u64().unwrap_or(0) as u8)
            .collect();
        let chans: Vec<(u64, u64, u64, u64)> = vd["m_Channels"]
            .as_array()?
            .iter()
            .map(|c| {
                (
                    c["stream"].as_u64().unwrap_or(0),
                    c["offset"].as_u64().unwrap_or(0),
                    c["format"].as_u64().unwrap_or(0),
                    c["dimension"].as_u64().unwrap_or(0) & 0x0f,
                )
            })
            .collect();
        let size = |fmt: u64| match fmt {
            0 => 4,
            1 => 2,
            _ => 0,
        };
        if chans
            .iter()
            .any(|c| c.3 > 0 && (c.0 != 0 || size(c.2) == 0))
        {
            return None;
        }
        let stride = chans
            .iter()
            .filter(|c| c.3 > 0)
            .map(|c| c.1 + size(c.2) * c.3)
            .max()? as usize;
        let read = |i: usize, ch: usize, k: usize| -> f32 {
            let c = chans[ch];
            let o = i * stride + c.1 as usize + k * size(c.2) as usize;
            match c.2 {
                0 => f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]),
                _ => half_to_f32(u16::from_le_bytes([data[o], data[o + 1]])),
            }
        };
        if data.len() < stride * count || chans.first()?.3 < 3 {
            return None;
        }
        let verts = (0..count)
            .map(|i| [read(i, 0, 0), read(i, 0, 1), read(i, 0, 2)])
            .collect();
        let uvs = if chans.get(4).is_some_and(|c| c.3 >= 2) {
            (0..count).map(|i| [read(i, 4, 0), read(i, 4, 1)]).collect()
        } else {
            vec![[0.0, 0.0]; count]
        };
        let ib: Vec<u8> = m["m_IndexBuffer"]
            .as_array()?
            .iter()
            .map(|b| b.as_u64().unwrap_or(0) as u8)
            .collect();
        let idx: Vec<u32> = if m["m_IndexFormat"].as_i64().unwrap_or(0) == 1 {
            ib.as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect()
        } else {
            ib.as_chunks::<2>()
                .0
                .iter()
                .map(|c| u32::from(u16::from_le_bytes(*c)))
                .collect()
        };
        let tris = idx
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|t| t.iter().all(|&v| (v as usize) < count))
            .copied()
            .collect();
        Some(Mesh { verts, uvs, tris })
    }
}

fn half_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = i32::from((h >> 10) & 0x1f);
    let m = f32::from(h & 0x3ff);
    match e {
        0 => s * m * (1.0 / 16_777_216.0),
        31 => s * f32::INFINITY,
        _ => s * (1.0 + m / 1024.0) * f32::from_bits(((e - 15 + 127) as u32) << 23),
    }
}

#[derive(Debug, Clone)]
struct Particle {
    pos: [f32; 3],
    vel: [f32; 3],
    /// Total velocity of the last step (base + lifetime modules): stretched billboards follow it.
    cur_vel: [f32; 3],
    age: f32,
    life: f32,
    /// x, y, z (z = x without 3D size)
    size: [f32; 3],
    rot: f32,
    rot_xy: [f32; 2],
    color: [f32; 4],
    /// Fixed per-particle randoms for curves in "random between" modes.
    rnd: [f32; 8],
    seed: u32,
}

#[derive(Debug, Clone)]
pub struct SystemState {
    particles: Vec<Particle>,
    rng: u32,
    /// Time since `Play` (includes the start delay), `None` while not playing.
    time: Option<f32>,
    delay: f32,
    emitting: bool,
    emit_acc: f32,
    /// Bursts fired in the current loop: (loop index, burst, cycle).
    fired: Vec<(u32, usize, u32)>,
    /// This system is another's Birth sub-emitter: it emits only from `subs`.
    pub sub_only: bool,
    /// Birth sub-emitter sources: one per live parent particle.
    subs: Vec<SubSource>,
}

/// A Birth sub-emitter instance riding on a parent particle (`SubModule`, type Birth): the
/// system's emission (rate and bursts on its own clock) from the parent's position, until the
/// parent dies.
#[derive(Debug, Clone)]
struct SubSource {
    parent: u32,
    origin: [f32; 3],
    time: f32,
    emit_acc: f32,
    fired: Vec<(u32, usize, u32)>,
    alive: bool,
}

fn next_u32(s: &mut u32) -> u32 {
    let mut x = *s;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

fn unit(s: &mut u32) -> f32 {
    (next_u32(s) >> 8) as f32 / (1u32 << 24) as f32
}

/// `Quaternion.Euler(x, y, z)` = `Ry·Rx·Rz`, degrees.
fn rot_euler(v: [f32; 3], deg: [f32; 3]) -> [f32; 3] {
    let (rx, ry, rz) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    let (sx, cx) = (sinf(rx), cosf(rx));
    let (sy, cy) = (sinf(ry), cosf(ry));
    let (sz, cz) = (sinf(rz), cosf(rz));
    let m = [
        [cy * cz + sy * sx * sz, -cy * sz + sy * sx * cz, sy * cx],
        [cx * sz, cx * cz, -sx],
        [-sy * cz + cy * sx * sz, sy * sz + cy * sx * cz, cy * cx],
    ];
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l < 1e-6 {
        [0.0, 0.0, 1.0]
    } else {
        [v[0] / l, v[1] / l, v[2] / l]
    }
}

impl SystemState {
    pub fn new(def: &SystemDef, seed: u32) -> Self {
        SystemState {
            particles: Vec::new(),
            rng: (def.seed ^ seed.wrapping_mul(2654435761)) | 1,
            time: None,
            delay: 0.0,
            emitting: false,
            emit_acc: 0.0,
            fired: Vec::new(),
            sub_only: false,
            subs: Vec::new(),
        }
    }

    /// (seed, position) of every live particle, for sub-emitters.
    pub fn particle_origins(&self) -> Vec<(u32, [f32; 3])> {
        self.particles.iter().map(|p| (p.seed, p.pos)).collect()
    }

    /// Keeps one Birth sub-emitter source per live parent particle (new parents start one,
    /// dead ones stop emitting) and moves them with their parents.
    pub fn sync_subs(&mut self, def: &SystemDef, parents: &[(u32, [f32; 3])]) {
        for s in &mut self.subs {
            match parents.iter().find(|(seed, _)| *seed == s.parent) {
                Some((_, pos)) => s.origin = *pos,
                None => s.alive = false,
            }
        }
        self.subs.retain(|s| s.alive);
        for &(seed, pos) in parents {
            if !self.subs.iter().any(|s| s.parent == seed) {
                // the sub system's own start delay counts from the parent's birth
                let delay = def.start_delay.eval(unit(&mut self.rng), 0.0);
                self.subs.push(SubSource {
                    parent: seed,
                    origin: pos,
                    time: -delay,
                    emit_acc: 0.0,
                    fired: Vec::new(),
                    alive: true,
                });
            }
        }
    }

    /// `ParticleSystem.Play`; with `prewarm` a looping system starts as if one loop had run.
    pub fn play(&mut self, def: &SystemDef, dt: f32) {
        self.particles.clear();
        self.fired.clear();
        self.emit_acc = 0.0;
        self.emitting = true;
        self.delay = def.start_delay.eval(unit(&mut self.rng), 0.0);
        self.time = Some(0.0);
        if def.prewarm && def.looping && def.duration > 0.0 {
            self.delay = 0.0;
            let n = (def.duration / dt).ceil() as u32;
            for _ in 0..n {
                self.step(def, dt);
            }
        }
    }

    /// `ParticleSystem.Stop()` (StopEmitting): live particles run their course.
    pub fn stop_emitting(&mut self) {
        self.emitting = false;
    }

    /// Deactivated GameObject: nothing left.
    pub fn clear(&mut self) {
        self.particles.clear();
        self.time = None;
        self.emitting = false;
    }

    pub fn is_alive(&self, def: &SystemDef) -> bool {
        if !self.particles.is_empty() || !self.subs.is_empty() {
            return true;
        }
        match self.time {
            Some(t) => self.emitting && (def.looping || t < self.delay + def.duration),
            None => false,
        }
    }

    pub fn step(&mut self, def: &SystemDef, dt: f32) {
        if self.sub_only {
            // driven by the parent: particles age, sources emit on their own clocks
            let dt = dt * def.sim_speed;
            self.update(def, dt);
            let mut subs = std::mem::take(&mut self.subs);
            for s in &mut subs {
                s.time += dt;
                if s.time < 0.0 {
                    continue;
                }
                let t1 = s.time;
                let dur = def.duration.max(1e-4);
                let (loop_i, in_loop) = if def.looping {
                    ((t1 / dur) as u32, t1 % dur)
                } else if t1 > dur {
                    continue;
                } else {
                    (0, t1)
                };
                let sys_t = in_loop / dur;
                s.emit_acc += def.rate.eval(unit(&mut self.rng), sys_t) * dt;
                while s.emit_acc >= 1.0 {
                    s.emit_acc -= 1.0;
                    self.spawn_at(def, sys_t, s.origin);
                }
                for (bi, b) in def.bursts.iter().enumerate() {
                    for c in 0..b.cycles.max(1) {
                        let at = b.time + b.interval * c as f32;
                        if in_loop + 1e-6 < at || s.fired.contains(&(loop_i, bi, c)) {
                            continue;
                        }
                        s.fired.push((loop_i, bi, c));
                        if unit(&mut self.rng) > b.probability {
                            continue;
                        }
                        let n = b.count.eval(unit(&mut self.rng), 0.0).round().max(0.0) as u32;
                        for _ in 0..n {
                            self.spawn_at(def, sys_t, s.origin);
                        }
                    }
                }
                s.fired.retain(|&(l, _, _)| l + 1 >= loop_i);
            }
            self.subs = subs;
            return;
        }
        let Some(time) = self.time else { return };
        let dt = dt * def.sim_speed;
        let t1 = time + dt;
        self.time = Some(t1);
        self.update(def, dt);
        if !self.emitting || t1 < self.delay {
            return;
        }
        let dur = def.duration.max(1e-4);
        let local = t1 - self.delay;
        let (loop_i, in_loop) = if def.looping {
            ((local / dur) as u32, local % dur)
        } else if local > dur {
            self.emitting = false;
            return;
        } else {
            (0, local)
        };
        // rate over time, sampled at the loop's normalized time
        let rate = def.rate.eval(unit(&mut self.rng), in_loop / dur);
        self.emit_acc += rate * dt;
        // the main module's curves and gradients are sampled at the system's normalized time
        let sys_t = in_loop / dur;
        while self.emit_acc >= 1.0 {
            self.emit_acc -= 1.0;
            self.spawn(def, sys_t);
        }
        for (bi, b) in def.bursts.iter().enumerate() {
            for c in 0..b.cycles.max(1) {
                let at = b.time + b.interval * c as f32;
                if in_loop + 1e-6 < at || self.fired.contains(&(loop_i, bi, c)) {
                    continue;
                }
                self.fired.push((loop_i, bi, c));
                if unit(&mut self.rng) > b.probability {
                    continue;
                }
                let n = b.count.eval(unit(&mut self.rng), 0.0).round().max(0.0) as u32;
                for _ in 0..n {
                    self.spawn(def, sys_t);
                }
            }
        }
        self.fired.retain(|&(l, _, _)| l + 1 >= loop_i);
    }

    fn spawn(&mut self, def: &SystemDef, sys_t: f32) {
        self.spawn_at(def, sys_t, [0.0; 3]);
    }

    fn spawn_at(&mut self, def: &SystemDef, sys_t: f32, origin: [f32; 3]) {
        if self.particles.len() >= def.max_particles {
            return;
        }
        let r = &mut self.rng;
        let mut rnd = [0.0; 8];
        for x in &mut rnd {
            *x = unit(r);
        }
        let life = def.lifetime.eval(unit(r), sys_t).max(1e-3);
        let speed = def.speed.eval(unit(r), sys_t);
        let sx = def.size.eval(unit(r), sys_t);
        let sy = def
            .size3d
            .as_ref()
            .map_or(sx, |(y, _)| y.eval(unit(r), sys_t));
        let (pos, dir) = if def.shape.enabled {
            shape_sample(&def.shape, r)
        } else {
            ([0.0; 3], [0.0, 0.0, 1.0])
        };
        let pos = [pos[0] + origin[0], pos[1] + origin[1], pos[2] + origin[2]];
        let vel = [dir[0] * speed, dir[1] * speed, dir[2] * speed];
        let color = def.color.eval(unit(r), sys_t);
        let sz = def
            .size3d
            .as_ref()
            .map_or(sx, |(_, z)| z.eval(unit(r), sys_t));
        let rot = def.rotation.eval(unit(r), sys_t);
        let rot_xy = def.rotation_xy.as_ref().map_or([0.0; 2], |[x, y]| {
            [x.eval(unit(r), sys_t), y.eval(unit(r), sys_t)]
        });
        let seed = next_u32(r);
        self.particles.push(Particle {
            pos,
            vel,
            cur_vel: vel,
            age: 0.0,
            life,
            size: [sx, sy, sz],
            rot,
            rot_xy,
            color,
            rnd,
            seed,
        });
    }

    #[allow(clippy::needless_range_loop)] // parallel xyz arrays
    fn update(&mut self, def: &SystemDef, dt: f32) {
        let g = -9.81;
        self.particles.retain_mut(|p| {
            p.age += dt;
            if p.age >= p.life {
                return false;
            }
            let t = p.age / p.life;
            p.vel[1] += g * def.gravity.eval(p.rnd[0], t) * dt;
            let mut v = p.vel;
            if let Some([x, y, z, m]) = &def.velocity {
                let k = m.eval(p.rnd[1], t);
                v[0] += x.eval(p.rnd[2], t) * k;
                v[1] += y.eval(p.rnd[3], t) * k;
                v[2] += z.eval(p.rnd[4], t) * k;
            }
            if let Some((strength, freq, damping)) = &def.noise {
                let s = strength.eval(p.rnd[5], t);
                if s != 0.0 {
                    let n = crate::fx::noise3(p.pos, *freq, p.seed);
                    let mut k = s / freq;
                    if *damping {
                        k *= 1.0 - t;
                    }
                    v[0] += n.0 * k;
                    v[1] += n.1 * k;
                    v[2] += n.2 * k;
                }
            }
            // Limit Velocity acts on the total velocity (base + velocity over lifetime); the
            // excess is taken out of the base velocity. Dampen is the fraction removed per
            // 1/30 s step (the Unity runtime's own step is not reversed: approximation).
            if let Some((limit, dampen)) = &def.clamp {
                let lim = limit.eval(p.rnd[6], t).max(1e-5);
                let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if mag > lim {
                    let per_step = 1.0 - (1.0 - lim / mag) * dampen.clamp(0.0, 1.0);
                    let f = sse_core::det_math::powf(per_step.max(0.0), dt * 30.0);
                    for i in 0..3 {
                        let limited = v[i] * f;
                        p.vel[i] += limited - v[i];
                        v[i] = limited;
                    }
                }
            }
            for i in 0..3 {
                p.pos[i] += v[i] * dt;
            }
            p.cur_vel = v;
            if let Some(w) = &def.rotation_ol {
                p.rot += w.eval(p.rnd[7], t) * dt;
            }
            if let Some([x, y]) = &def.rotation_ol_xy {
                p.rot_xy[0] += x.eval(p.rnd[7], t) * dt;
                p.rot_xy[1] += y.eval(p.rnd[7], t) * dt;
            }
            true
        });
    }

    /// Quads in the emitter's local frame (world units): corners (−,−) (+,−) (+,+) (−,+),
    /// UV rect (u0, v0, u1, v1 with v up) and colour. `stretch` directions are in local space.
    /// Quads in the emitter's local frame (world units), with the local simulation space turned by `rot` (row-major 3×3)
    /// before the orthographic projection: positions and stretch directions follow the
    /// emitter's rotation, billboard corners keep facing the camera.
    pub fn quads_rotated(&self, def: &SystemDef, rot: Option<[[f32; 3]; 3]>) -> Vec<Quad> {
        let turn = |v: [f32; 3]| match rot {
            Some(m) => [
                m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
                m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
                m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
            ],
            None => v,
        };
        let mut out = Vec::with_capacity(self.particles.len());
        for p0 in &self.particles {
            let mut moved = p0.clone();
            moved.pos = turn(p0.pos);
            moved.cur_vel = turn(p0.cur_vel);
            let p = &moved;
            let t = (p.age / p.life).clamp(0.0, 1.0);
            let mut c = p.color;
            if let Some(ol) = &def.color_ol {
                let k = ol.eval(p.rnd[0], t);
                for i in 0..4 {
                    c[i] *= k[i];
                }
            }
            if c[3] <= 0.002 {
                continue;
            }
            let (mut w, mut h, mut d) = (p.size[0], p.size[1], p.size[2]);
            if let Some((x, y)) = &def.size_ol {
                let kx = x.eval(p.rnd[1], t);
                let ky = y.as_ref().map_or(kx, |y| y.eval(p.rnd[1], t));
                w *= kx;
                h *= ky;
                d *= kx;
            }
            match def.render_mode {
                // `maxParticleSize` is a fraction of the viewport height (2 units)
                RenderMode::Billboard => {
                    let cap = def.max_particle_size * 2.0;
                    w = w.min(cap);
                    h = h.min(cap);
                }
                RenderMode::Stretch => w = w.min(def.max_particle_size * 2.0),
                RenderMode::Mesh => {}
            }
            let uv = sheet_uv(def, p, t);
            let cell = |u: f32, v: f32| [uv[0] + (uv[2] - uv[0]) * u, uv[1] + (uv[3] - uv[1]) * v];
            let custom = def.custom1x.as_ref().map_or(0.0, |k| k.eval(p.rnd[4], t));
            // 3D: billboards with a 3D rotation and every mesh particle. The vertices are scaled
            // by the particle size, rotated (Unity `Quaternion.Euler` order, the same sign as the
            // 2D billboard rotation) and projected by the orthographic scenario camera.
            let three_d = def.render_mode == RenderMode::Mesh
                || (def.render_mode == RenderMode::Billboard && p.rot_xy != [0.0; 2]);
            if three_d {
                let quad = Mesh::quad();
                let (mesh, scale) = match def.render_mode {
                    RenderMode::Mesh => (def.mesh.as_deref().unwrap_or(&quad), [w, h, d]),
                    _ => (&quad, [w, h, 1.0]),
                };
                let deg = [
                    -p.rot_xy[0].to_degrees(),
                    -p.rot_xy[1].to_degrees(),
                    -p.rot.to_degrees(),
                ];
                let proj: Vec<[f32; 2]> = mesh
                    .verts
                    .iter()
                    .map(|v| {
                        let r = rot_euler([v[0] * scale[0], v[1] * scale[1], v[2] * scale[2]], deg);
                        [p.pos[0] + r[0], p.pos[1] + r[1]]
                    })
                    .collect();
                let uvs: Vec<[f32; 2]> = mesh.uvs.iter().map(|t| cell(t[0], t[1])).collect();
                if mesh.tris == [[0, 1, 2], [0, 2, 3]] && proj.len() == 4 {
                    out.push((
                        [proj[0], proj[1], proj[2], proj[3]],
                        [uvs[0], uvs[1], uvs[2], uvs[3]],
                        c,
                        custom,
                    ));
                } else {
                    for &[a, b, e] in &mesh.tris {
                        let (a, b, e) = (a as usize, b as usize, e as usize);
                        out.push((
                            [proj[a], proj[b], proj[e], proj[e]],
                            [uvs[a], uvs[b], uvs[e], uvs[e]],
                            c,
                            custom,
                        ));
                    }
                }
                continue;
            }
            let corners = match def.render_mode {
                RenderMode::Stretch => {
                    // width = size.x; length = size.y × lengthScale + speed × velocityScale,
                    // along the particle's current velocity, the head at the particle
                    let d = [p.cur_vel[0], p.cur_vel[1]];
                    let sp = (d[0] * d[0] + d[1] * d[1]).sqrt();
                    let (ux, uy) = if sp < 1e-6 {
                        (0.0, 1.0)
                    } else {
                        (d[0] / sp, d[1] / sp)
                    };
                    let len = h * def.length_scale + sp * def.velocity_scale;
                    let (hl, hw) = (len * 0.5, w * 0.5);
                    // the texture's v runs along the velocity, head at the particle
                    let (ax, ay) = (ux * hl, uy * hl);
                    let (bx, by) = (-uy * hw, ux * hw);
                    let (cx, cy) = (p.pos[0] - ax, p.pos[1] - ay);
                    [
                        [cx - ax - bx, cy - ay - by],
                        [cx - ax + bx, cy - ay + by],
                        [cx + ax + bx, cy + ay + by],
                        [cx + ax - bx, cy + ay - by],
                    ]
                }
                _ => {
                    let (s, co) = (sinf(-p.rot), cosf(-p.rot));
                    let (hw, hh) = (w * 0.5, h * 0.5);
                    let q = |x: f32, y: f32| [p.pos[0] + x * co - y * s, p.pos[1] + x * s + y * co];
                    [q(-hw, -hh), q(hw, -hh), q(hw, hh), q(-hw, hh)]
                }
            };
            out.push((
                corners,
                [
                    cell(0.0, 0.0),
                    cell(1.0, 0.0),
                    cell(1.0, 1.0),
                    cell(0.0, 1.0),
                ],
                c,
                custom,
            ));
        }
        out
    }
}

fn sheet_uv(def: &SystemDef, p: &Particle, t: f32) -> [f32; 4] {
    let Some((tx, ty, frame, start, cycles, row)) = &def.sheet else {
        return [0.0, 0.0, 1.0, 1.0];
    };
    let per_row = *tx;
    let frames = if row.is_some() { *tx } else { tx * ty };
    let ft = ((t * cycles) % 1.0
        + if (t * cycles) >= 1.0 && (t * cycles) % 1.0 == 0.0 {
            1.0
        } else {
            0.0
        })
    .min(1.0);
    let k = (frame.eval(p.rnd[2], ft) + start.eval(p.rnd[3], 0.0)) * frames as f32;
    let idx = (k.floor().max(0.0) as u32).min(frames - 1);
    let (col, r) = match row {
        Some(r) => (idx, *r),
        None => (idx % per_row, idx / per_row),
    };
    let (w, h) = (1.0 / *tx as f32, 1.0 / *ty as f32);
    // tile 0 is the top-left; UVs have v up
    [
        col as f32 * w,
        1.0 - (r + 1) as f32 * h,
        (col + 1) as f32 * w,
        1.0 - r as f32 * h,
    ]
}

/// A point and direction on the emission shape (before the emitter transform).
#[allow(clippy::needless_range_loop)] // parallel xyz arrays
fn shape_sample(s: &Shape, r: &mut u32) -> ([f32; 3], [f32; 3]) {
    let tau = std::f32::consts::TAU;
    let shell = |r: &mut u32, thick: f32| 1.0 - thick * unit(r);
    let (mut pos, mut dir) = match s.kind {
        // sphere / hemisphere (and their shells)
        0..=3 => {
            let z = unit(r) * 2.0 - 1.0;
            let a = unit(r) * tau;
            let rr = (1.0 - z * z).max(0.0).sqrt();
            let mut d = [rr * cosf(a), rr * sinf(a), z];
            if s.kind >= 2 {
                d[2] = d[2].abs();
            }
            let rad = if s.kind == 1 || s.kind == 3 {
                s.radius
            } else {
                s.radius * shell(r, s.radius_thickness).cbrt()
            };
            ([d[0] * rad, d[1] * rad, d[2] * rad], d)
        }
        // cone variants: base disc, direction opening by `angle`
        4 | 7..=9 => {
            let a = unit(r) * s.arc.to_radians();
            let rr = s.radius * shell(r, s.radius_thickness).sqrt();
            let (ca, sa) = (cosf(a), sinf(a));
            let open = s.angle.to_radians();
            let tilt = if s.radius > 1e-6 {
                rr / s.radius * open
            } else {
                unit(r) * open
            };
            (
                [ca * rr, sa * rr, 0.0],
                norm([ca * sinf(tilt), sa * sinf(tilt), cosf(tilt)]),
            )
        }
        // box (volume / shell / edge): emits along +z
        5 | 15 | 16 => {
            let p = [unit(r) - 0.5, unit(r) - 0.5, unit(r) - 0.5];
            (p, [0.0, 0.0, 1.0])
        }
        // circle / circle edge: in the XY plane, outward
        10 | 11 => {
            let a = unit(r) * s.arc.to_radians();
            let rr = if s.kind == 11 {
                s.radius
            } else {
                s.radius * shell(r, s.radius_thickness).sqrt()
            };
            let (ca, sa) = (cosf(a), sinf(a));
            ([ca * rr, sa * rr, 0.0], [ca, sa, 0.0])
        }
        // single-sided edge: a line along x, emitting along +y
        12 => (
            [(unit(r) * 2.0 - 1.0) * s.radius, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ),
        // rectangle: the XY unit square, emitting along +z
        18 => ([unit(r) - 0.5, unit(r) - 0.5, 0.0], [0.0, 0.0, 1.0]),
        _ => ([0.0; 3], [0.0, 0.0, 1.0]),
    };
    let _ = s.box_thickness;
    if s.random_direction > 0.0 {
        let z = unit(r) * 2.0 - 1.0;
        let a = unit(r) * tau;
        let rr = (1.0 - z * z).max(0.0).sqrt();
        let rd = [rr * cosf(a), rr * sinf(a), z];
        for i in 0..3 {
            dir[i] += (rd[i] - dir[i]) * s.random_direction;
        }
        dir = norm(dir);
    }
    if s.spherical_direction > 0.0 {
        let sd = norm(pos);
        for i in 0..3 {
            dir[i] += (sd[i] - dir[i]) * s.spherical_direction;
        }
        dir = norm(dir);
    }
    for i in 0..3 {
        pos[i] *= s.scale[i];
    }
    let pos = rot_euler(pos, s.rotation);
    let dir = rot_euler(dir, s.rotation);
    (
        [
            pos[0] + s.position[0],
            pos[1] + s.position[1],
            pos[2] + s.position[2],
        ],
        dir,
    )
}

impl SystemDef {
    #[cfg(test)]
    fn test(json: &str) -> Self {
        let v: Value = serde_json::from_str(json).unwrap();
        SystemDef::parse(&v["ps"], &v["r"], 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system(extra: &str) -> SystemDef {
        SystemDef::test(&format!(
            r#"{{"ps": {{"lengthInSec": 1.0, "looping": false, "prewarm": false, "playOnAwake": true,
              "startDelay": {{"minMaxState": 0, "scalar": 0.0}},
              "InitialModule": {{"startLifetime": {{"minMaxState": 0, "scalar": 2.0}},
                "startSpeed": {{"minMaxState": 0, "scalar": 1.0}}, "startSize": {{"minMaxState": 0, "scalar": 0.5}},
                "startRotation": {{"minMaxState": 0, "scalar": 0.0}}, "gravityModifier": {{"minMaxState": 0, "scalar": 0.0}},
                "startColor": {{"minMaxState": 0, "maxColor": {{"r": 1, "g": 1, "b": 1, "a": 1}}}}, "maxNumParticles": 1000}},
              "EmissionModule": {{"enabled": true, "rateOverTime": {{"minMaxState": 0, "scalar": 10.0}}, "m_Bursts": [
                {{"time": 0.0, "countCurve": {{"minMaxState": 0, "scalar": 5.0}}, "cycleCount": 1, "repeatInterval": 0.01}}]}},
              "ShapeModule": {{"enabled": false}} {extra}}},
             "r": {{"m_RenderMode": 0, "m_SortingOrder": 230}}}}"#
        ))
    }

    #[test]
    fn bursts_and_rate_emit_and_stop_after_the_duration() {
        let def = system("");
        let mut st = SystemState::new(&def, 7);
        st.play(&def, 1.0 / 60.0);
        for _ in 0..60 {
            st.step(&def, 1.0 / 60.0);
        }
        // 5 burst + ~10 from the rate over one second
        let n = st.particles.len();
        assert!((14..=16).contains(&n), "{n}");
        for _ in 0..60 {
            st.step(&def, 1.0 / 60.0);
        }
        assert!(!st.emitting);
        // speed 1 along +z for ~1-2 s
        assert!(st.particles.iter().all(|p| p.pos[2] > 0.9));
    }

    #[test]
    fn random_color_samples_the_gradient_at_the_particles_random() {
        // `hologram` prefab triangles: fixed-mode keys cyan / yellow / magenta
        let v: Value = serde_json::from_str(
            r#"{"minMaxState": 4, "maxGradient": {"m_Mode": 1, "m_NumColorKeys": 3, "m_NumAlphaKeys": 1,
                "ctime0": 21588, "ctime1": 42791, "ctime2": 65535, "atime0": 0,
                "key0": {"r": 0, "g": 1, "b": 1, "a": 1}, "key1": {"r": 1, "g": 1, "b": 0, "a": 1},
                "key2": {"r": 1, "g": 0, "b": 1, "a": 1}}}"#,
        )
        .unwrap();
        let c = ColorSpec::parse(&v);
        assert_eq!(c.eval(0.1, 0.0)[..3], [0.0, 1.0, 1.0]);
        assert_eq!(c.eval(0.5, 0.0)[..3], [1.0, 1.0, 0.0]);
        assert_eq!(c.eval(0.9, 0.0)[..3], [1.0, 0.0, 1.0]);
    }

    #[test]
    fn gradients_interpolate_colour_and_alpha_separately() {
        let g: Value = serde_json::from_str(
            r#"{"m_NumColorKeys": 2, "m_NumAlphaKeys": 2, "ctime0": 0, "ctime1": 65535, "atime0": 0, "atime1": 32767,
                "key0": {"r": 0, "g": 0, "b": 0, "a": 0}, "key1": {"r": 1, "g": 1, "b": 1, "a": 1}}"#,
        )
        .unwrap();
        let g = Gradient::parse(&g);
        let c = g.at(0.25);
        assert!(
            (c[0] - 0.25).abs() < 1e-3 && (c[3] - 0.5).abs() < 2e-3,
            "{c:?}"
        );
    }
}
