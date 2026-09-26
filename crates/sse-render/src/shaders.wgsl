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
    extra: vec4<f32>,  // mode 2: _Line, _SubColor.a, _SubTex.r; mode 3: _SamplingDistance
    mask0: vec4<f32>,  // sprite mask: corner xy, inverse basis row 0
    mask1: vec4<f32>,  // inverse basis row 1, interaction (1 inside / 2 outside), cutoff
    mask2: vec4<f32>,  // mask sprite texture rect u0, v0, u1, v1
};

@group(0) @binding(0) var<uniform> quad: Quad;
@group(0) @binding(1) var quad_tex: texture_2d<f32>;
@group(0) @binding(2) var quad_smp: sampler;
@group(0) @binding(3) var quad_mask: texture_2d<f32>;

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
    if (quad.dst.z > 4.5) {
        // Sekai/UI/UIDollyZoomEffect: barrel distortion of the raw UV about the centre,
        // uv' = 0.5 + d·(1 + _DistortionStrength·|d|²), clamped to [0, 1]; then, when
        // _SamplingDistance > 0.001, the UIGaussianBlur cross with every tap clamped.
        let d = i.uv - vec2<f32>(0.5);
        let uv = clamp(d * (1.0 + quad.extra.y * dot(d, d)) + vec2<f32>(0.5), vec2<f32>(0.0), vec2<f32>(1.0));
        var c3 = vec4<f32>(0.0);
        if (quad.extra.x > 0.001) {
            let texel = 1.0 / vec2<f32>(textureDimensions(quad_tex));
            let sy = vec2<f32>(0.0, texel.y * quad.extra.x);
            let sx = vec2<f32>(texel.x * quad.extra.x, 0.0);
            let w = array<f32, 7>(0.036, 0.113, 0.216, 0.269, 0.216, 0.113, 0.036);
            for (var k = 0; k < 7; k = k + 1) {
                let o = f32(k - 3);
                c3 = c3 + textureSampleLevel(quad_tex, quad_smp, clamp(uv + sy * o, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0) * (w[k] * 0.5);
                c3 = c3 + textureSampleLevel(quad_tex, quad_smp, clamp(uv + sx * o, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0) * (w[k] * 0.5);
            }
        } else {
            c3 = textureSampleLevel(quad_tex, quad_smp, uv, 0.0);
        }
        let c4 = c3 * quad.color;
        return vec4<f32>(c4.rgb * c4.a, c4.a);
    }
    if (quad.dst.z > 3.5) {
        // Sekai/Live2D/Live2DBlur: B = max(_Blur, 1); taps at (i, j) · B / _ScreenParams for
        // i, j = -B, -B + 1, … ≤ B, weighted exp2(-0.7213·|offset|²)·0.159155 (offsets in UV)
        let b = max(quad.extra.x, 1.0);
        let step = vec2<f32>(b) / quad.dst.xy;
        var acc = vec4<f32>(0.0);
        var wsum = 0.0;
        var x = -b;
        loop {
            if (x > b) { break; }
            var y = -b;
            loop {
                if (y > b) { break; }
                let o = step * vec2<f32>(x, y);
                // exp2(x) with |x| < 2e-4 here: 1 + x·ln2 is exact to f32 (rule R-2)
                let w = (1.0 + dot(o, o) * -0.721347511 * 0.693147181) * 0.159154981;
                acc = acc + textureSampleLevel(quad_tex, quad_smp, i.uv + o, 0.0) * w;
                wsum = wsum + w;
                y = y + 1.0;
            }
            x = x + 1.0;
        }
        let c2 = acc / wsum * quad.color;
        return vec4<f32>(c2.rgb * c2.a, c2.a);
    }
    if (quad.dst.z > 2.5) {
        // Sekai/UI/UIGaussianBlur: 7 taps down the column and 7 along the row, 3 each side,
        // `_SamplingDistance` texels apart; each line weighted (0.036 0.113 0.216 0.269 …)
        // and the two lines averaged. Blend SrcAlpha / OneMinusSrcAlpha → premultiplied here.
        let texel = 1.0 / vec2<f32>(textureDimensions(quad_tex));
        let sy = vec2<f32>(0.0, texel.y * quad.extra.x);
        let sx = vec2<f32>(texel.x * quad.extra.x, 0.0);
        let w = array<f32, 7>(0.036, 0.113, 0.216, 0.269, 0.216, 0.113, 0.036);
        var acc = vec4<f32>(0.0);
        for (var k = 0; k < 7; k = k + 1) {
            let o = f32(k - 3);
            acc = acc + textureSampleLevel(quad_tex, quad_smp, i.uv + sy * o, 0.0) * quad.color * (w[k] * 0.5);
            acc = acc + textureSampleLevel(quad_tex, quad_smp, i.uv + sx * o, 0.0) * quad.color * (w[k] * 0.5);
        }
        return vec4<f32>(acc.rgb * acc.a, acc.a);
    }
    let t = textureSample(quad_tex, quad_smp, i.uv);
    let c = t * quad.color;
    if (quad.dst.z > 1.5) {
        // Sekai/Live2D/Live2DHologram (Live2D/Materials/Live2DHologram), blend SrcAlpha /
        // OneMinusSrcAlpha: returned premultiplied for this pipeline's One / OneMinusSrcAlpha.
        let lum = dot(c.rgb, vec3<f32>(0.2, 0.45, 0.35)) * c.a;
        let inf = 0.6;
        var rgb = lum * vec3<f32>(0.9, 1.2, 1.15) * inf + c.rgb * (1.0 - inf);
        let scan = quad.extra.z;
        if (quad.extra.x >= scan) {
            rgb = rgb + vec3<f32>(scan * 0.07);
        }
        let a = c.a * quad.extra.y;
        return vec4<f32>(rgb * a, a);
    }
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

// Blitter.BlitCameraTexture(..., bilinear: false): nearest texel of the source.
@fragment
fn point_fs(i: QuadOut) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(post_tex));
    let p = vec2<i32>(clamp(floor(i.uv * dims), vec2<f32>(0.0), dims - 1.0));
    return textureLoad(post_tex, p, 0);
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

// ---------------------------------------------------------------- particles

struct ParticleOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn particle_vs(@location(0) px: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> ParticleOut {
    var o: ParticleOut;
    o.pos = vec4<f32>(px.x / quad.dst.x * 2.0 - 1.0, 1.0 - px.y / quad.dst.y * 2.0, 0.0, 1.0);
    o.uv = uv;
    o.color = color;
    return o;
}

// `Sekai/Particles/{Additive,AlphaBlended}`: `SV_Target0 = tex × COLOR0`; the blend state
// does the rest.
// `SpriteMask` interaction: the mask writes stencil where its sprite passes the alpha test
// (`clip(a - _Cutoff)`); `VisibleInsideMask` draws only there, `VisibleOutsideMask` only
// elsewhere.
@fragment
fn particle_fs(i: ParticleOut) -> @location(0) vec4<f32> {
    let c = textureSample(quad_tex, quad_smp, i.uv) * i.color;
    if (quad.mask1.z > 0.5) {
        let d = i.pos.xy - quad.mask0.xy;
        let st = vec2<f32>(dot(quad.mask0.zw, d), dot(quad.mask1.xy, d));
        var inside = false;
        if (all(st >= vec2<f32>(0.0)) && all(st <= vec2<f32>(1.0))) {
            let uv = vec2<f32>(mix(quad.mask2.x, quad.mask2.z, st.x), mix(quad.mask2.w, quad.mask2.y, st.y));
            inside = textureSampleLevel(quad_mask, quad_smp, uv, 0.0).a - quad.mask1.w >= 0.0;
        }
        if (inside != (quad.mask1.z < 1.5)) {
            discard;
        }
    }
    return c;
}
