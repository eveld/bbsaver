use crate::atlas::FontAtlas;
use crate::cell::Row;

/// Per-instance data sent to the GPU for each visible cell.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuInstance {
    grid_pos: [u32; 2], // col, row
    glyph: u32,
    colors: u32, // fg | (bg << 8)
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    cell_size: [f32; 2],
    scroll_offset: f32,
    margin_left: f32,
    margin_top: f32,
    _pad: f32,
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    max_instances: usize,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        atlas: &FontAtlas,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pre-allocate instance buffer for up to 160 * 100 cells (wide art, 100 visible rows)
        let max_instances = 160 * 100;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("instances"),
            size: (max_instances * std::mem::size_of::<GpuInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&atlas.sampler),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuInstance>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        // grid_pos
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Uint32x2,
                        },
                        // glyph
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Uint32,
                        },
                        // colors
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Uint32,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Renderer {
            pipeline,
            uniform_buffer,
            instance_buffer,
            bind_group,
            max_instances,
        }
    }

    /// Calculate cell size and margins.
    /// Cell size is derived from `reference_width` (the narrowest screen) so that
    /// the art fills that screen edge-to-edge.  Wider screens get centered black bars.
    /// Vertically, only whole rows are shown, centered with equal top/bottom margins.
    /// Returns (cell_w, cell_h, margin_left, margin_top).
    pub fn layout(
        window_width: u32,
        window_height: u32,
        reference_width: u32,
        cols: usize,
    ) -> (f32, f32, f32, f32) {
        let cols_f = cols as f32;
        let cell_w = reference_width as f32 / cols_f;
        let cell_h = cell_w * 2.0; // 8:16 aspect ratio
        let art_width = cell_w * cols_f;
        let margin_left = (window_width as f32 - art_width) / 2.0;
        let visible_rows = (window_height as f32 / cell_h).floor();
        let margin_top = (window_height as f32 - visible_rows * cell_h) / 2.0;
        (cell_w, cell_h, margin_left.max(0.0), margin_top)
    }

    /// How many whole rows fit in the viewport.
    pub fn viewport_rows(window_height: u32, reference_width: u32, cols: usize) -> usize {
        let (_, cell_h, _, _) = Self::layout(reference_width, window_height, reference_width, cols);
        (window_height as f32 / cell_h).floor() as usize + 1
    }

    /// Render visible rows of the row buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        queue: &wgpu::Queue,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        rows: &[Row],
        cols: usize,
        scroll_offset: f64,
        window_size: [u32; 2],
        reference_width: u32,
    ) {
        if rows.is_empty() {
            return;
        }

        let (cell_w, cell_h, margin_left, margin_top) =
            Self::layout(window_size[0], window_size[1], reference_width, cols);
        let viewport_rows = Self::viewport_rows(window_size[1], reference_width, cols);

        let first_row = scroll_offset.floor() as usize;
        let frac = scroll_offset - scroll_offset.floor();

        // Build instance data for visible cells
        let mut instances = Vec::with_capacity(viewport_rows * cols);
        for i in 0..viewport_rows {
            let row_idx = (first_row + i) % rows.len();
            let row = &rows[row_idx];
            for (col, cell) in row.iter().enumerate() {
                instances.push(GpuInstance {
                    grid_pos: [col as u32, i as u32],
                    glyph: cell.glyph as u32,
                    colors: cell.fg as u32 | ((cell.bg as u32) << 8),
                });
            }
        }

        let instance_count = instances.len().min(self.max_instances);

        // Update uniforms
        let uniforms = Uniforms {
            screen_size: [window_size[0] as f32, window_size[1] as f32],
            cell_size: [cell_w, cell_h],
            scroll_offset: frac as f32,
            margin_left,
            margin_top,
            _pad: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Update instance buffer
        queue.write_buffer(
            &self.instance_buffer,
            0,
            bytemuck::cast_slice(&instances[..instance_count]),
        );

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.instance_buffer.slice(..));

            // Clip to the area containing whole rows so partial rows don't peek through
            let visible_rows = (window_size[1] as f32 / cell_h).floor();
            let clip_h = (visible_rows * cell_h) as u32;
            let clip_y = margin_top as u32;
            pass.set_scissor_rect(0, clip_y, window_size[0], clip_h.min(window_size[1] - clip_y));

            pass.draw(0..6, 0..instance_count as u32);
        }
    }
}
