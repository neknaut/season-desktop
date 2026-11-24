//! GPU: thiết bị dùng chung (hình nền + cửa sổ cài đặt) và bộ vẽ hình nền.

use std::sync::Arc;
use winit::window::Window;

use crate::draw::{Blend, DrawList, Sprite, Tex};

pub struct Gpu {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl Gpu {
    /// Tạo GPU dùng được với `window`. Ưu tiên DX12 (nhẹ RAM hơn), rồi tới Vulkan.
    pub fn new(window: Arc<Window>) -> Result<(Gpu, wgpu::Surface<'static>), String> {
        for backends in [wgpu::Backends::DX12, wgpu::Backends::VULKAN] {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
            let Ok(surface) = instance.create_surface(window.clone()) else { continue };
            let Some(adapter) = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })) else {
                continue;
            };
            let (device, queue) = pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("season-desktop"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            ))
            .map_err(|e| e.to_string())?;
            crate::log(&format!("GPU: {} ({:?})", adapter.get_info().name, adapter.get_info().backend));
            return Ok((Gpu { instance, adapter, device, queue }, surface));
        }
        Err("không tìm thấy GPU dùng được".into())
    }

    /// GPU không cần cửa sổ (dùng cho chế độ chụp ảnh --snapshot).
    pub fn headless() -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN, ..Default::default() });
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).ok_or("không có GPU")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).map_err(|e| e.to_string())?;
        Ok(Gpu { instance, adapter, device, queue })
    }

    pub fn surface_config(&self, surface: &wgpu::Surface, w: u32, h: u32) -> wgpu::SurfaceConfiguration {
        let caps = surface.get_capabilities(&self.adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w.max(1),
            height: h.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    res: [f32; 2],
    scale: f32,
    _p0: f32,
    off: [f32; 2],
    _p1: [f32; 2],
}

struct Texture {
    tex: wgpu::Texture,
    bind: wgpu::BindGroup,
    size: (u32, u32),
}

pub struct Wallpaper {
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    pipe_alpha: wgpu::RenderPipeline,
    pipe_add: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    uploaded_globals: Option<Globals>,
    globals_bind: wgpu::BindGroup,
    tex_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    instances: wgpu::Buffer,
    capacity: usize,
    textures: [Option<Texture>; 4],
}

impl Wallpaper {
    pub fn new(gpu: &Gpu, surface: wgpu::Surface<'static>, w: u32, h: u32) -> Self {
        let config = gpu.surface_config(&surface, w, h);
        surface.configure(&gpu.device, &config);
        Self::with_config(gpu, Some(surface), config)
    }

    /// Bộ vẽ không có cửa sổ, chỉ vẽ ra ảnh.
    pub fn offscreen(gpu: &Gpu, w: u32, h: u32) -> Self {
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: w,
            height: h,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        Self::with_config(gpu, None, config)
    }

    fn with_config(gpu: &Gpu, surface: Option<wgpu::Surface<'static>>, config: wgpu::SurfaceConfiguration) -> Self {
        let device = &gpu.device;

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let g_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let globals_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &g_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let tex_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sprite"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/sprite.wgsl").into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&g_layout, &tex_layout],
            push_constant_ranges: &[],
        });
        let attrs = wgpu::vertex_attr_array![
            0 => Float32x2, 1 => Float32x2, 2 => Float32, 3 => Uint32, 4 => Float32x4, 5 => Float32x4
        ];
        let make = |blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("sprite"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Sprite>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &attrs,
                    }],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview: None,
                cache: None,
            })
        };
        let pipe_alpha = make(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let pipe_add = make(wgpu::BlendState { color: add, alpha: add });

        let capacity = 512;
        let instances = Self::make_instances(device, capacity);
        Self { surface, config, pipe_alpha, pipe_add, globals, uploaded_globals: None, globals_bind, tex_layout, sampler, instances, capacity, textures: [None, None, None, None] }
    }

    fn make_instances(device: &wgpu::Device, n: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sprites"),
            size: (n * std::mem::size_of::<Sprite>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    pub fn resize(&mut self, gpu: &Gpu, w: u32, h: u32) {
        if w == 0 || h == 0 || (w, h) == self.size() {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        if let Some(s) = &self.surface {
            s.configure(&gpu.device, &self.config);
        }
    }

    /// Đưa ảnh (RGBA đã nhân alpha) lên GPU vào ô `slot`.
    pub fn set_texture(&mut self, gpu: &Gpu, slot: Tex, w: u32, h: u32, data: &[u8]) {
        let i = slot as usize;
        let reuse = matches!(&self.textures[i], Some(t) if t.size == (w, h));
        if !reuse {
            let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&Default::default());
            let bind = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.tex_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.textures[i] = Some(Texture { tex, bind, size: (w, h) });
        }
        let t = self.textures[i].as_ref().unwrap();
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &t.tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            data,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    pub fn render(&mut self, gpu: &Gpu, dl: &DrawList, scale: f32, off: [f32; 2]) {
        let Some(surface) = &self.surface else { return };
        let frame = match surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                surface.configure(&gpu.device, &self.config);
                return;
            }
            Err(_) => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let enc = self.encode(gpu, dl, scale, off, &view);
        gpu.queue.submit([enc.finish()]);
        frame.present();
    }

    /// Vẽ ra ảnh RGBA (chế độ --snapshot).
    pub fn render_image(&mut self, gpu: &Gpu, dl: &DrawList, scale: f32, off: [f32; 2]) -> Vec<u8> {
        let (w, h) = self.size();
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let row = (4 * w).div_ceil(256) * 256;
        let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.encode(gpu, dl, scale, off, &view);
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        gpu.queue.submit([enc.finish()]);
        buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
        let data = buf.slice(..).get_mapped_range();
        let bgra = matches!(self.config.format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let s = (y * row) as usize;
            for px in data[s..s + (w * 4) as usize].chunks(4) {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], 255]);
                } else {
                    out.extend_from_slice(&[px[0], px[1], px[2], 255]);
                }
            }
        }
        out
    }

    fn encode(&mut self, gpu: &Gpu, dl: &DrawList, scale: f32, off: [f32; 2], view: &wgpu::TextureView) -> wgpu::CommandEncoder {
        if dl.sprites.len() > self.capacity {
            self.capacity = dl.sprites.len().next_power_of_two();
            self.instances = Self::make_instances(&gpu.device, self.capacity);
        }
        let g = Globals { res: [self.config.width as f32, self.config.height as f32], scale, _p0: 0.0, off, _p1: [0.0; 2] };
        if self.uploaded_globals != Some(g) {
            gpu.queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&g));
            self.uploaded_globals = Some(g);
        }
        if !dl.sprites.is_empty() {
            gpu.queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(&dl.sprites));
        }
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_bind_group(0, &self.globals_bind, &[]);
            pass.set_vertex_buffer(0, self.instances.slice(..));
            for cmd in &dl.cmds {
                let Some(t) = &self.textures[cmd.tex as usize] else { continue };
                pass.set_pipeline(if cmd.blend == Blend::Add { &self.pipe_add } else { &self.pipe_alpha });
                pass.set_bind_group(1, &t.bind, &[]);
                pass.draw(0..6, cmd.start..cmd.end);
            }
        }
        enc
    }
}
