use std::{borrow::Cow, convert::TryInto, time::Duration};

use anyhow::{Error, Result};
use cgmath::{Deg, Euler, InnerSpace, Matrix4, One, Quaternion, Rotation, Rotation3, Vector3};
use sdl2::{
    controller::GameController,
    event::{Event, WindowEvent},
    video::Window,
    VideoSubsystem,
};
use wgpu::util::DeviceExt;

use crate::backend::sdl::{
    model::{self, ModelVertex, Vertex},
    texture,
};

use super::model::DrawModel;

const SAMPLE_COUNT: u32 = 4;

pub struct Overlay {
    depth_texture: texture::Texture,
    uniform_bind_group: wgpu::BindGroup,
    model: model::Model,
    window: Window,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    multisampled_framebuffer: wgpu::TextureView,
    pipeline: wgpu::RenderPipeline,
    config: wgpu::SurfaceConfiguration,
    rotation: Quaternion<f64>,
}

impl Overlay {
    pub fn new(video_subsystem: &VideoSubsystem, wgpu_instance: &wgpu::Instance) -> Result<Self> {
        let mut window = video_subsystem
            .window("Raw Window Handle Example", 800, 600)
            .position_centered()
            .resizable()
            .build()?;
        let (width, height) = window.size();

        window.set_opacity(0.5).map_err(|s| Error::msg(s))?;

        let surface = unsafe { wgpu_instance.create_surface(&window) };
        let adapter =
            pollster::block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
            }))
            .unwrap();

        let limits = wgpu::Limits {
            max_push_constant_size: 64,
            ..wgpu::Limits::downlevel_defaults()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("my device"),
                features: wgpu::Features::PUSH_CONSTANTS,
                limits: limits.clone(),
            },
            None,
        ))?;

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_preferred_format(&adapter).unwrap(),
            width,
            height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &config);

        // Create other resources
        let mx_total = generate_matrix(width as f32 / height as f32);
        let mx_ref: &[f32; 16] = mx_total.as_ref();
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(mx_ref),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("shad"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let vertex_buffers = [ModelVertex::desc()];

        // Create pipeline layout
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                }],
            });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("uniform bind group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buf,
                    offset: 0,
                    size: None,
                }),
            }],
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("my pipeline layout"),
            bind_group_layouts: &[&texture_bind_group_layout, &uniform_bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..64,
            }],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("my pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &vertex_buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[surface.get_preferred_format(&adapter).unwrap().into()],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: SAMPLE_COUNT,
                ..Default::default()
            },
        });

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(&device, &config);

        let res_dir = std::path::Path::new("models");
        let model = model::Model::load(
            &device,
            &queue,
            &texture_bind_group_layout,
            res_dir.join("controller.obj"),
        )?;

        let depth_texture =
            texture::Texture::create_depth_texture(&device, &config, SAMPLE_COUNT, "depth texture");

        Ok(Self {
            depth_texture,
            uniform_bind_group,
            model,
            pipeline,
            device,
            surface,
            queue,
            window,
            config,
            multisampled_framebuffer,
            rotation: Quaternion::one(),
        })
    }

    fn create_multisampled_framebuffer(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::TextureView {
        let multisampled_texture_extent = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let multisampled_frame_descriptor = &wgpu::TextureDescriptor {
            size: multisampled_texture_extent,
            mip_level_count: 1,
            sample_count: SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        };

        device
            .create_texture(multisampled_frame_descriptor)
            .create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn event(&mut self, event: &Event) {
        match event {
            Event::Window {
                window_id,
                win_event: WindowEvent::SizeChanged(width, height),
                ..
            } if *window_id == self.window.id() => {
                self.config.width = (*width).try_into().unwrap();
                self.config.height = (*height).try_into().unwrap();
                self.surface.configure(&self.device, &self.config);
                self.multisampled_framebuffer =
                    Self::create_multisampled_framebuffer(&self.device, &self.config);
                self.depth_texture = texture::Texture::create_depth_texture(
                    &self.device,
                    &self.config,
                    SAMPLE_COUNT,
                    "depth texture",
                );
            }
            _ => {}
        }
    }

    pub fn tick(
        &mut self,
        delta_rotation: Euler<Deg<f64>>,
        up_vector: cgmath::Vector3<f64>,
        _dt: Duration,
        _controller: &GameController,
    ) -> Result<()> {
        if delta_rotation != Euler::new(Deg(0.), Deg(0.), Deg(0.)) {
            self.rotation = (self.rotation * Quaternion::from(delta_rotation)).normalize();
            let raw_rot = Euler::from(self.rotation);
            let computed_up = self.rotation.invert().rotate_vector(Vector3::unit_y());
            self.rotation = self.rotation
                * Quaternion::one()
                    .slerp(Quaternion::between_vectors(computed_up, up_vector), 0.01)
                    .invert()
                * Quaternion::from_angle_y(-raw_rot.y * 0.0005);
        }

        let frame = self.surface.get_current_frame()?;
        let view = &frame
            .output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[wgpu::RenderPassColorAttachment {
                    view: &self.multisampled_framebuffer,
                    resolve_target: Some(view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true,
                    },
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });
            rpass.push_debug_group("Prepare data for draw.");
            rpass.set_pipeline(&self.pipeline);
            let rot_raw: [u8; 4 * 16] =
                unsafe { std::mem::transmute(Matrix4::from(self.rotation.cast::<f32>().unwrap())) };

            rpass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, &rot_raw);
            rpass.draw_model(&self.model, &self.uniform_bind_group);
        }

        self.queue.submit(Some(encoder.finish()));

        Ok(())
    }
}
fn generate_matrix(aspect_ratio: f32) -> cgmath::Matrix4<f32> {
    let mx_projection = cgmath::perspective(cgmath::Deg(45f32), aspect_ratio, 1.0, 10.0);
    let mx_view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::new(0., 5., 0.),
        cgmath::Point3::new(0., 0., 0.),
        -cgmath::Vector3::unit_z(),
    );
    let mx_correction = OPENGL_TO_WGPU_MATRIX;
    mx_correction * mx_projection * mx_view
}

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
);
