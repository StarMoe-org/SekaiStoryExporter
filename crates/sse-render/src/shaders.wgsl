// sse-render shaders. Determinism rules R-2..R-4: no transcendental functions, f32 only.

// ---------------------------------------------------------------- Cubism (into the character RT)

struct Draw {
    // world = offset + scale * vertex; ndc = world / half_extent
    xform: vec4<f32>,      // (scale, offset_x, offset_y, _)
    half_extent: vec4<f32>,// (half_w, half_h, rt_w, rt_h)
    tint: vec4<f32>,       // ambient model colour (rgb), model opacity (a)
    params: vec4<f32>,     // (drawable opacity, masked, inverted, _)
};

@group(0) @binding(0) var<storage, read> draws: array<Draw>;
@group(0) @binding(1) var model_tex: texture_2d<f32>;
@group(0) @binding(2) var model_smp: sampler;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;

struct CubismOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) draw: u32,
};

@vertex
fn cubism_vs(@location(0) p: vec2<f32>, @location(1) uv: vec2<f32>,
             @builtin(instance_index) draw: u32) -> CubismOut {
    let d = draws[draw];
    let world = vec2<f32>(d.xform.y + d.xform.x * p.x, d.xform.z + d.xform.x * p.y);
    var o: CubismOut;
    o.pos = vec4<f32>(world.x / d.half_extent.x, world.y / d.half_extent.y, 0.0, 1.0);
    o.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    o.draw = draw;
    return o;
}

@fragment
fn cubism_fs(i: CubismOut) -> @location(0) vec4<f32> {
    let d = draws[i.draw];
    let tex = textureSample(model_tex, model_smp, i.uv);
    var c = vec4<f32>(tex.rgb, tex.a * d.params.x);
    // Live2D Cubism/Unlit: rgb *= a; col *= cubism_ModelOpacity
    c = vec4<f32>(c.rgb * c.a, c.a);
    c = c * d.tint.a;
    c = vec4<f32>(c.rgb * d.tint.rgb, c.a);
    if (d.params.y > 0.5) {
        let m_uv = i.pos.xy / d.half_extent.zw;
        var m = textureSample(mask_tex, model_smp, m_uv).r;
        if (d.params.z > 0.5) {
            m = 1.0 - m;
        }
        c = c * m;
    }
    return c;
}

@fragment
fn mask_fs(i: CubismOut) -> @location(0) vec4<f32> {
    let d = draws[i.draw];
    let tex = textureSample(model_tex, model_smp, i.uv);
    let a = tex.a * d.params.x;
    return vec4<f32>(a, a, a, a);
}

// ---------------------------------------------------------------- screen quads

struct Quad {
    rect: vec4<f32>,   // x, y, w, h in target pixels (top-left origin)
    uv: vec4<f32>,     // u0, v0, u1, v1
    color: vec4<f32>,  // vertex colour
    dst: vec4<f32>,    // target w, h, mode, _
};

@group(0) @binding(0) var<uniform> quad: Quad;
@group(0) @binding(1) var quad_tex: texture_2d<f32>;
@group(0) @binding(2) var quad_smp: sampler;

struct QuadOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn quad_vs(@builtin(vertex_index) vi: u32) -> QuadOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    let px = quad.rect.xy + corner * quad.rect.zw;
    var o: QuadOut;
    o.pos = vec4<f32>(px.x / quad.dst.x * 2.0 - 1.0, 1.0 - px.y / quad.dst.y * 2.0, 0.0, 1.0);
    o.uv = mix(quad.uv.xy, quad.uv.zw, corner);
    return o;
}

@fragment
fn quad_fs(i: QuadOut) -> @location(0) vec4<f32> {
    let t = textureSample(quad_tex, quad_smp, i.uv);
    let c = t * quad.color;
    if (quad.dst.z > 0.5) {
        // premultiplied source (text canvas)
        return t * quad.color.a;
    }
    // UI/Default: out.rgb = c.rgb * c.a (straight-alpha assumption)
    return vec4<f32>(c.rgb * c.a, c.a);
}

// ---------------------------------------------------------------- post (blur + monotone)

struct Post {
    step: vec4<f32>,    // texel step x, y, blur mix, _
    mono: vec4<f32>,
    tone: vec4<f32>,
    influence: vec4<f32>, // influence, enabled, _, _
};

@group(0) @binding(0) var<uniform> post: Post;
@group(0) @binding(1) var post_tex: texture_2d<f32>;
@group(0) @binding(2) var post_smp: sampler;

@vertex
fn post_vs(@builtin(vertex_index) vi: u32) -> QuadOut {
    let corner = vec2<f32>(f32(vi & 1u), f32((vi >> 1u) & 1u));
    var o: QuadOut;
    o.pos = vec4<f32>(corner.x * 2.0 - 1.0, 1.0 - corner.y * 2.0, 0.0, 1.0);
    o.uv = corner;
    return o;
}

@fragment
fn blur_fs(i: QuadOut) -> @location(0) vec4<f32> {
    // ScenarioGuassianBlur 5-tap weights
    let s = post.step.xy;
    var c = textureSample(post_tex, post_smp, i.uv) * 0.4026;
    c = c + textureSample(post_tex, post_smp, i.uv + s * 1.0) * 0.2442;
    c = c + textureSample(post_tex, post_smp, i.uv - s * 1.0) * 0.2442;
    c = c + textureSample(post_tex, post_smp, i.uv + s * 2.0) * 0.0545;
    c = c + textureSample(post_tex, post_smp, i.uv - s * 2.0) * 0.0545;
    return c;
}

@fragment
fn mono_fs(i: QuadOut) -> @location(0) vec4<f32> {
    let c = textureSample(post_tex, post_smp, i.uv);
    if (post.influence.y < 0.5) {
        return vec4<f32>(c.rgb, 1.0);
    }
    // MonotoneColord: lum = dot(c.rgb, mono) * c.a; out = tone*lum*inf + c*(1-inf); a = 1
    let lum = dot(c.rgb, post.mono.rgb) * c.a;
    let inf = post.influence.x;
    return vec4<f32>(post.tone.rgb * lum * inf + c.rgb * (1.0 - inf), 1.0);
}
