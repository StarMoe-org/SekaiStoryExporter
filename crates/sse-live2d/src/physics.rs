//! Cubism physics, as run by the game's Cubism SDK for Unity (4.x, before 4.2).
//!
//! `CubismPhysicsController` evaluates once per `LateUpdate` with `Time.deltaTime`: no fixed
//! physics FPS and no output interpolation (both were added in SDK 4.2 / 5). Algorithm and
//! constants follow the Cubism Framework semantics (ADR-0003: reimplemented in Rust).
//! The rotation of `totalTranslation` deliberately reuses the already-rotated X, exactly as
//! the SDK does.

use serde::Deserialize;
use sse_core::det_math::{atan2f, cosf, sinf, sqrtf};

const MAXIMUM_WEIGHT: f32 = 100.0;
const MOVEMENT_THRESHOLD: f32 = 0.001;
const AIR_RESISTANCE: f32 = 5.0;
const PI: f32 = std::f32::consts::PI;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct V2 {
    x: f32,
    y: f32,
}

impl V2 {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
    fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }
    fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }
    fn scale(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }
    fn normalized(self) -> V2 {
        let len = sqrtf(self.x * self.x + self.y * self.y);
        if len == 0.0 {
            self
        } else {
            V2::new(self.x / len, self.y / len)
        }
    }
}

fn direction_to_radian(from: V2, to: V2) -> f32 {
    let q1 = atan2f(to.y, to.x);
    let q2 = atan2f(from.y, from.x);
    let mut ret = q1 - q2;
    while ret < -PI {
        ret += PI * 2.0;
    }
    while ret > PI {
        ret -= PI * 2.0;
    }
    ret
}

fn radian_to_direction(a: f32) -> V2 {
    V2::new(sinf(a), cosf(a))
}

fn degrees_to_radian(d: f32) -> f32 {
    d / 180.0 * PI
}

// ---- physics3.json -------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Json {
    physics_settings: Vec<JSetting>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JSetting {
    input: Vec<JInput>,
    output: Vec<JOutput>,
    vertices: Vec<JVertex>,
    normalization: JNormalization,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JId {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JInput {
    source: JId,
    weight: f32,
    #[serde(rename = "Type")]
    kind: String,
    reflect: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JOutput {
    destination: JId,
    vertex_index: usize,
    scale: f32,
    weight: f32,
    #[serde(rename = "Type")]
    kind: String,
    reflect: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JXy {
    x: f32,
    y: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JVertex {
    position: JXy,
    mobility: f32,
    delay: f32,
    acceleration: f32,
    radius: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JRange {
    minimum: f32,
    default: f32,
    maximum: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct JNormalization {
    position: JRange,
    angle: JRange,
}

// ---- rig ------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    X,
    Y,
    Angle,
}

fn kind(s: &str) -> Kind {
    match s {
        "X" => Kind::X,
        "Y" => Kind::Y,
        _ => Kind::Angle,
    }
}

#[derive(Debug, Clone)]
struct Input {
    param: Option<usize>,
    weight: f32,
    kind: Kind,
    reflect: bool,
}

#[derive(Debug, Clone)]
struct Output {
    param: Option<usize>,
    vertex: usize,
    scale: f32,
    weight: f32,
    kind: Kind,
    reflect: bool,
}

#[derive(Debug, Clone)]
struct Particle {
    mobility: f32,
    delay: f32,
    acceleration: f32,
    radius: f32,
    position: V2,
    last_position: V2,
    last_gravity: V2,
    velocity: V2,
    force: V2,
}

#[derive(Debug, Clone, Copy)]
struct Norm {
    min: f32,
    def: f32,
    max: f32,
}

#[derive(Debug, Clone)]
struct SubRig {
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    particles: Vec<Particle>,
    norm_position: Norm,
    norm_angle: Norm,
}

/// A physics rig bound to one model's parameter table.
#[derive(Debug, Clone)]
pub struct Physics {
    rigs: Vec<SubRig>,
    wind: V2,
    gravity: V2,
}

impl Physics {
    pub fn from_json(bytes: &[u8], parameter_ids: &[String]) -> Result<Self, serde_json::Error> {
        let json: Json = serde_json::from_slice(bytes)?;
        let find = |id: &str| parameter_ids.iter().position(|p| p == id);
        let rigs = json
            .physics_settings
            .into_iter()
            .map(|s| {
                let mut particles: Vec<Particle> = s
                    .vertices
                    .iter()
                    .map(|v| Particle {
                        mobility: v.mobility,
                        delay: v.delay,
                        acceleration: v.acceleration,
                        radius: v.radius,
                        position: V2::new(v.position.x, v.position.y),
                        last_position: V2::default(),
                        last_gravity: V2::new(0.0, 1.0),
                        velocity: V2::default(),
                        force: V2::default(),
                    })
                    .collect();
                // Initialize(): chain hangs straight down from the origin.
                let mut prev = V2::default();
                for (i, p) in particles.iter_mut().enumerate() {
                    let initial = if i == 0 {
                        V2::default()
                    } else {
                        prev.add(V2::new(0.0, p.radius))
                    };
                    p.position = initial;
                    p.last_position = initial;
                    prev = initial;
                }
                SubRig {
                    inputs: s
                        .input
                        .iter()
                        .map(|i| Input {
                            param: find(&i.source.id),
                            weight: i.weight,
                            kind: kind(&i.kind),
                            reflect: i.reflect,
                        })
                        .collect(),
                    outputs: s
                        .output
                        .iter()
                        .map(|o| Output {
                            param: find(&o.destination.id),
                            vertex: o.vertex_index,
                            scale: o.scale,
                            weight: o.weight,
                            kind: kind(&o.kind),
                            reflect: o.reflect,
                        })
                        .collect(),
                    particles,
                    norm_position: Norm {
                        min: s.normalization.position.minimum,
                        def: s.normalization.position.default,
                        max: s.normalization.position.maximum,
                    },
                    norm_angle: Norm {
                        min: s.normalization.angle.minimum,
                        def: s.normalization.angle.default,
                        max: s.normalization.angle.maximum,
                    },
                }
            })
            .collect();
        Ok(Self {
            rigs,
            wind: V2::default(),
            gravity: V2::new(0.0, -1.0),
        })
    }

    /// One `LateUpdate`: reads and writes `values` in place.
    pub fn evaluate(&mut self, values: &mut [f32], min: &[f32], max: &[f32], delta: f32) {
        if delta <= 0.0 {
            return;
        }
        for rig in &mut self.rigs {
            let mut total_translation = V2::default();
            let mut total_angle = 0.0_f32;
            for input in &rig.inputs {
                let Some(p) = input.param else { continue };
                let w = input.weight / MAXIMUM_WEIGHT;
                let norm = if input.kind == Kind::Angle {
                    rig.norm_angle
                } else {
                    rig.norm_position
                };
                let v = normalize(values[p], min[p], max[p], norm, input.reflect) * w;
                match input.kind {
                    Kind::X => total_translation.x += v,
                    Kind::Y => total_translation.y += v,
                    Kind::Angle => total_angle += v,
                }
            }
            let rad = degrees_to_radian(-total_angle);
            total_translation.x = total_translation.x * cosf(rad) - total_translation.y * sinf(rad);
            total_translation.y = total_translation.x * sinf(rad) + total_translation.y * cosf(rad);

            update_particles(
                &mut rig.particles,
                total_translation,
                total_angle,
                self.wind,
                MOVEMENT_THRESHOLD * rig.norm_position.max,
                delta,
            );

            for output in &rig.outputs {
                let Some(p) = output.param else { continue };
                let i = output.vertex;
                if i < 1 || i >= rig.particles.len() {
                    continue;
                }
                let translation = rig.particles[i].position.sub(rig.particles[i - 1].position);
                let mut value = match output.kind {
                    Kind::X => translation.x,
                    Kind::Y => translation.y,
                    Kind::Angle => {
                        let parent = if i >= 2 {
                            rig.particles[i - 1]
                                .position
                                .sub(rig.particles[i - 2].position)
                        } else {
                            self.gravity.scale(-1.0)
                        };
                        direction_to_radian(parent, translation)
                    }
                };
                if output.reflect {
                    value = -value;
                }
                let mut v = value * output.scale;
                if v < min[p] {
                    v = min[p];
                } else if v > max[p] {
                    v = max[p];
                }
                let w = output.weight / MAXIMUM_WEIGHT;
                values[p] = if w >= 1.0 {
                    v
                } else {
                    values[p] * (1.0 - w) + v * w
                };
            }
        }
    }
}

fn normalize(value: f32, pmin: f32, pmax: f32, n: Norm, inverted: bool) -> f32 {
    let max = pmax.max(pmin);
    let min = pmax.min(pmin);
    let value = value.clamp(min, max);
    let nmin = n.min.min(n.max);
    let nmax = n.min.max(n.max);
    let middle = min + (max - min).abs() / 2.0;
    let pv = value - middle;
    let result = if pv > 0.0 {
        let plen = max - middle;
        if plen != 0.0 {
            pv * ((nmax - n.def) / plen) + n.def
        } else {
            0.0
        }
    } else if pv < 0.0 {
        let plen = min - middle;
        if plen != 0.0 {
            pv * ((nmin - n.def) / plen) + n.def
        } else {
            0.0
        }
    } else {
        n.def
    };
    if inverted { result } else { -result }
}

fn update_particles(
    strand: &mut [Particle],
    total_translation: V2,
    total_angle: f32,
    wind: V2,
    threshold: f32,
    delta: f32,
) {
    strand[0].position = total_translation;
    let gravity = radian_to_direction(degrees_to_radian(total_angle)).normalized();
    for i in 1..strand.len() {
        let prev_pos = strand[i - 1].position;
        let p = &mut strand[i];
        p.force = gravity.scale(p.acceleration).add(wind);
        p.last_position = p.position;
        let delay = p.delay * delta * 30.0;
        let mut direction = p.position.sub(prev_pos);
        let radian = direction_to_radian(p.last_gravity, gravity) / AIR_RESISTANCE;
        direction.x = cosf(radian) * direction.x - direction.y * sinf(radian);
        direction.y = sinf(radian) * direction.x + direction.y * cosf(radian);
        p.position = prev_pos.add(direction);
        let velocity = p.velocity.scale(delay);
        let force = p.force.scale(delay * delay);
        p.position = p.position.add(velocity).add(force);
        let new_direction = p.position.sub(prev_pos).normalized();
        p.position = prev_pos.add(new_direction.scale(p.radius));
        if p.position.x.abs() < threshold {
            p.position.x = 0.0;
        }
        if delay != 0.0 {
            let d = p.position.sub(p.last_position);
            p.velocity = V2::new(d.x / delay, d.y / delay).scale(p.mobility);
        }
        p.force = V2::default();
        p.last_gravity = gravity;
    }
}
