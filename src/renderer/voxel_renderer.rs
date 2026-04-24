use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::core::voxel::VoxelGrid;
use crate::renderer::camera::OrbitCamera;

// ─── GPU Vertex Types ────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct CubeVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct VoxelInstance {
    pub position: [f32; 3],
    pub color: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    light_dir: [f32; 3],
    _pad: f32,
}

// Unit cube: 24 vertices (4 per face), 36 indices
static CUBE_VERTICES: &[CubeVertex] = &[
    // -Z
    CubeVertex { position: [0.0, 0.0, 0.0], normal: [0.0,  0.0, -1.0] },
    CubeVertex { position: [1.0, 0.0, 0.0], normal: [0.0,  0.0, -1.0] },
    CubeVertex { position: [1.0, 1.0, 0.0], normal: [0.0,  0.0, -1.0] },
    CubeVertex { position: [0.0, 1.0, 0.0], normal: [0.0,  0.0, -1.0] },
    // +Z
    CubeVertex { position: [1.0, 0.0, 1.0], normal: [0.0,  0.0,  1.0] },
    CubeVertex { position: [0.0, 0.0, 1.0], normal: [0.0,  0.0,  1.0] },
    CubeVertex { position: [0.0, 1.0, 1.0], normal: [0.0,  0.0,  1.0] },
    CubeVertex { position: [1.0, 1.0, 1.0], normal: [0.0,  0.0,  1.0] },
    // -X
    CubeVertex { position: [0.0, 0.0, 1.0], normal: [-1.0, 0.0,  0.0] },
    CubeVertex { position: [0.0, 0.0, 0.0], normal: [-1.0, 0.0,  0.0] },
    CubeVertex { position: [0.0, 1.0, 0.0], normal: [-1.0, 0.0,  0.0] },
    CubeVertex { position: [0.0, 1.0, 1.0], normal: [-1.0, 0.0,  0.0] },
    // +X
    CubeVertex { position: [1.0, 0.0, 0.0], normal: [ 1.0, 0.0,  0.0] },
    CubeVertex { position: [1.0, 0.0, 1.0], normal: [ 1.0, 0.0,  0.0] },
    CubeVertex { position: [1.0, 1.0, 1.0], normal: [ 1.0, 0.0,  0.0] },
    CubeVertex { position: [1.0, 1.0, 0.0], normal: [ 1.0, 0.0,  0.0] },
    // -Y
    CubeVertex { position: [0.0, 0.0, 1.0], normal: [ 0.0, -1.0, 0.0] },
    CubeVertex { position: [1.0, 0.0, 1.0], normal: [ 0.0, -1.0, 0.0] },
    CubeVertex { position: [1.0, 0.0, 0.0], normal: [ 0.0, -1.0, 0.0] },
    CubeVertex { position: [0.0, 0.0, 0.0], normal: [ 0.0, -1.0, 0.0] },
    // +Y
    CubeVertex { position: [0.0, 1.0, 0.0], normal: [ 0.0,  1.0, 0.0] },
    CubeVertex { position: [1.0, 1.0, 0.0], normal: [ 0.0,  1.0, 0.0] },
    CubeVertex { position: [1.0, 1.0, 1.0], normal: [ 0.0,  1.0, 0.0] },
    CubeVertex { position: [0.0, 1.0, 1.0], normal: [ 0.0,  1.0, 0.0] },
];

static CUBE_INDICES: &[u16] = &[
    0,  1,  2,  0,  2,  3,   // -Z
    4,  5,  6,  4,  6,  7,   // +Z
    8,  9,  10, 8,  10, 11,  // -X
    12, 13, 14, 12, 14, 15,  // +X
    16, 17, 18, 16, 18, 19,  // -Y
    20, 21, 22, 20, 22, 23,  // +Y
];

// Offscreen formats. sRGB encodes gamma automatically so the voxel shader
// outputs raw linear values and the GPU handles the sRGB conversion.
const OFFSCREEN_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const OFFSCREEN_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

// ─── Renderer ────────────────────────────────────────────────────────────────

pub struct VoxelRenderer {
    // Voxel pass: renders to offscreen with full depth buffer
    voxel_pipeline: wgpu::RenderPipeline,
    vertex_buf: wgpu::Buffer,
    index_buf: wgpu::Buffer,
    instance_buf: wgpu::Buffer,
    uniform_buf: wgpu::Buffer,
    uniform_bg: wgpu::BindGroup,

    // Blit pass: copies offscreen color into egui render pass
    blit_pipeline: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    blit_bind_group: Option<wgpu::BindGroup>,
    blit_sampler: wgpu::Sampler,

    // Offscreen targets (lazily created; recreated on resize)
    offscreen_color: Option<wgpu::Texture>,
    offscreen_color_view: Option<wgpu::TextureView>,
    offscreen_depth: Option<wgpu::Texture>,
    offscreen_depth_view: Option<wgpu::TextureView>,
    offscreen_size: [u32; 2],

    instance_count: u32,
    max_instances: u32,
}

impl VoxelRenderer {
    const MAX_INSTANCES: u32 = 2_000_000;

    pub fn new(device: &wgpu::Device, egui_format: wgpu::TextureFormat) -> Self {
        let voxel_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("voxel"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/voxel.wgsl").into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/blit.wgsl").into()),
        });

        // ── Uniform buffer + bind group (shared by voxel pipeline) ──────────
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("voxel_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("voxel_bg"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        // ── Voxel render pipeline (targets OFFSCREEN_COLOR_FORMAT with depth) ─
        let vertex_attrs_v = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let vertex_attrs_i = wgpu::vertex_attr_array![2 => Float32x3, 3 => Float32x3];

        let voxel_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("voxel_layout"),
            bind_group_layouts: &[&uniform_bgl],
            push_constant_ranges: &[],
        });
        let voxel_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("voxel_pipeline"),
            layout: Some(&voxel_layout),
            vertex: wgpu::VertexState {
                module: &voxel_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<CubeVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &vertex_attrs_v,
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<VoxelInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &vertex_attrs_i,
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: OFFSCREEN_DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &voxel_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: OFFSCREEN_COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // ── Blit bind group layout (texture + sampler) ───────────────────────
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blit_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // ── Blit pipeline (no depth; targets egui surface format) ───────────
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_layout"),
            bind_group_layouts: &[&blit_bgl],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: egui_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        // ── Geometry buffers ─────────────────────────────────────────────────
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_verts"),
            contents: bytemuck::cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cube_idx"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (Self::MAX_INSTANCES as usize * std::mem::size_of::<VoxelInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            voxel_pipeline,
            vertex_buf,
            index_buf,
            instance_buf,
            uniform_buf,
            uniform_bg,
            blit_pipeline,
            blit_bgl,
            blit_bind_group: None,
            blit_sampler,
            offscreen_color: None,
            offscreen_color_view: None,
            offscreen_depth: None,
            offscreen_depth_view: None,
            offscreen_size: [0, 0],
            instance_count: 0,
            max_instances: Self::MAX_INSTANCES,
        }
    }

    /// Create or resize offscreen color+depth textures when the viewport changes.
    pub fn ensure_offscreen(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if size[0] == 0 || size[1] == 0 { return; }
        if self.offscreen_size == size && self.offscreen_color.is_some() { return; }
        self.offscreen_size = size;

        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_color"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&Default::default());

        let depth_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen_depth"),
            size: wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OFFSCREEN_DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_tex.create_view(&Default::default());

        // Recreate blit bind group pointing at the new color view.
        // wgpu BindGroup holds internal Arc refs, so the view can be moved after.
        self.blit_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bg"),
            layout: &self.blit_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                },
            ],
        }));

        self.offscreen_color = Some(color_tex);
        self.offscreen_color_view = Some(color_view);
        self.offscreen_depth = Some(depth_tex);
        self.offscreen_depth_view = Some(depth_view);
    }

    /// Upload instance data from the voxel grid (call after voxelization).
    pub fn update_instances(&mut self, queue: &wgpu::Queue, grid: &VoxelGrid) {
        let instances: Vec<VoxelInstance> = grid
            .iter_occupied()
            .take(self.max_instances as usize)
            .map(|(pos, voxel)| VoxelInstance {
                position: [pos.x as f32, pos.y as f32, pos.z as f32],
                color: [
                    voxel.color[0] as f32 / 255.0,
                    voxel.color[1] as f32 / 255.0,
                    voxel.color[2] as f32 / 255.0,
                ],
            })
            .collect();

        self.instance_count = instances.len() as u32;
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buf, 0, bytemuck::cast_slice(&instances));
        }
    }

    /// Upload view-projection + light uniforms (call every frame in prepare).
    pub fn update_uniforms(&self, queue: &wgpu::Queue, camera: &OrbitCamera) {
        let uniforms = Uniforms {
            view_proj: camera.view_proj().to_cols_array_2d(),
            light_dir: [0.5, 1.0, 0.3],
            _pad: 0.0,
        };
        queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Record a render pass into `encoder` that draws voxels to the offscreen
    /// texture with depth testing. Call this from CallbackTrait::prepare so it
    /// executes before egui's main render pass begins.
    pub fn render_to_offscreen(&self, encoder: &mut wgpu::CommandEncoder) {
        let (Some(color_view), Some(depth_view)) =
            (&self.offscreen_color_view, &self.offscreen_depth_view)
        else {
            return;
        };

        // BG_DARK (22,22,30) converted from sRGB to linear for the clear value.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("voxel_offscreen"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0064,
                        g: 0.0064,
                        b: 0.011,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        if self.instance_count > 0 {
            pass.set_pipeline(&self.voxel_pipeline);
            pass.set_bind_group(0, &self.uniform_bg, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            pass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..self.instance_count);
        }
    }

    /// Draw the offscreen texture to the current egui render pass as a fullscreen
    /// quad. Call this from CallbackTrait::paint.
    pub fn blit<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        let Some(bg) = &self.blit_bind_group else { return };
        render_pass.set_pipeline(&self.blit_pipeline);
        render_pass.set_bind_group(0, bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
