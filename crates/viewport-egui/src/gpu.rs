//! The wgpu side: pipelines for the visual styles, buffers built from a `Batch`, and an offscreen target that the
//! widget (or a test) draws into.

use bytemuck::{Pod, Zeroable};
use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use openbim_view::{Batch, Element, FaceKind};
use openbim_viewport::{base_colour, crease_edges, intersection_edges, Camera, SectionBox, Style, Sun};
use std::collections::HashSet;

pub const WGSL: &str = include_str!("shader.wgsl");
pub const COLOUR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Side of the shadow map in texels (the shader has the same number).
pub const SHADOW_SIZE: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex { pos: [f32; 3], nrm: [f32; 3], col: [f32; 4] }

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms { vp: [[f32; 4]; 4], light: [f32; 4], eye: [f32; 4], line: [f32; 4], sec_min: [f32; 4], sec_max: [f32; 4], sun: [f32; 4], light_vp: [[f32; 4]; 4] }

/// The sun's orthographic view of the scene as a column-major matrix: x and y fill -1..1 over the scene's bounding
/// sphere, depth runs 0..1 along the rays.
pub fn sun_matrix(scene: &Scene) -> [[f32; 4]; 4] {
    let f = scene.sun.direction().map(|x| -x); // the way the light travels
    let r = { let c = [f[1], -f[0], 0.0]; let n = (c[0] * c[0] + c[1] * c[1]).sqrt(); [c[0] / n, c[1] / n, 0.0] };
    let u = [r[1] * f[2] - r[2] * f[1], r[2] * f[0] - r[0] * f[2], r[0] * f[1] - r[1] * f[0]];
    let rad = scene.radius.max(1e-3) * 1.02;
    let c = scene.centre.map(f64::from);
    let eye = [c[0] - f[0] * rad * 2.0, c[1] - f[1] * rad * 2.0, c[2] - f[2] * rad * 2.0];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (near, far) = (0.0, rad * 4.0);
    // rows: x' = r.(p-eye)/rad, y' = u.(p-eye)/rad, z' = (f.(p-eye)-near)/(far-near)
    let rows = [
        [r[0] / rad, r[1] / rad, r[2] / rad, -dot(r, eye) / rad],
        [u[0] / rad, u[1] / rad, u[2] / rad, -dot(u, eye) / rad],
        [f[0] / (far - near), f[1] / (far - near), f[2] / (far - near), (-dot(f, eye) - near) / (far - near)],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let mut m = [[0.0f32; 4]; 4];
    for (rr, row) in rows.iter().enumerate() { for (cc, v) in row.iter().enumerate() { m[cc][rr] = *v as f32; } }
    m
}

/// A batch uploaded for one style.
pub struct Scene {
    fills: wgpu::Buffer,
    fill_count: u32,
    lines: wgpu::Buffer,
    line_count: u32,
    selected: Option<(wgpu::Buffer, u32)>,
    /// triangles' corner data kept for building the selection
    pub style: Style,
    /// model position of the zero of the buffers
    pub origin: [f64; 3],
    /// half the diagonal of the box around the batch
    pub radius: f64,
    /// middle of the box, relative to `origin`
    pub centre: [f32; 3],
    /// where the sun is, for Realistic
    pub sun: Sun,
}

pub struct Gpu {
    uniform_layout: wgpu::BindGroupLayout,
    shadow_pipeline: wgpu::RenderPipeline,
    shadow_view: wgpu::TextureView,
    shadow_bind: wgpu::BindGroup,
    shaded: wgpu::RenderPipeline,
    real: wgpu::RenderPipeline,
    select: wgpu::RenderPipeline,
    paper: wgpu::RenderPipeline,
    lines_hidden: wgpu::RenderPipeline,
    lines_through: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

/// A colour and a depth texture to draw the 3D picture into, and the bind group to blit it.
pub struct Target {
    pub size: [u32; 2],
    pub colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    depth_view: wgpu::TextureView,
    blit_bind: wgpu::BindGroup,
}

impl Gpu {
    /// `target_format` is the format of the surface the blit will draw into.
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Gpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("openbim viewport"), source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport uniforms layout"),
            entries: &[wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("viewport"), bind_group_layouts: &[Some(&layout)], immediate_size: 0 });

        // the sun's depth map, and how the Realistic pipeline reads it
        let shadow_tex = device.create_texture(&wgpu::TextureDescriptor { label: Some("shadow map"), size: wgpu::Extent3d { width: SHADOW_SIZE, height: SHADOW_SIZE, depth_or_array_layers: 1 }, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: DEPTH_FORMAT, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, view_formats: &[] });
        let shadow_view = shadow_tex.create_view(&Default::default());
        let shadow_samp = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, compare: Some(wgpu::CompareFunction::LessEqual), ..Default::default() });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Depth, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison), count: None },
            ],
        });
        let shadow_bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("shadow"), layout: &shadow_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&shadow_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&shadow_samp) }] });
        let real_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("viewport realistic"), bind_group_layouts: &[Some(&layout), Some(&shadow_layout)], immediate_size: 0 });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow caster"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_shadow"), compilation_options: Default::default(), buffers: &[Some(wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &wgpu::vertex_attr_array![0 => Float32x3] })] },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_shadow"), compilation_options: Default::default(), targets: &[] }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(true), depth_compare: Some(wgpu::CompareFunction::Less), stencil: Default::default(), bias: wgpu::DepthBiasState { constant: 2, slope_scale: 2.0, clamp: 0.0 } }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let fill_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let fill_buf = wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &fill_attrs };
        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3];
        let line_buf = wgpu::VertexBufferLayout { array_stride: 12, step_mode: wgpu::VertexStepMode::Vertex, attributes: &line_attrs };
        let colour_target = [Some(wgpu::ColorTargetState { format: COLOUR_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })];

        let fill = |label: &str, fs: &str, bias: i32, pl: &wgpu::PipelineLayout| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_fill"), compilation_options: Default::default(), buffers: &[Some(fill_buf.clone())] },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(fs), compilation_options: Default::default(), targets: &colour_target }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, cull_mode: Some(wgpu::Face::Back), front_face: wgpu::FrontFace::Ccw, ..Default::default() },
            // pushed back a little so that the lines on the faces win the depth test
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(bias >= 0), depth_compare: Some(if bias < 0 { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Less }), stencil: Default::default(), bias: wgpu::DepthBiasState { constant: bias, slope_scale: if bias > 0 { 1.5 } else { -1.5 }, clamp: 0.0 } }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let lines = |label: &str, test: bool| device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_line"), compilation_options: Default::default(), buffers: &[Some(line_buf.clone())] },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_line"), compilation_options: Default::default(), targets: &colour_target }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::LineList, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState { format: DEPTH_FORMAT, depth_write_enabled: Some(false), depth_compare: Some(if test { wgpu::CompareFunction::LessEqual } else { wgpu::CompareFunction::Always }), stencil: Default::default(), bias: Default::default() }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });

        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("viewport blit layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
                wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            ],
        });
        let bpl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("viewport blit"), bind_group_layouts: &[Some(&blit_layout)], immediate_size: 0 });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("viewport blit"),
            layout: Some(&bpl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_blit"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_blit"), compilation_options: Default::default(), targets: &[Some(wgpu::ColorTargetState { format: target_format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear, ..Default::default() });

        Gpu {
            uniform_layout: layout, shaded: fill("shaded", "fs_shaded", 2, &pl), real: fill("realistic", "fs_real", 2, &real_pl), select: fill("select", "fs_select", -2, &pl), paper: fill("paper", "fs_paper", 2, &pl), shadow_pipeline, shadow_view, shadow_bind,
            lines_hidden: lines("lines hidden", true), lines_through: lines("lines through", false), blit, blit_layout, sampler,
        }
    }

    /// Put a batch on the GPU for a style.
    pub fn upload(&self, device: &wgpu::Device, b: &Batch, style: Style) -> Scene {
        let mut fills: Vec<Vertex> = Vec::new();
        if style != Style::Wireframe {
            fills.reserve(b.tris.len() * 3);
            for (ti, t) in b.tris.iter().enumerate() {
                let p = t.map(|i| b.verts[i as usize]);
                let (e1, e2) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if l == 0.0 { continue; }
                let n = n.map(|x| x / l);
                let c = base_colour(b, ti, style);
                let boost = if b.tri_face[ti] == FaceKind::Top { 1.0 } else { 0.0 };
                for v in p { fills.push(Vertex { pos: v, nrm: n, col: [c[0] as f32, c[1] as f32, c[2] as f32, boost] }); }
            }
        }
        let mut lines: Vec<[f32; 3]> = Vec::new();
        if matches!(style, Style::HiddenLine | Style::Wireframe) {
            for e in crease_edges(b, 25.0).into_iter().chain(intersection_edges(b)) {
                for p in e { lines.push([p[0] as f32, p[1] as f32, p[2] as f32]); }
            }
        }
        let mk = |label: &str, bytes: &[u8], usage| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents: if bytes.is_empty() { &[0u8; 16] } else { bytes }, usage });
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for v in &b.verts { for k in 0..3 { lo[k] = lo[k].min(v[k]); hi[k] = hi[k].max(v[k]); } }
        let radius = if b.verts.is_empty() { 1.0 } else { (0..3).map(|k| ((hi[k] - lo[k]) as f64).powi(2)).sum::<f64>().sqrt() / 2.0 };
        Scene {
            fills: mk("fills", bytemuck::cast_slice(&fills), wgpu::BufferUsages::VERTEX),
            fill_count: fills.len() as u32,
            lines: mk("lines", bytemuck::cast_slice(&lines), wgpu::BufferUsages::VERTEX),
            line_count: lines.len() as u32,
            selected: None,
            style, origin: b.origin, radius, sun: Sun::default(), centre: if b.verts.is_empty() { [0.0; 3] } else { [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0] },
        }
    }

    /// Mark the elements to draw as selected (an empty set clears it).
    pub fn select(&self, device: &wgpu::Device, scene: &mut Scene, b: &Batch, elements: &HashSet<Element>) {
        let mut vs: Vec<Vertex> = Vec::new();
        if !elements.is_empty() {
            for (ti, t) in b.tris.iter().enumerate() {
                if !elements.contains(&b.elements[b.tri_element[ti] as usize]) { continue; }
                let p = t.map(|i| b.verts[i as usize]);
                let (e1, e2) = ([p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]], [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]]);
                let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                if l == 0.0 { continue; }
                for v in p { vs.push(Vertex { pos: v, nrm: n.map(|x| x / l), col: [1.0; 4] }); }
            }
        }
        scene.selected = (!vs.is_empty()).then(|| (device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("selection"), contents: bytemuck::cast_slice(&vs), usage: wgpu::BufferUsages::VERTEX }), vs.len() as u32));
    }

    pub fn target(&self, device: &wgpu::Device, size: [u32; 2]) -> Target {
        let size = [size[0].max(1), size[1].max(1)];
        let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let colour = device.create_texture(&wgpu::TextureDescriptor { label: Some("viewport colour"), size: extent, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: COLOUR_FORMAT, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC, view_formats: &[] });
        let depth = device.create_texture(&wgpu::TextureDescriptor { label: Some("viewport depth"), size: extent, mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2, format: DEPTH_FORMAT, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] });
        let colour_view = colour.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let blit_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viewport blit"), layout: &self.blit_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&colour_view) }, wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) }],
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor { label: Some("viewport uniforms"), size: std::mem::size_of::<Uniforms>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("viewport uniforms"), layout: &self.uniform_layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }] });
        Target { size, colour, colour_view, uniforms, bind, depth_view, blit_bind }
    }

    /// Draw the scene as seen by `cam` into the target.
    pub fn draw(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, target: &Target, scene: &Scene, cam: &Camera, section: Option<SectionBox>) {
        let aspect = target.size[0] as f64 / target.size[1] as f64;
        let (near, far) = cam.clip_range(scene.radius);
        let (r, u, f) = (cam.right(), cam.up(), cam.forward());
        // a light fixed to the camera, from up-left of the eye
        let l = [-0.35, 0.45, 0.8];
        let l = [r[0] * l[0] + u[0] * l[1] - f[0] * l[2], r[1] * l[0] + u[1] * l[1] - f[1] * l[2], r[2] * l[0] + u[2] * l[1] - f[2] * l[2]];
        let eye = cam.eye();
        let uni = Uniforms {
            vp: cam.view_proj(aspect, near, far, scene.origin),
            light: [l[0] as f32, l[1] as f32, l[2] as f32, 0.0],
            eye: [(eye[0] - scene.origin[0]) as f32, (eye[1] - scene.origin[1]) as f32, (eye[2] - scene.origin[2]) as f32, 0.0],
            line: [20.0 / 255.0, 20.0 / 255.0, 20.0 / 255.0, 1.0],
            sec_min: section.map_or([0.0; 4], |s| [(s.min[0] - scene.origin[0]) as f32, (s.min[1] - scene.origin[1]) as f32, (s.min[2] - scene.origin[2]) as f32, 1.0]),
            sec_max: section.map_or([0.0; 4], |s| [(s.max[0] - scene.origin[0]) as f32, (s.max[1] - scene.origin[1]) as f32, (s.max[2] - scene.origin[2]) as f32, 1.0]),
            // the sun: from the south-west, high
            sun: { let d = scene.sun.direction(); [d[0] as f32, d[1] as f32, d[2] as f32, scene.sun.strength() as f32] },
            light_vp: sun_matrix(scene),
        };
        queue.write_buffer(&target.uniforms, 0, bytemuck::bytes_of(&uni));
        if scene.style == Style::Realistic && scene.fill_count > 0 && scene.sun.strength() > 0.0 {
            let mut sp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.shadow_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
                timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
            });
            sp.set_pipeline(&self.shadow_pipeline);
            sp.set_bind_group(0, &target.bind, &[]);
            sp.set_vertex_buffer(0, scene.fills.slice(..));
            sp.draw(0..scene.fill_count, 0..1);
        }
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("viewport"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &target.colour_view, depth_slice: None, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store } })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &target.depth_view, depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }), stencil_ops: None }),
            timestamp_writes: None, occlusion_query_set: None, multiview_mask: None,
        });
        pass.set_bind_group(0, &target.bind, &[]);
        if scene.style == Style::Realistic { pass.set_bind_group(1, &self.shadow_bind, &[]); }
        if scene.fill_count > 0 {
            pass.set_pipeline(match scene.style { Style::HiddenLine => &self.paper, Style::Realistic => &self.real, _ => &self.shaded });
            pass.set_vertex_buffer(0, scene.fills.slice(..));
            pass.draw(0..scene.fill_count, 0..1);
        }
        if let Some((buf, n)) = &scene.selected {
            pass.set_pipeline(&self.select);
            pass.set_vertex_buffer(0, buf.slice(..));
            pass.draw(0..*n, 0..1);
        }
        if scene.line_count > 0 {
            pass.set_pipeline(if scene.style == Style::Wireframe { &self.lines_through } else { &self.lines_hidden });
            pass.set_vertex_buffer(0, scene.lines.slice(..));
            pass.draw(0..scene.line_count, 0..1);
        }
    }

    /// Copy the finished picture of the target into the surface pass that egui is drawing.
    pub fn blit(&self, pass: &mut wgpu::RenderPass<'_>, target: &Target) {
        pass.set_pipeline(&self.blit);
        pass.set_bind_group(0, &target.blit_bind, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Draw the scene without a window and read the picture back, RGBA row by row from the top. For tests and for
/// machines without a display.
pub fn render_rgba(device: &wgpu::Device, queue: &wgpu::Queue, gpu: &Gpu, scene: &Scene, cam: &Camera, size: [u32; 2]) -> Vec<u8> {
    render_rgba_cut(device, queue, gpu, scene, cam, size, None)
}

/// As `render_rgba`, with a section box.
pub fn render_rgba_cut(device: &wgpu::Device, queue: &wgpu::Queue, gpu: &Gpu, scene: &Scene, cam: &Camera, size: [u32; 2], section: Option<SectionBox>) -> Vec<u8> {
    let target = gpu.target(device, size);
    let mut enc = device.create_command_encoder(&Default::default());
    gpu.draw(queue, &mut enc, &target, scene, cam, section);
    let row = (size[0] * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size: (row * size[1]) as u64, usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target.colour, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(size[1]) } },
        wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |r| r.expect("map readback"));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    let data = slice.get_mapped_range().expect("mapped range");
    let mut out = Vec::with_capacity((size[0] * size[1] * 4) as usize);
    for y in 0..size[1] as usize { out.extend_from_slice(&data[y * row as usize..y * row as usize + size[0] as usize * 4]); }
    out
}
