struct U {
    vp: mat4x4<f32>,
    light: vec4<f32>,   // direction toward the light, model space
    eye: vec4<f32>,     // eye position relative to the batch origin
    line: vec4<f32>,    // colour of lines
    sec_min: vec4<f32>, // section box, relative to the batch origin; w is 1 when it is on
    sec_max: vec4<f32>,
    sun: vec4<f32>,     // direction toward the sun, model space
    light_vp: mat4x4<f32>, // the sun's view of the scene, for shadows
};
@group(0) @binding(0) var<uniform> u: U;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,   // rgb, and the boost of top faces
};
struct LOut { @builtin(position) p: vec4<f32>, @location(0) w: vec3<f32> };
struct VOut {
    @builtin(position) p: vec4<f32>,
    @location(0) w: vec3<f32>,
    @location(1) n: vec3<f32>,
    @location(2) col: vec4<f32>,
};

@vertex
fn vs_fill(v: VIn) -> VOut {
    var o: VOut;
    o.p = u.vp * vec4<f32>(v.pos, 1.0);
    o.w = v.pos;
    o.n = v.nrm;
    o.col = v.col;
    return o;
}

fn cut(w: vec3<f32>) -> bool {
    return u.sec_min.w > 0.5 && (any(w < u.sec_min.xyz) || any(w > u.sec_max.xyz));
}

@fragment
fn fs_shaded(i: VOut) -> @location(0) vec4<f32> {
    if cut(i.w) { discard; }
    let n = normalize(i.n);
    let h = max(dot(n, normalize(u.light.xyz)), 0.0);
    let k = min(0.45 + 0.55 * h + 0.05 * i.col.w, 1.0);
    return vec4<f32>(i.col.rgb * k, 1.0);
}

// Hidden Line: faces are paper white and only hide what is behind them.
@fragment
fn fs_paper(i: VOut) -> @location(0) vec4<f32> {
    if cut(i.w) { discard; }
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

@group(1) @binding(0) var shadow_map: texture_depth_2d;
@group(1) @binding(1) var shadow_samp: sampler_comparison;

// 1 where the sun reaches the point, 0 in shadow; 3 x 3 samples of the shadow map.
fn lit(w: vec3<f32>, n: vec3<f32>) -> f32 {
    let lp = u.light_vp * vec4<f32>(w + n * 0.04, 1.0);
    let uv = vec2<f32>(lp.x * 0.5 + 0.5, -lp.y * 0.5 + 0.5);
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || lp.z > 1.0 { return 1.0; }
    let texel = 1.0 / 2048.0;
    var sum = 0.0;
    for (var dx = -1; dx <= 1; dx = dx + 1) {
        for (var dy = -1; dy <= 1; dy = dy + 1) {
            sum = sum + textureSampleCompareLevel(shadow_map, shadow_samp, uv + vec2<f32>(f32(dx), f32(dy)) * texel, lp.z - 0.0008);
        }
    }
    return sum / 9.0;
}

// Depth of the casters, seen from the sun.
@vertex
fn vs_shadow(@location(0) pos: vec3<f32>) -> LOut {
    var o: LOut;
    o.p = u.light_vp * vec4<f32>(pos, 1.0);
    o.w = pos;
    return o;
}
@fragment
fn fs_shadow(i: LOut) {
    if cut(i.w) { discard; }
}

// Realistic: a sun, sky light from above and ground light from below, and a highlight. No shadows yet.
@fragment
fn fs_real(i: VOut) -> @location(0) vec4<f32> {
    if cut(i.w) { discard; }
    var n = normalize(i.n);
    let v = normalize(u.eye.xyz - i.w);
    if dot(n, v) < 0.0 { n = -n; }
    let sky = vec3<f32>(0.60, 0.70, 0.88);
    let ground = vec3<f32>(0.36, 0.33, 0.30);
    let amb = mix(ground, sky, n.z * 0.5 + 0.5) * 0.7;
    let sd = normalize(u.sun.xyz);
    let diff = max(dot(n, sd), 0.0) * lit(i.w, n);
    let hv = normalize(sd + v);
    let spec = pow(max(dot(n, hv), 0.0), 48.0) * 0.18 * step(0.0, diff);
    let c = i.col.rgb * (amb + vec3<f32>(1.0, 0.95, 0.86) * diff * 0.75) + vec3<f32>(spec);
    return vec4<f32>(min(c, vec3<f32>(1.0)), 1.0);
}

// The selected elements, drawn over their own faces.
@fragment
fn fs_select(i: VOut) -> @location(0) vec4<f32> {
    if cut(i.w) { discard; }
    let n = normalize(i.n);
    let h = max(dot(n, normalize(u.light.xyz)), 0.0);
    return vec4<f32>(vec3<f32>(1.0, 0.52, 0.10) * (0.65 + 0.35 * h), 1.0);
}

@vertex
fn vs_line(@location(0) pos: vec3<f32>) -> LOut {
    var o: LOut;
    o.p = u.vp * vec4<f32>(pos, 1.0);
    o.w = pos;
    return o;
}

@fragment
fn fs_line(i: LOut) -> @location(0) vec4<f32> {
    if cut(i.w) { discard; }
    return u.line;
}

// Blit of the finished picture into the rectangle of the widget.
struct BOut { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_blit(@builtin(vertex_index) i: u32) -> BOut {
    var o: BOut;
    let xy = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.p = vec4<f32>(xy * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(xy.x, 1.0 - xy.y);
    return o;
}
@group(0) @binding(0) var pic: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@fragment
fn fs_blit(i: BOut) -> @location(0) vec4<f32> {
    return textureSample(pic, samp, i.uv);
}
