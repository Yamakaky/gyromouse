use std::{borrow::Cow, ops::Deref, path::Path};

use anyhow::Result;
use cgmath::Matrix4;

use crate::backend::sdl::{
    model::{ModelVertex, Vertex},
    texture,
};

use super::{animation::AnimationStore, material::Materials, model::Node};

pub const SAMPLE_COUNT: u32 = 4;

pub struct Scene {
    #[allow(unused)]
    materials: Materials,
    models: Vec<Node>,
    view_projection: Matrix4<f32>,
    pipeline: wgpu::RenderPipeline,
    pub animations: AnimationStore,
}

impl Scene {
    pub fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: wgpu::ColorTargetState,
        path: impl AsRef<Path>,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let (document, buffers, _images) = gltf::import(path)?;
        let scene = document.default_scene().expect("missing default scene");

        let materials = Materials::load(device, queue, &buffers, &document)?;
        let models = scene
            .nodes()
            .map(|node| Node::load(device, node, &materials, &buffers))
            .collect::<Result<_>>()?;
        let animations = AnimationStore::load(document.animations(), &buffers);

        // Create other resources
        let view_projection = generate_matrix(width as f32 / height as f32);

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("shad"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("my pipeline layout"),
            bind_group_layouts: &[&materials.bind_group_layout],
            push_constant_ranges: &[wgpu::PushConstantRange {
                stages: wgpu::ShaderStages::VERTEX,
                range: 0..128,
            }],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("my pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[ModelVertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[target],
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

        Ok(Self {
            materials,
            models,
            view_projection,
            pipeline,
            animations,
        })
    }

    pub fn draw<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, transform: impl Into<Matrix4<f32>>) {
        let transform = transform.into();
        pass.push_debug_group("Scene render");
        pass.set_pipeline(&self.pipeline);
        for model in &self.models {
            model.draw(pass, &self.animations, &self.view_projection, &transform);
        }
        pass.pop_debug_group();
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
