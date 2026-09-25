//! `fx_transition_scenario`: the Sekai transition triangles (`ScenarioPlayer.effectLayer`).
//!
//! The prefab ships inside the app, so its parameters come from the `--ui` kit
//! (`fx_transition_scenario.json`, `sse-fx` v1, written by `tools/ui-kit/extract.py`); this
//! module is a CPU re-implementation of the Unity ParticleSystem modules the prefab actually enables
//! (initial, shape, emission/bursts, rotation, colour, velocity over lifetime, UV tile,
//! noise, clamp velocity). `ScalingMode.Local` and the local simulation space mean the whole
//! effect lives in the UI camera's orthographic world: the screen is 2 world units tall
//! (`orthographicSize` 1) and particles are unaware of the canvas scale.
//!
//! ## Approximations (reported on export)
//! - The noise field is value noise from our own hash, not Unity's gradient/simplex noise;
//!   the module's `strength` / `frequency` / `damping` semantics are kept.
//! - Per-particle random streams cannot match Unity's PRNG, so the layout differs particle
//!   for particle. The distributions, counts and timings do match.
//! - Rotation over lifetime is applied as a full Euler rotation of the billboard (so 3D
//!   rotation foreshortens it) rather than Unity's per-vertex rotation caching.

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

/// `ParticleSystem.MinMaxCurve`.
#[derive(Debug, Clone, Deserialize)]
pub enum Curve {
    Const(f32),
    RandConst(f32, f32),
    #[serde(rename = "Curve")]
    Keys(Vec<Key>),
    TwoCurves(Vec<Key>, Vec<Key>),
}

/// One Hermite key: time, value, in-slope, out-slope.
pub type Key = (f32, f32, f32, f32);

/// A particle quad: material, corners as `[x, y]` pixels, uv rect, colour.
pub type Billboard = (Material, [[f32; 2]; 4], [f32; 4], [f32; 4]);

impl Curve {
    pub fn at(&self, t: f32) -> f32 {
        match self {
            Curve::Const(v) => *v,
            Curve::RandConst(_, max) => *max,
            Curve::Keys(k) => sample(k, t),
            Curve::TwoCurves(_, max) => sample(max, t),
        }
    }

    /// Unity's `MinMaxCurve`: `RandomBetweenTwoCurves` picks one `t` per particle and
    /// interpolates between the two curves; the other modes return `at`.
    pub fn sample(&self, rnd: f32, t: f32) -> f32 {
        match self {
            Curve::RandConst(min, max) => min + (max - min) * rnd,
            Curve::TwoCurves(min, max) => {
                let (a, b) = (sample(min, t), sample(max, t));
                a + (b - a) * rnd
            }
            _ => self.at(t),
        }
    }
}

/// Unity animation curves are cubic Hermite between keys, clamped outside the range
/// (`m_PreInfinity == m_PostInfinity == 2`).
fn sample(keys: &[Key], t: f32) -> f32 {
    match keys {
        [] => 0.0,
        [(_, v, _, _)] => *v,
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
                    let u = (t - t0) / dt;
                    let (u2, u3) = (u * u, u * u * u);
                    let h00 = 2.0 * u3 - 3.0 * u2 + 1.0;
                    let h10 = u3 - 2.0 * u2 + u;
                    let h01 = -2.0 * u3 + 3.0 * u2;
                    let h11 = u3 - u2;
                    return h00 * v0 + h10 * dt * s0 + h01 * v1 + h11 * dt * s1;
                }
            }
            keys[keys.len() - 1].1
        }
    }
}

/// `ParticleSystem.MinMaxGradient` in its `Gradient` mode: colour over lifetime plus a
/// separate alpha over lifetime (TMP's gradients serialize the two tracks independently).
#[derive(Debug, Clone, Deserialize)]
pub struct Gradient {
    pub colors: Vec<(f32, f32, f32, f32)>,
    pub alphas: Vec<(f32, f32)>,
}

impl Gradient {
    fn at(&self, t: f32) -> [f32; 4] {
        let c = match self.colors.as_slice() {
            [] => [1.0, 1.0, 1.0],
            _ => {
                let ks: Vec<Key> = self.colors.iter().map(|k| (k.0, k.1, 0.0, 0.0)).collect();
                let gs: Vec<Key> = self.colors.iter().map(|k| (k.0, k.2, 0.0, 0.0)).collect();
                let bs: Vec<Key> = self.colors.iter().map(|k| (k.0, k.3, 0.0, 0.0)).collect();
                [sample(&ks, t), sample(&gs, t), sample(&bs, t)]
            }
        };
        let al: Vec<Key> = self.alphas.iter().map(|k| (k.0, k.1, 0.0, 0.0)).collect();
        [c[0], c[1], c[2], sample(&al, t)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub enum Material {
    /// `Sekai/Particles/Additive`: `tex × vertexColour`, blend SrcAlpha / One.
    Additive,
    /// `Sekai/Particles/AlphaBlended`: blend SrcAlpha / OneMinusSrcAlpha.
    AlphaBlended,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Burst {
    pub time: f32,
    pub count: Curve,
    pub cycles: u32,
    pub interval: f32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Shape {
    pub angle: f32,
    pub radius: f32,
    /// Euler degrees applied to the cone (`m_Rotation`).
    pub rotation: [f32; 3],
    /// The cone's own scale — the prefab sets x to 0, collapsing the base disc onto a line.
    pub scale: [f32; 3],
    pub position: [f32; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct Velocity {
    pub x: Curve,
    pub y: Curve,
    pub z: Curve,
    pub speed_modifier: Curve,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[allow(dead_code)] // `octaves` is 1 for every emitter; kept from the prefab
pub struct Noise {
    pub strength: f32,
    pub frequency: f32,
    pub octaves: u32,
    pub damping: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Clamp {
    pub limit: Curve,
    pub dampen: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // `gravity` (0) and `length` (bursts end long before it) are unused
pub struct Emitter {
    pub name: String,
    pub material: Material,
    pub sorting_order: i32,
    /// `UVModule.startFrame` scaled to the 4×4 atlas' tile index.
    pub tile: f32,
    pub position: [f32; 3],
    pub lifetime: Curve,
    pub speed: Curve,
    pub size: Curve,
    pub rotation: Curve,
    pub rotation_x: Curve,
    pub rotation_y: Curve,
    pub color: Gradient,
    pub gravity: Curve,
    pub max_particles: u32,
    pub shape: Shape,
    pub bursts: Vec<Burst>,
    pub velocity: Velocity,
    pub angular_velocity: [Curve; 3],
    pub noise: Noise,
    pub clamp: Option<Clamp>,
    pub length: f32,
    /// `autoRandomSeed` — the game randomises these every play, so a fixed seed of ours is
    /// as faithful as any: only the distribution is meaningful.
    pub auto_seed: bool,
    pub random_seed: i32,
}

/// `fx_transition_scenario.json`: the prefab's emitters in hierarchy order.
#[derive(Debug, Clone, Deserialize)]
pub struct FxPrefab {
    format: String,
    version: u32,
    pub emitters: Vec<Emitter>,
}

impl FxPrefab {
    pub const FILE: &str = "fx_transition_scenario.json";

    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let prefab: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if prefab.format != "sse-fx" || prefab.version != 1 {
            return Err(format!(
                "{}: expected sse-fx v1, found {} v{}",
                path.display(),
                prefab.format,
                prefab.version
            ));
        }
        Ok(prefab)
    }
}

/// One live particle, in the effect's local space (world units).
struct Particle {
    pos: [f32; 3],
    vel: [f32; 3],
    age: f32,
    life: f32,
    size: f32,
    rot: [f32; 3],
    ang_vel: [f32; 3],
    /// Noise-field seed and a per-particle random stream (`RandomBetweenTwoCurves`
    /// samples a fresh `t` every frame, as Unity does).
    seed: u32,
    rng: u32,
}

/// Per-emitter runtime state.
struct EmitterState {
    particles: Vec<Particle>,
    rng: u32,
    /// Emitter-local clock, `None` until the prefab is instantiated.
    time: Option<f32>,
    /// Bursts already fired, as (cycle, burst index).
    fired: Vec<(u32, usize)>,
}

pub struct TransitionFx {
    prefab: Arc<FxPrefab>,
    states: Vec<EmitterState>,
    elapsed: f32,
}

impl TransitionFx {
    /// Starts `fx_transition_scenario` (`GameObject.Instantiate` under `effectLayer`).
    /// `instance` seeds the emitters that carry `autoRandomSeed` (the game randomises those
    /// per instance; ours is derived from the instantiation frame, so it is reproducible).
    pub fn new(prefab: Arc<FxPrefab>, instance: u32) -> Self {
        Self {
            states: prefab
                .emitters
                .iter()
                .map(|e| EmitterState {
                    particles: Vec::new(),
                    rng: if e.auto_seed {
                        seed_of(&e.name) ^ instance.wrapping_mul(2654435761)
                    } else {
                        e.random_seed as u32
                    },
                    time: Some(0.0),
                    fired: Vec::new(),
                })
                .collect(),
            prefab,
            elapsed: 0.0,
        }
    }

    /// Advances one frame. `dt` is the frame duration; the game runs these with
    /// `useUnscaledTime = false` at the scenario's frame rate.
    pub fn step(&mut self, dt: f32) {
        self.elapsed += dt;
        for (e, st) in self.prefab.emitters.iter().zip(self.states.iter_mut()) {
            let Some(time) = st.time.as_mut() else {
                continue;
            };
            *time += dt;
            let t = *time;
            emit(e, st, t);
            update(e, st, dt);
        }
    }

    /// Live particle count (diagnostics and tests).
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.states.iter().map(|s| s.particles.len()).sum()
    }

    /// Billboards in draw order (see the sort below).
    /// Returns `(material, corners as [x, y] pixels, uv rect, colour)`; the UV rect follows
    /// the `tex_common_tri_01` 4×4 atlas with v measured upward, as Unity samples it.
    pub fn billboards(&self, screen: [f32; 2]) -> Vec<Billboard> {
        // Same sorting layer and order (245) for every emitter, so Unity falls back to camera
        // distance, far first: the `tri_0N (1)` glow copies sit at local z = −10, nearer the
        // camera (which looks down +z), and draw over their siblings.
        let emitters = &self.prefab.emitters;
        let mut order: Vec<usize> = (0..emitters.len()).collect();
        order.sort_by(|&a, &b| {
            let (ea, eb) = (&emitters[a], &emitters[b]);
            ea.sorting_order
                .cmp(&eb.sorting_order)
                .then(eb.position[2].total_cmp(&ea.position[2]))
        });
        let world_h = 2.0;
        let px_per_unit = screen[1] / world_h;
        let mut out = Vec::new();
        for i in order {
            let e = &emitters[i];
            let base = [
                e.tile.rem_euclid(4.0) as u32,
                (e.tile / 4.0).floor().max(0.0) as u32,
            ];
            for p in &self.states[i].particles {
                let t = (p.age / p.life).clamp(0.0, 1.0);
                let c = e.color.at(t);
                if c[3] <= 0.001 {
                    continue;
                }
                let h = p.size * 0.5;
                // Unity rotates the billboard quad; an orthographic camera flattens the z.
                let corners = [[-h, -h, 0.0], [h, -h, 0.0], [h, h, 0.0], [-h, h, 0.0]];
                let mut pts = [[0.0f32; 2]; 4];
                for (k, corner) in corners.iter().enumerate() {
                    let q = rot_euler(*corner, p.rot);
                    let w = [p.pos[0] + q[0], p.pos[1] + q[1], p.pos[2] + q[2]];
                    // EffectLayer's world is the UI camera's: origin centred, y up.
                    pts[k] = [
                        screen[0] * 0.5 + w[0] * px_per_unit,
                        screen[1] * 0.5 - w[1] * px_per_unit,
                    ];
                }
                let (tw, th) = (1.0 / 4.0, 1.0 / 4.0);
                let uv = [
                    base[0] as f32 * tw,
                    1.0 - (base[1] + 1) as f32 * th,
                    (base[0] + 1) as f32 * tw,
                    1.0 - base[1] as f32 * th,
                ];
                out.push((e.material, pts, uv, c));
            }
        }
        out
    }
}

/// `Quaternion.Euler(x, y, z)` = `Ry·Rx·Rz` (Unity's yaw·pitch·roll order).
fn rot_euler(v: [f32; 3], deg: [f32; 3]) -> [f32; 3] {
    let (rx, ry, rz) = (
        deg[0].to_radians(),
        deg[1].to_radians(),
        deg[2].to_radians(),
    );
    let (sx, cx) = rx.sin_cos();
    let (sy, cy) = ry.sin_cos();
    let (sz, cz) = rz.sin_cos();
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

fn seed_of(name: &str) -> u32 {
    let mut h = 2166136261u32;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h | 1
}

/// xorshift32.
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

/// `EmissionModule`: bursts fire once per cycle at `time + interval × cycle`.
fn emit(e: &Emitter, st: &mut EmitterState, t: f32) {
    for (bi, b) in e.bursts.iter().enumerate() {
        for cycle in 0..b.cycles.max(1) {
            let at = b.time + b.interval * cycle as f32;
            if t + 1e-6 < at || st.fired.contains(&(cycle, bi)) {
                continue;
            }
            st.fired.push((cycle, bi));
            let count = b.count.at(1.0) as u32;
            for _ in 0..count {
                if st.particles.len() as u32 >= e.max_particles {
                    break;
                }
                st.particles.push(spawn(e, &mut st.rng));
            }
        }
    }
}

fn spawn(e: &Emitter, rng: &mut u32) -> Particle {
    let life = e.lifetime.sample(unit(rng), 0.0);
    let speed = e.speed.sample(unit(rng), 0.0);
    let size = e.size.sample(unit(rng), 0.0);
    let rot = [
        e.rotation_x.sample(unit(rng), 0.0).to_degrees(),
        e.rotation_y.sample(unit(rng), 0.0).to_degrees(),
        e.rotation.sample(unit(rng), 0.0).to_degrees(),
    ];
    let ang_vel = [
        e.angular_velocity[0].sample(unit(rng), 0.0).to_degrees(),
        e.angular_velocity[1].sample(unit(rng), 0.0).to_degrees(),
        e.angular_velocity[2].sample(unit(rng), 0.0).to_degrees(),
    ];
    // Shape `type 4` = cone: the base disc (radius) with `angle` opening, emitted along +z.
    // `radiusThickness 1` fills the disc, hence `sqrt` on the radius.
    let r = unit(rng).sqrt() * e.shape.radius;
    let theta = unit(rng) * std::f32::consts::TAU;
    let (st_, ct) = theta.sin_cos();
    let a = e.shape.angle.to_radians() * 0.5;
    let (sa, ca) = (unit(rng) * a).sin_cos();
    let local_pos = [r * ct * e.shape.scale[0], r * st_ * e.shape.scale[1], 0.0];
    let local_dir = [
        sa * st_ * e.shape.scale[0],
        sa * ct * e.shape.scale[1],
        ca * e.shape.scale[2],
    ];
    let q = rot_euler(local_pos, e.shape.rotation);
    let d = rot_euler(local_dir, e.shape.rotation);
    let pos = [
        e.position[0] + e.shape.position[0] + q[0],
        e.position[1] + e.shape.position[1] + q[1],
        e.position[2] + e.shape.position[2] + q[2],
    ];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-6);
    let vel = [d[0] / len * speed, d[1] / len * speed, d[2] / len * speed];
    Particle {
        pos,
        vel,
        age: 0.0,
        life,
        size,
        rot,
        ang_vel,
        seed: next_u32(rng),
        rng: next_u32(rng) | 1,
    }
}

fn update(e: &Emitter, st: &mut EmitterState, dt: f32) {
    let mut i = 0;
    while i < st.particles.len() {
        let p = &mut st.particles[i];
        p.age += dt;
        if p.age >= p.life {
            st.particles.swap_remove(i);
            continue;
        }
        let t = p.age / p.life;
        // `VelocityModule` adds a velocity on top of the initial one, so the position
        // integrates the sum rather than accumulating an acceleration.
        let sm = e.velocity.speed_modifier.at(t);
        let mut v = [
            p.vel[0] + e.velocity.x.sample(unit(&mut p.rng), t) * sm,
            p.vel[1] + e.velocity.y.sample(unit(&mut p.rng), t) * sm,
            p.vel[2] + e.velocity.z.sample(unit(&mut p.rng), t) * sm,
        ];
        // `NoiseModule` with `positionAmount` displaces the particle in a static field
        // (`scrollSpeed == 0`); `damping` fades the field out over the particle's life.
        // `strength / frequency` keeps the displacement comparable across frequencies.
        if e.noise.strength != 0.0 {
            let (nx, ny, nz) = noise_field(p.pos, e.noise.frequency, p.seed);
            let mut k = e.noise.strength / e.noise.frequency.max(1e-4);
            if e.noise.damping {
                k *= 1.0 - t;
            }
            v[0] += nx * k;
            v[1] += ny * k;
            v[2] += nz * k;
        }
        // `ClampVelocityModule` limits the *base* velocity's magnitude to `limit`,
        // dragging it back at the `dampen` rate; the lifetime velocity rides on top.
        if let Some(c) = &e.clamp {
            let limit = c.limit.sample(unit(&mut p.rng), t).max(1e-5);
            let mag = (p.vel[0] * p.vel[0] + p.vel[1] * p.vel[1] + p.vel[2] * p.vel[2]).sqrt();
            if mag > limit {
                let f = limit / mag;
                p.vel[0] *= f;
                p.vel[1] *= f;
                p.vel[2] *= f;
            } else if c.dampen > 0.0 {
                let f = (1.0 - c.dampen * dt).clamp(0.0, 1.0);
                p.vel[0] *= f;
                p.vel[1] *= f;
                p.vel[2] *= f;
            }
        }
        p.pos[0] += v[0] * dt;
        p.pos[1] += v[1] * dt;
        p.pos[2] += v[2] * dt;
        p.rot[0] += p.ang_vel[0] * dt;
        p.rot[1] += p.ang_vel[1] * dt;
        p.rot[2] += p.ang_vel[2] * dt;
        i += 1;
    }
}

/// The noise field shared with the generic particle systems (`particle.rs`).
pub fn noise3(pos: [f32; 3], freq: f32, seed: u32) -> (f32, f32, f32) {
    noise_field(pos, freq, seed)
}

/// Value noise in a 2D lattice, trilinear in 3D: an approximation of Unity's
/// `NoiseModule` field (see the module docs).
fn noise_field(pos: [f32; 3], freq: f32, seed: u32) -> (f32, f32, f32) {
    let x = pos[0] * freq;
    let y = pos[1] * freq;
    let z = pos[2] * freq;
    let axis = |a: u32| {
        let v = vnoise(
            x + a as f32 * 37.0,
            y + a as f32 * 11.0,
            z + a as f32 * 53.0,
            seed.wrapping_add(a),
        );
        v * 2.0 - 1.0
    };
    (axis(0), axis(1), axis(2))
}

fn vnoise(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let (xi, yi, zi) = (x.floor(), y.floor(), z.floor());
    let (xf, yf, zf) = (x - xi, y - yi, z - zi);
    let (u, v, w) = (fade(xf), fade(yf), fade(zf));
    let h = |i: f32, j: f32, k: f32| {
        let n = ((xi + i) as i32 as u32).wrapping_mul(374761393)
            ^ ((yi + j) as i32 as u32).wrapping_mul(668265263)
            ^ ((zi + k) as i32 as u32).wrapping_mul(2246822519)
            ^ seed;
        let mut n2 = n.wrapping_mul(1274126177);
        n2 ^= n2 >> 15;
        (n2 >> 8) as f32 / (1u32 << 24) as f32
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(h(0.0, 0.0, 0.0), h(1.0, 0.0, 0.0), u);
    let x10 = lerp(h(0.0, 1.0, 0.0), h(1.0, 1.0, 0.0), u);
    let x01 = lerp(h(0.0, 0.0, 1.0), h(1.0, 0.0, 1.0), u);
    let x11 = lerp(h(0.0, 1.0, 1.0), h(1.0, 1.0, 1.0), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

fn fade(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn euler_matches_unity_yaw_pitch_roll() {
        // the prefab's cone rotation (-35, -90, 0): the axis must point up-left
        let axis = rot_euler([0.0, 0.0, 1.0], [-35.0, -90.0, 0.0]);
        assert!(
            (axis[0] + 0.819).abs() < 1e-3
                && (axis[1] - 0.574).abs() < 1e-3
                && axis[2].abs() < 1e-3,
            "{axis:?}"
        );
    }

    #[test]
    fn hermite_curves_hit_their_keys_and_clamp() {
        let k: &[Key] = &[(0.0, 1.0, 0.0, 0.0), (1.0, 3.0, 0.0, 0.0)];
        assert_eq!(sample(k, -1.0), 1.0);
        assert_eq!(sample(k, 2.0), 3.0);
        assert!((sample(k, 0.5) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn bursts_fire_once_per_cycle() {
        let json = r#"{"format": "sse-fx", "version": 1, "emitters": [{
            "name": "a", "material": "Additive", "sorting_order": 245, "tile": 0.0,
            "position": [0.0, 0.0, 0.0], "lifetime": {"Const": 5.0}, "speed": {"Const": 1.0},
            "size": {"RandConst": [0.1, 0.4]}, "rotation": {"Const": 0.0},
            "rotation_x": {"Const": 0.0}, "rotation_y": {"Const": 0.0},
            "color": {"colors": [[0.0, 1.0, 1.0, 1.0]], "alphas": [[0.0, 1.0], [1.0, 0.0]]},
            "gravity": {"Const": 0.0}, "max_particles": 1000,
            "shape": {"angle": 35.0, "radius": 0.8, "rotation": [0.0, 0.0, 0.0],
                      "scale": [1.0, 1.0, 1.0], "position": [0.0, 0.0, 0.0]},
            "bursts": [{"time": 0.0, "count": {"Const": 20.0}, "cycles": 1, "interval": 0.01},
                       {"time": 0.1, "count": {"Const": 7.0}, "cycles": 3, "interval": 0.2}],
            "velocity": {"x": {"Const": 0.0}, "y": {"Const": 0.0}, "z": {"Const": 0.0},
                         "speed_modifier": {"Const": 1.0}},
            "angular_velocity": [{"RandConst": [0.0, 0.0]}, {"RandConst": [0.0, 0.0]},
                                 {"RandConst": [-1.0, 1.0]}],
            "noise": {"strength": 0.0, "frequency": 0.5, "octaves": 1, "damping": true},
            "clamp": null, "length": 5.0, "auto_seed": true, "random_seed": 0
        }]}"#;
        let prefab: FxPrefab = serde_json::from_str(json).unwrap();
        let mut fx = TransitionFx::new(Arc::new(prefab), 1);
        for _ in 0..60 {
            fx.step(1.0 / 60.0);
        }
        // 20 + 7 × 3 cycles (0.1, 0.3, 0.5 s)
        assert_eq!(fx.len(), 41);
    }
}
