struct U {
    vp: mat4x4<f32>,
    light: vec4<f32>,   // direction toward the light, model space
    eye: vec4<f32>,     // eye position relative to the batch origin
    line: vec4<f32>,    // colour of lines
};
@group(0) @binding(0) var<uniform> u: U;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) nrm: vec3<f32>,
    @location(2) col: vec4<f32>,   // rgb, and the boost of top faces
};
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

@fragment
fn fs_shaded(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.n);
    let h = max(dot(n, normalize(u.light.xyz)), 0.0);
    let k = min(0.45 + 0.55 * h + 0.05 * i.col.w, 1.0);
    return vec4<f32>(i.col.rgb * k, 1.0);
}

// Hidden Line: faces are paper white and only hide what is behind them.
@fragment
fn fs_paper(i: VOut) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

@vertex
fn vs_line(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return u.vp * vec4<f32>(pos, 1.0);
}

@fragment
fn fs_line() -> @location(0) vec4<f32> {
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
