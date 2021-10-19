use std::{convert::TryInto, time::Duration};

use anyhow::{Error, Result};
use cgmath::{Deg, Euler, InnerSpace, One, Quaternion, Rotation, Rotation3, Vector3};
use sdl2::{
    controller::GameController,
    event::{Event, WindowEvent},
    video::Window,
    VideoSubsystem,
};

use crate::backend::sdl::{scene, texture};

pub struct Overlay {
    depth_texture: texture::Texture,
    scene: scene::Scene,
    window: Window,
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    multisampled_framebuffer: wgpu::TextureView,
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
                force_fallback_adapter: false,
            }))
            .unwrap();

        let limits = wgpu::Limits {
            max_push_constant_size: 128,
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

        let multisampled_framebuffer = Self::create_multisampled_framebuffer(&device, &config);

        let res_dir = std::path::Path::new("models");
        let scene = scene::Scene::load(
            &device,
            &queue,
            surface.get_preferred_format(&adapter).unwrap().into(),
            res_dir.join("controller.gltf"),
            width,
            height,
        )?;

        let depth_texture = texture::Texture::create_depth_texture(
            &device,
            &config,
            scene::SAMPLE_COUNT,
            "depth texture",
        );

        Ok(Self {
            depth_texture,
            scene,
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
            sample_count: scene::SAMPLE_COUNT,
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
                    scene::SAMPLE_COUNT,
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

        let frame = self.surface.get_current_texture()?;
        let view = &frame
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
            self.scene.draw(&mut rpass);
        }

        self.queue.submit(Some(encoder.finish()));

        frame.present();

        Ok(())
    }
}
