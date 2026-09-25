//! wgpu backend of `sse-render`. Only immutable resources are cached across frames; all
//! per-frame state arrives in a [`FramePlan`].

use std::collections::BTreeMap;

use bytemuck::{Pod, Zeroable};
use sse_assets::Library;
use sse_core::consts;

#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("no GPU adapter: {0}")]
    Adapter(String),
    #[error("device: {0}")]
    Device(String),
    #[error("readback: {0}")]
    Readback(String),
    #[error("model {0}: {1}")]
    Model(String, String),
}

const FMT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const MASK_FMT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
/// Masks are rendered at half the character RT resolution.
const MASK_DIV: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ImageId(pub usize);

pub struct Image {
    pub id: ImageId,
}

/// A screen-space textured (or solid) quad in target pixels.
/// `scenarioRoot` on screen in target pixels: uniform scale `zoom` about `center`, then
/// `offset` (the root's shake minus the studio camera's position).
#[derive(Debug, Clone, Copy)]
pub struct ScreenXf {
    pub center: [f32; 2],
    pub zoom: f32,
    pub offset: [f32; 2],
}

impl ScreenXf {
    pub fn point(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.center[0] + self.zoom * (p[0] - self.center[0]) + self.offset[0],
            self.center[1] + self.zoom * (p[1] - self.center[1]) + self.offset[1],
        ]
    }

    pub fn rect(&self, r: [f32; 4]) -> [f32; 4] {
        let [x, y] = self.point([r[0], r[1]]);
        [x, y, r[2] * self.zoom, r[3] * self.zoom]
    }

    pub fn is_identity(&self) -> bool {
        self.zoom == 1.0 && self.offset == [0.0, 0.0]
    }
}

pub struct QuadDraw {
    image: Option<ImageId>,
    rect: [f32; 4],
    /// u0, v0, u1, v1 (top-down image rows); swapped ends mirror.
    uv: [f32; 4],
    color: [f32; 4],
    premultiplied: bool,
    /// `Sekai/UI/UIGaussianBlur` with this `_SamplingDistance` (texels).
    blur: Option<f32>,
    /// `Sekai/UI/UIDollyZoomEffect` with `[_SamplingDistance, _DistortionStrength]`.
    dolly: Option<[f32; 2]>,
}

impl QuadDraw {
    /// Draws with the `UIGaussianBlur` material.
    pub fn with_blur(mut self, sampling_distance: f32) -> Self {
        self.blur = Some(sampling_distance);
        self
    }

    /// Draws with the `UIDollyZoomEffect` material.
    pub fn with_dolly(mut self, sampling_distance: f32, distortion: f32) -> Self {
        self.dolly = Some([sampling_distance, distortion]);
        self
    }

    /// Applies a [`ScreenXf`] to the rect.
    pub fn transform(&mut self, xf: &ScreenXf) {
        self.rect = xf.rect(self.rect);
    }

    pub fn image(id: ImageId, rect: [f32; 4], color: [f32; 4]) -> Self {
        Self {
            image: Some(id),
            rect,
            uv: [0.0, 0.0, 1.0, 1.0],
            color,
            premultiplied: false,
            blur: None,
            dolly: None,
        }
    }
    pub fn image_uv(id: ImageId, rect: [f32; 4], uv: [f32; 4], color: [f32; 4]) -> Self {
        Self {
            image: Some(id),
            rect,
            uv,
            color,
            premultiplied: false,
            blur: None,
            dolly: None,
        }
    }
    pub fn solid(rect: [f32; 4], color: [f32; 4]) -> Self {
        Self {
            image: None,
            rect,
            uv: [0.0, 0.0, 1.0, 1.0],
            color,
            premultiplied: false,
            blur: None,
            dolly: None,
        }
    }
}

/// One `fx_transition_scenario` billboard in target pixels.
pub struct ParticleDraw {
    /// `Sekai/Particles/Additive` (true) or `AlphaBlended`.
    pub additive: bool,
    pub image: ImageId,
    /// Corners in order (0, 1, 2, 3) around the quad, with their UVs.
    pub corners: [[f32; 2]; 4],
    pub uvs: [[f32; 2]; 4],
    /// Vertex colour, straight alpha (`startColor × colourOverLifetime`).
    pub color: [f32; 4],
}

pub struct CharacterDraw {
    pub model: usize,
    pub params: Vec<f32>,
    pub opacity: f32,
    pub color: [f32; 4],
    /// Composite rectangle of the RT in target pixels.
    pub rect: [f32; 4],
    /// `Live2DHologram` material on the `RawImage` instead of `UI/Default`.
    pub hologram: Option<Hologram>,
    /// `Live2DBlur` material with this `_Blur`.
    pub blur: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
pub struct Hologram {
    /// `_Line`
    pub line: f32,
    /// `_SubColor.a`
    pub alpha: f32,
    /// `_SubTex.r` this frame
    pub scan: f32,
}

#[derive(Default)]
pub struct FramePlan {
    /// Drawn into the scene before characters (background).
    pub scene: Vec<QuadDraw>,
    pub characters: Vec<CharacterDraw>,
    pub blur: f32,
    pub camera_color: Option<sse_params::CameraColor>,
    /// `EffectLayer` particles: canvas sorting order 245, between `ForegroundLayer` (240)
    /// and the `UILayer` (280) that holds the talk window and `FrontCover` fader.
    pub particles: Vec<ParticleDraw>,
    /// Scenario UI (talk window, menu button): after post effects, under the fader.
    pub ui: Vec<QuadDraw>,
    /// `FrontCover` `ColorFader`: above the scenario UI.
    pub overlay: Vec<QuadDraw>,
    /// Dialog layer (full-screen text): above the fader.
    pub ui_top: Vec<QuadDraw>,
    pub text: Option<ImageId>,
    /// `SideFadePlayer`: last sibling of `UILayer`, drawn over the talk window and its text.
    pub cover: Vec<QuadDraw>,
    /// Scenario effect prefabs below the characters' canvas (sorting order < 220) and above
    /// it; both are drawn by the scenario camera, so before its post effects.
    pub effects_back: Vec<ParticleDraw>,
    pub effects_front: Vec<ParticleDraw>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct DrawGpu {
    xform: [f32; 4],
    half_extent: [f32; 4],
    tint: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct QuadGpu {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    target: [f32; 4],
    /// mode 2 (hologram): `_Line`, `_SubColor.a`, `_SubTex.r`
    extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PostGpu {
    step: [f32; 4],
    mono: [f32; 4],
    tone: [f32; 4],
    influence: [f32; 4],
}

struct Tex {
    _tex: wgpu::Texture,
    view: wgpu::TextureView,
}

pub struct GpuModel {
    core: sse_live2d::Model,
    textures: Vec<Tex>,
    positions: wgpu::Buffer,
    uvs: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: wgpu::Buffer,
    /// Per drawable: (first vertex, first index, index count).
    ranges: Vec<(u32, u32, u32)>,
    vertex_count: usize,
    /// Distinct mask sets → mask slot.
    mask_sets: BTreeMap<Vec<usize>, usize>,
    /// (texture, mask slot or usize::MAX) → bind group
    bind_groups: BTreeMap<(usize, usize), wgpu::BindGroup>,
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    width: u32,
    height: u32,
    sampler: wgpu::Sampler,
    cubism_layout: wgpu::BindGroupLayout,
    quad_layout: wgpu::BindGroupLayout,
    cubism_pipes: [wgpu::RenderPipeline; 3],
    mask_pipe: wgpu::RenderPipeline,
    quad_pipe: wgpu::RenderPipeline,
    blur_pipe: wgpu::RenderPipeline,
    point_pipe: wgpu::RenderPipeline,
    mono_pipe: wgpu::RenderPipeline,
    /// Particle billboards: [additive, alpha-blended].
    particle_pipes: [wgpu::RenderPipeline; 2],
    rt: Tex,
    masks: Vec<Tex>,
    dummy_mask: Tex,
    scene: Tex,
    /// `RenderBlur` temporaries at `1 / DownSample` resolution.
    half: [Tex; 2],
    output: wgpu::Texture,
    output_view: wgpu::TextureView,
    readback: wgpu::Buffer,
    padded_row: u32,
    images: Vec<Tex>,
    white: ImageId,
    text: ImageId,
    uniforms: Vec<wgpu::Buffer>,
}

fn tex(
    device: &wgpu::Device,
    w: u32,
    h: u32,
    format: wgpu::TextureFormat,
    extra: wgpu::TextureUsages,
) -> Tex {
    let t = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST | extra,
        view_formats: &[],
    });
    let view = t.create_view(&wgpu::TextureViewDescriptor::default());
    Tex { _tex: t, view }
}

fn blend(
    src_c: wgpu::BlendFactor,
    dst_c: wgpu::BlendFactor,
    src_a: wgpu::BlendFactor,
    dst_a: wgpu::BlendFactor,
) -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: src_c,
            dst_factor: dst_c,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: src_a,
            dst_factor: dst_a,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

impl Gpu {
    pub fn new(width: u32, height: u32) -> Result<Self, GpuError> {
        use wgpu::BlendFactor as F;
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            ..Default::default()
        }))
        .map_err(|e| GpuError::Adapter(e.to_string()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| GpuError::Device(e.to_string()))?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sse"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders.wgsl").into()),
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let tex_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let smp_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let cubism_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                smp_entry(2),
                tex_entry(3),
            ],
        });
        let quad_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                tex_entry(1),
                smp_entry(2),
            ],
        });
        let cubism_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&cubism_layout)],
            immediate_size: 0,
        });
        let quad_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&quad_layout)],
            immediate_size: 0,
        });
        let vbufs = [
            Some(wgpu::VertexBufferLayout {
                array_stride: 8,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: 8,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 1,
                }],
            }),
        ];
        let pipe = |layout: &wgpu::PipelineLayout,
                    vs: &str,
                    fs: &str,
                    buffers: &[Option<wgpu::VertexBufferLayout>],
                    format: wgpu::TextureFormat,
                    blend: Option<wgpu::BlendState>,
                    strip: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive: wgpu::PrimitiveState {
                    topology: if strip {
                        wgpu::PrimitiveTopology::TriangleStrip
                    } else {
                        wgpu::PrimitiveTopology::TriangleList
                    },
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let normal = blend(F::One, F::OneMinusSrcAlpha, F::One, F::OneMinusSrcAlpha);
        let additive = blend(F::One, F::One, F::Zero, F::One);
        let multiply = blend(F::Dst, F::OneMinusSrcAlpha, F::Zero, F::One);
        let cubism_pipes = [
            pipe(
                &cubism_pl,
                "cubism_vs",
                "cubism_fs",
                &vbufs,
                FMT,
                Some(normal),
                false,
            ),
            pipe(
                &cubism_pl,
                "cubism_vs",
                "cubism_fs",
                &vbufs,
                FMT,
                Some(additive),
                false,
            ),
            pipe(
                &cubism_pl,
                "cubism_vs",
                "cubism_fs",
                &vbufs,
                FMT,
                Some(multiply),
                false,
            ),
        ];
        let mask_pipe = pipe(
            &cubism_pl,
            "cubism_vs",
            "mask_fs",
            &vbufs,
            MASK_FMT,
            Some(blend(F::One, F::One, F::One, F::One)),
            false,
        );
        let quad_pipe = pipe(&quad_pl, "quad_vs", "quad_fs", &[], FMT, Some(normal), true);
        let blur_pipe = pipe(&quad_pl, "post_vs", "blur_fs", &[], FMT, None, true);
        let point_pipe = pipe(&quad_pl, "post_vs", "point_fs", &[], FMT, None, true);
        let mono_pipe = pipe(&quad_pl, "post_vs", "mono_fs", &[], FMT, None, true);
        // `Sekai/Particles/Additive`: Blend SrcAlpha One; `AlphaBlended`: SrcAlpha
        // OneMinusSrcAlpha (both `ZWrite Off`, `Cull Off`; destination alpha kept).
        let pvb = [Some(wgpu::VertexBufferLayout {
            array_stride: 8 * 4,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
            ],
        })];
        let particle_pipes = [
            pipe(
                &quad_pl,
                "particle_vs",
                "particle_fs",
                &pvb,
                FMT,
                Some(blend(F::SrcAlpha, F::One, F::Zero, F::One)),
                false,
            ),
            pipe(
                &quad_pl,
                "particle_vs",
                "particle_fs",
                &pvb,
                FMT,
                Some(blend(F::SrcAlpha, F::OneMinusSrcAlpha, F::Zero, F::One)),
                false,
            ),
        ];

        let [rtw, rth] = consts::LIVE2D_RT_SIZE;
        let ra = wgpu::TextureUsages::RENDER_ATTACHMENT;
        let rt = tex(&device, rtw, rth, FMT, ra);
        let dummy_mask = tex(&device, 1, 1, MASK_FMT, ra);
        queue.write_texture(
            dummy_mask._tex.as_image_copy(),
            &[255],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let scene = tex(&device, width, height, FMT, ra);
        let (hw, hh) = (
            width / consts::BLUR_DOWN_SAMPLE,
            height / consts::BLUR_DOWN_SAMPLE,
        );
        let half = [tex(&device, hw, hh, FMT, ra), tex(&device, hw, hh, FMT, ra)];
        let output = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("output"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FMT,
            usage: ra | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let output_view = output.create_view(&Default::default());
        let padded_row = (width * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: u64::from(padded_row) * u64::from(height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut gpu = Self {
            device,
            queue,
            width,
            height,
            sampler,
            cubism_layout,
            quad_layout,
            cubism_pipes,
            mask_pipe,
            quad_pipe,
            blur_pipe,
            mono_pipe,
            particle_pipes,
            rt,
            masks: Vec::new(),
            dummy_mask,
            scene,
            half,
            point_pipe,
            output,
            output_view,
            readback,
            padded_row,
            images: Vec::new(),
            white: ImageId(0),
            text: ImageId(0),
            uniforms: Vec::new(),
        };
        gpu.white = gpu
            .image(&image::RgbaImage::from_pixel(1, 1, image::Rgba([255; 4])))
            .id;
        let t = tex(
            &gpu.device,
            width,
            height,
            FMT,
            wgpu::TextureUsages::empty(),
        );
        gpu.images.push(t);
        gpu.text = ImageId(gpu.images.len() - 1);
        Ok(gpu)
    }

    pub fn image(&mut self, img: &image::RgbaImage) -> Image {
        let (w, h) = img.dimensions();
        let t = tex(&self.device, w, h, FMT, wgpu::TextureUsages::empty());
        self.queue.write_texture(
            t._tex.as_image_copy(),
            img.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.images.push(t);
        Image {
            id: ImageId(self.images.len() - 1),
        }
    }

    pub fn text_image(&self) -> ImageId {
        self.text
    }

    /// Overwrites a full-target-size image (movie frames).
    pub fn upload_image(&mut self, id: ImageId, rgba: &[u8]) {
        self.queue.write_texture(
            self.images[id.0]._tex.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn upload_text(&mut self, rgba: &[u8]) {
        self.queue.write_texture(
            self.images[self.text.0]._tex.as_image_copy(),
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
    }

    pub fn load_model(
        &mut self,
        lib: &Library,
        bundle: &str,
    ) -> Result<GpuModel, crate::RenderError> {
        let err = |e: String| GpuError::Model(bundle.to_owned(), e);
        let m3 = lib.load_model3(bundle)?;
        let bytes = std::fs::read(&m3.moc).map_err(|e| err(e.to_string()))?;
        let moc = sse_live2d::Moc::new(&bytes).map_err(|e| err(e.to_string()))?;
        let core = sse_live2d::Model::new(moc).map_err(|e| err(e.to_string()))?;
        let mut textures = Vec::new();
        for t in &m3.textures {
            let img = sse_assets::load_png(t)?;
            let (w, h) = img.dimensions();
            let tx = tex(&self.device, w, h, FMT, wgpu::TextureUsages::empty());
            self.queue.write_texture(
                tx._tex.as_image_copy(),
                img.as_raw(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            textures.push(tx);
        }
        let mut uvs: Vec<f32> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
        let mut ranges = Vec::new();
        let mut mask_sets = BTreeMap::new();
        for d in &core.drawables {
            let first_vertex = (uvs.len() / 2) as u32;
            let first_index = indices.len() as u32;
            for uv in &d.uvs {
                uvs.extend_from_slice(uv);
            }
            indices.extend(d.indices.iter().map(|&i| u32::from(i)));
            ranges.push((first_vertex, first_index, d.indices.len() as u32));
            if !d.masks.is_empty() {
                let n = mask_sets.len();
                mask_sets.entry(d.masks.clone()).or_insert(n);
            }
        }
        while !indices.len().is_multiple_of(2) {
            indices.push(0);
        }
        use wgpu::util::DeviceExt;
        let vertex_count = uvs.len() / 2;
        let mk = |data: &[u8], usage| {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: data,
                    usage,
                })
        };
        let uv_buf = mk(bytemuck::cast_slice(&uvs), wgpu::BufferUsages::VERTEX);
        let index_buf = mk(bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX);
        let positions = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (vertex_count * 8).max(8) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let draws = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (core.drawables.len().max(1) * std::mem::size_of::<DrawGpu>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let [rtw, rth] = consts::LIVE2D_RT_SIZE;
        while self.masks.len() < mask_sets.len() {
            let t = tex(
                &self.device,
                rtw / MASK_DIV,
                rth / MASK_DIV,
                MASK_FMT,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
            self.masks.push(t);
        }
        Ok(GpuModel {
            core,
            textures,
            positions,
            uvs: uv_buf,
            indices: index_buf,
            draws,
            ranges,
            vertex_count,
            mask_sets,
            bind_groups: BTreeMap::new(),
        })
    }

    fn quad_bind(&mut self, image: ImageId, q: QuadGpu) -> wgpu::BindGroup {
        use wgpu::util::DeviceExt;
        let buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::bytes_of(&q),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.quad_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.images[image.0].view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.uniforms.push(buf);
        bg
    }

    fn view_bind(&mut self, view: &wgpu::TextureView, bytes: &[u8]) -> wgpu::BindGroup {
        use wgpu::util::DeviceExt;
        let buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytes,
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.quad_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.uniforms.push(buf);
        bg
    }

    fn quads(&mut self, draws: &[QuadDraw]) -> Vec<wgpu::BindGroup> {
        let (w, h) = (self.width as f32, self.height as f32);
        draws
            .iter()
            .map(|q| {
                let img = q.image.unwrap_or(self.white);
                self.quad_bind(
                    img,
                    QuadGpu {
                        rect: q.rect,
                        uv: q.uv,
                        color: q.color,
                        target: [
                            w,
                            h,
                            match (q.dolly, q.blur, q.premultiplied) {
                                (Some(_), _, _) => 5.0,
                                (None, Some(_), _) => 3.0,
                                (None, None, true) => 1.0,
                                (None, None, false) => 0.0,
                            },
                            0.0,
                        ],
                        extra: match (q.dolly, q.blur) {
                            (Some([sd, dist]), _) => [sd, dist, 0.0, 0.0],
                            (None, b) => [b.unwrap_or(0.0), 0.0, 0.0, 0.0],
                        },
                    },
                )
            })
            .collect()
    }

    /// Draws billboards in order, switching blend per draw (Unity renders them in sorting
    /// order; one draw per billboard keeps the additive / alpha interleaving exact).
    fn particles(&mut self, enc: &mut wgpu::CommandEncoder, draws: &[ParticleDraw]) {
        let out_view = self.output_view.clone();
        self.particles_to(enc, &out_view, draws);
    }

    fn particles_to(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        draws: &[ParticleDraw],
    ) {
        use wgpu::util::DeviceExt;
        if draws.is_empty() {
            return;
        }
        let (w, h) = (self.width as f32, self.height as f32);
        let mut verts: Vec<f32> = Vec::with_capacity(draws.len() * 6 * 8);
        for d in draws {
            for &k in &[0usize, 1, 2, 0, 2, 3] {
                verts.extend_from_slice(&[
                    d.corners[k][0],
                    d.corners[k][1],
                    d.uvs[k][0],
                    d.uvs[k][1],
                ]);
                verts.extend_from_slice(&d.color);
            }
        }
        let vbuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("particles"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let q = QuadGpu {
            rect: [0.0; 4],
            uv: [0.0; 4],
            color: [1.0; 4],
            target: [w, h, 0.0, 0.0],
            extra: [0.0; 4],
        };
        let bgs: Vec<wgpu::BindGroup> = draws.iter().map(|d| self.quad_bind(d.image, q)).collect();
        let mut rp = Self::pass(enc, target, None);
        rp.set_vertex_buffer(0, vbuf.slice(..));
        for (i, d) in draws.iter().enumerate() {
            rp.set_pipeline(&self.particle_pipes[if d.additive { 0 } else { 1 }]);
            rp.set_bind_group(0, &bgs[i], &[]);
            let v0 = (i * 6) as u32;
            rp.draw(v0..v0 + 6, 0..1);
        }
    }

    fn pass<'e>(
        enc: &'e mut wgpu::CommandEncoder,
        view: &'e wgpu::TextureView,
        clear: Option<wgpu::Color>,
    ) -> wgpu::RenderPass<'e> {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: clear.map_or(wgpu::LoadOp::Load, wgpu::LoadOp::Clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        })
    }

    /// Renders the character's drawables into the RT (one encoder per character so the
    /// shared RT can be reused).
    fn character(
        &mut self,
        models: &mut [GpuModel],
        c: &CharacterDraw,
        enc: &mut wgpu::CommandEncoder,
    ) {
        let m = &mut models[c.model];
        m.core.update(&c.params, None);
        let frames = m.core.drawable_frames();
        let order = m.core.render_order();
        let mut positions = Vec::with_capacity(m.vertex_count * 2);
        for f in &frames {
            for p in &f.positions {
                positions.extend_from_slice(p);
            }
        }
        self.queue
            .write_buffer(&m.positions, 0, bytemuck::cast_slice(&positions));
        let [rtw, rth] = consts::LIVE2D_RT_SIZE;
        // RenderStudio (landscape): ortho size 1.5, camera at the studio origin, model at
        // stageRoot (0, fixedStagePosition.y × orthoSize) + standPosition (0, 0.383),
        // standScale 2.8.
        let half_h = consts::LIVE2D_ORTHO_SIZE;
        let half_w = half_h * rtw as f32 / rth as f32;
        let draws: Vec<DrawGpu> = m
            .core
            .drawables
            .iter()
            .enumerate()
            .map(|(i, d)| DrawGpu {
                xform: [
                    2.8,
                    0.0,
                    consts::LIVE2D_FIXED_STAGE_Y * consts::LIVE2D_ORTHO_SIZE
                        + consts::LIVE2D_STAND_Y,
                    0.0,
                ],
                half_extent: [half_w, half_h, rtw as f32, rth as f32],
                tint: [1.0; 4],
                params: [
                    frames[i].opacity,
                    if d.masks.is_empty() { 0.0 } else { 1.0 },
                    if d.inverted_mask { 1.0 } else { 0.0 },
                    0.0,
                ],
            })
            .collect();
        self.queue
            .write_buffer(&m.draws, 0, bytemuck::cast_slice(&draws));

        // bind groups (texture, mask slot)
        let mut needed: Vec<(usize, usize)> = m
            .core
            .drawables
            .iter()
            .map(|d| {
                (
                    d.texture,
                    if d.masks.is_empty() {
                        usize::MAX
                    } else {
                        m.mask_sets[&d.masks]
                    },
                )
            })
            .collect();
        needed.sort();
        needed.dedup();
        for key in needed {
            if m.bind_groups.contains_key(&key) {
                continue;
            }
            let mask_view = if key.1 == usize::MAX {
                &self.dummy_mask.view
            } else {
                &self.masks[key.1].view
            };
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.cubism_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: m.draws.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(
                            &m.textures[key.0.min(m.textures.len() - 1)].view,
                        ),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(mask_view),
                    },
                ],
            });
            m.bind_groups.insert(key, bg);
        }

        // masks
        for (set, &slot) in &m.mask_sets {
            let mut rp = Self::pass(enc, &self.masks[slot].view, Some(wgpu::Color::TRANSPARENT));
            rp.set_pipeline(&self.mask_pipe);
            rp.set_vertex_buffer(0, m.positions.slice(..));
            rp.set_vertex_buffer(1, m.uvs.slice(..));
            rp.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
            for &i in set {
                let d = &m.core.drawables[i];
                let (fv, fi, n) = m.ranges[i];
                if n == 0 {
                    continue;
                }
                rp.set_bind_group(0, &m.bind_groups[&(d.texture, usize::MAX)], &[]);
                rp.draw_indexed(fi..fi + n, fv as i32, i as u32..i as u32 + 1);
            }
        }
        // drawables
        let mut rp = Self::pass(enc, &self.rt.view, Some(wgpu::Color::TRANSPARENT));
        rp.set_vertex_buffer(0, m.positions.slice(..));
        rp.set_vertex_buffer(1, m.uvs.slice(..));
        rp.set_index_buffer(m.indices.slice(..), wgpu::IndexFormat::Uint32);
        for i in order {
            let f = &frames[i];
            let d = &m.core.drawables[i];
            let (fv, fi, n) = m.ranges[i];
            if !f.visible || f.opacity <= 0.0 || n == 0 {
                continue;
            }
            let pipe = if d.additive {
                &self.cubism_pipes[1]
            } else if d.multiply {
                &self.cubism_pipes[2]
            } else {
                &self.cubism_pipes[0]
            };
            rp.set_pipeline(pipe);
            let key = (
                d.texture,
                if d.masks.is_empty() {
                    usize::MAX
                } else {
                    m.mask_sets[&d.masks]
                },
            );
            rp.set_bind_group(0, &m.bind_groups[&key], &[]);
            rp.draw_indexed(fi..fi + n, fv as i32, i as u32..i as u32 + 1);
        }
    }

    fn draw_quads(
        &self,
        enc: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        bgs: &[wgpu::BindGroup],
        clear: Option<wgpu::Color>,
    ) {
        let mut rp = Self::pass(enc, view, clear);
        rp.set_pipeline(&self.quad_pipe);
        for bg in bgs {
            rp.set_bind_group(0, bg, &[]);
            rp.draw(0..4, 0..1);
        }
    }

    pub fn render(
        &mut self,
        models: &mut [GpuModel],
        plan: &FramePlan,
    ) -> Result<Vec<u8>, GpuError> {
        self.uniforms.clear();
        let (w, h) = (self.width as f32, self.height as f32);
        let black = Some(wgpu::Color::BLACK);

        // background
        let bgs = self.quads(&plan.scene);
        let mut enc = self.device.create_command_encoder(&Default::default());
        self.draw_quads(&mut enc, &self.scene.view, &bgs, black);
        self.queue.submit([enc.finish()]);
        if !plan.effects_back.is_empty() {
            let mut enc = self.device.create_command_encoder(&Default::default());
            let scene_view = self.scene.view.clone();
            self.particles_to(&mut enc, &scene_view, &plan.effects_back);
            self.queue.submit([enc.finish()]);
        }

        // characters: RT then composite, one submission each (shared RT / position writes)
        for c in &plan.characters {
            let mut enc = self.device.create_command_encoder(&Default::default());
            self.character(models, c, &mut enc);
            let rt_view = self.rt.view.clone();
            let (mode, extra) = match (c.hologram, c.blur) {
                (Some(g), _) => (2.0, [g.line, g.alpha, g.scan, 0.0]),
                (None, Some(b)) => (4.0, [b, 0.0, 0.0, 0.0]),
                (None, None) => (0.0, [0.0; 4]),
            };
            let q = QuadGpu {
                rect: c.rect,
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [c.color[0], c.color[1], c.color[2], c.opacity],
                target: [w, h, mode, 0.0],
                extra,
            };
            let bg = self.view_bind(&rt_view, bytemuck::bytes_of(&q));
            self.draw_quads(&mut enc, &self.scene.view, &[bg], None);
            self.queue.submit([enc.finish()]);
        }
        if !plan.effects_front.is_empty() {
            let mut enc = self.device.create_command_encoder(&Default::default());
            let scene_view = self.scene.view.clone();
            self.particles_to(&mut enc, &scene_view, &plan.effects_front);
            self.queue.submit([enc.finish()]);
        }

        let mut enc = self.device.create_command_encoder(&Default::default());
        // `ScenarioPostProcessRenderPass.RenderBlur`: point-sampled blit to
        // 1/DownSample, then Iterations × (V pass, U pass) with `_BlurSize = spread·i + 1`,
        // then a point-sampled blit back. `Hidden/Sekai/Scenario/Post` offsets its taps by
        // `_BlitTexture_TexelSize × _BlurSize`; `Blitter.BlitTexture` binds the
        // temporary via `MaterialPropertyBlock.SetTexture(int, Texture)`, so the texel size is
        // the half-resolution one. The blur's on-screen size therefore depends on the render
        // resolution (ADR-0010: native = output resolution).
        if plan.blur > 0.001 {
            let spread = plan.blur * consts::BLUR_MAX_SPREAD;
            let (hw, hh) = (
                (self.width / consts::BLUR_DOWN_SAMPLE) as f32,
                (self.height / consts::BLUR_DOWN_SAMPLE) as f32,
            );
            let scene_view = self.scene.view.clone();
            let (a, b) = (self.half[0].view.clone(), self.half[1].view.clone());
            let blit = |this: &mut Self,
                        enc: &mut wgpu::CommandEncoder,
                        src: &wgpu::TextureView,
                        dst: &wgpu::TextureView,
                        pipe: bool,
                        step: [f32; 2]| {
                let p = PostGpu {
                    step: [step[0], step[1], 0.0, 0.0],
                    mono: [0.0; 4],
                    tone: [0.0; 4],
                    influence: [0.0; 4],
                };
                let bg = this.view_bind(src, bytemuck::bytes_of(&p));
                let mut rp = Self::pass(enc, dst, None);
                rp.set_pipeline(if pipe {
                    &this.blur_pipe
                } else {
                    &this.point_pipe
                });
                rp.set_bind_group(0, &bg, &[]);
                rp.draw(0..4, 0..1);
            };
            blit(self, &mut enc, &scene_view, &a, false, [0.0; 2]);
            for i in 0..consts::BLUR_ITERATIONS {
                let size = spread * i as f32 + 1.0;
                blit(self, &mut enc, &a, &b, true, [0.0, size / hh]);
                blit(self, &mut enc, &b, &a, true, [size / hw, 0.0]);
            }
            blit(self, &mut enc, &a, &scene_view, false, [0.0; 2]);
        }
        // monotone → output
        let cc = plan.camera_color;
        let p = PostGpu {
            step: [0.0; 4],
            mono: cc.map_or([0.0; 4], |c| c.mono),
            tone: cc.map_or([0.0; 4], |c| c.tone),
            influence: [
                cc.map_or(0.0, |c| c.influence),
                if cc.is_some() { 1.0 } else { 0.0 },
                0.0,
                0.0,
            ],
        };
        let scene_view = self.scene.view.clone();
        let bg = self.view_bind(&scene_view, bytemuck::bytes_of(&p));
        {
            let mut rp = Self::pass(&mut enc, &self.output_view, black);
            rp.set_pipeline(&self.mono_pipe);
            rp.set_bind_group(0, &bg, &[]);
            rp.draw(0..4, 0..1);
        }
        // EffectLayer particles, then the UILayer (talk window, fader) on top
        self.particles(&mut enc, &plan.particles);
        // overlay + UI + text
        let mut ui: Vec<wgpu::BindGroup> = self.quads(&plan.ui);
        ui.extend(self.quads(&plan.overlay));
        ui.extend(self.quads(&plan.ui_top));
        if let Some(t) = plan.text {
            ui.extend(self.quads(&[QuadDraw {
                image: Some(t),
                rect: [0.0, 0.0, w, h],
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [1.0; 4],
                premultiplied: true,
                blur: None,
                dolly: None,
            }]));
        }
        ui.extend(self.quads(&plan.cover));
        let out_view = self.output_view.clone();
        self.draw_quads(&mut enc, &out_view, &ui, None);

        enc.copy_texture_to_buffer(
            self.output.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([enc.finish()]);

        let slice = self.readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        rx.recv()
            .map_err(|e| GpuError::Readback(e.to_string()))?
            .map_err(|e| GpuError::Readback(e.to_string()))?;
        let row = (self.width * 4) as usize;
        let mut out = Vec::with_capacity(row * self.height as usize);
        {
            let data = slice
                .get_mapped_range()
                .map_err(|e| GpuError::Readback(e.to_string()))?;
            for y in 0..self.height as usize {
                let s = y * self.padded_row as usize;
                out.extend_from_slice(&data[s..s + row]);
            }
        }
        self.readback.unmap();
        Ok(out)
    }
}
