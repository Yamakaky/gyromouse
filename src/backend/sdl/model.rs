use anyhow::*;
use cgmath::{Matrix4, Transform};
use std::convert::TryInto;
use std::sync::Arc;
use wgpu::util::DeviceExt;

use super::material::{Material, Materials};

pub trait Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a>;
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelVertex {
    position: [f32; 3],
    uv: [f32; 2],
    normal: [f32; 3],
}

impl Vertex for ModelVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<ModelVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

pub struct Primitive {
    vertices_buffer: wgpu::Buffer,
    indices_buffer: wgpu::Buffer,
    num_elements: u32,
    material: Arc<Material>,
}

impl Primitive {
    pub fn load(
        device: &wgpu::Device,
        primitive: gltf::Primitive,
        materials: &Materials,
        buffers: &[gltf::buffer::Data],
        mesh_label: Option<&str>,
        primitive_idx: usize,
    ) -> Result<Self> {
        assert_eq!(primitive.mode(), gltf::mesh::Mode::Triangles);
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        let vertices: Vec<_> = reader
            .read_positions()
            .expect("missing positions")
            .zip(reader.read_tex_coords(0).expect("missing uv").into_f32())
            .zip(reader.read_normals().expect("missing normals"))
            .map(|((position, uv), normal)| ModelVertex {
                position,
                uv,
                normal,
            })
            .collect();
        let positions_label =
            mesh_label.map(|s| format!("Mesh '{}' > Primitive {} > Vertices", s, primitive_idx));
        let vertices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: positions_label.as_deref(),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices: Vec<_> = reader
            .read_indices()
            .expect("missing indices")
            .into_u32()
            .collect();
        let indices_label =
            mesh_label.map(|s| format!("Mesh '{}' > Primitive '{}' > Indices", s, primitive_idx));
        let indices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: indices_label.as_deref(),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let material = materials[primitive
            .material()
            .index()
            .expect("missing material index")]
        .clone();

        Ok(Self {
            vertices_buffer,
            indices_buffer,

            num_elements: indices.len().try_into().expect("int overflow"),
            material,
        })
    }

    fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        view_projection: &Matrix4<f32>,
        transform: &Matrix4<f32>,
    ) {
        pass.set_vertex_buffer(0, self.vertices_buffer.slice(..));
        pass.set_index_buffer(self.indices_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.material.as_ref().set_bind_group(pass, 0);
        let raw_transform: [u8; 2 * 4 * 16] =
            unsafe { std::mem::transmute((view_projection * transform, transform.clone())) };
        pass.set_push_constants(wgpu::ShaderStages::VERTEX, 0, &raw_transform);
        pass.draw_indexed(0..self.num_elements, 0, 0..1)
    }
}

pub struct Mesh {
    name: Option<String>,
    primitives: Vec<Primitive>,
}

impl Mesh {
    pub fn load(
        device: &wgpu::Device,
        mesh: gltf::Mesh,
        materials: &Materials,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self> {
        let name = mesh.name().map(String::from);
        let primitives = mesh
            .primitives()
            .enumerate()
            .map(|(i, primitive)| {
                Primitive::load(device, primitive, materials, buffers, name.as_deref(), i)
            })
            .collect::<Result<_>>()?;

        Ok(Self { name, primitives })
    }

    fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        view_projection: &Matrix4<f32>,
        transform: &Matrix4<f32>,
    ) {
        if let Some(name) = &self.name {
            pass.push_debug_group(&format!("Render {}", name));
        }
        for primitive in &self.primitives {
            primitive.draw(pass, view_projection, transform);
        }
        if self.name.is_some() {
            pass.pop_debug_group();
        }
    }
}

pub struct Model {
    transform: Matrix4<f32>,
    mesh: Option<Mesh>,
    children: Vec<Model>,
}

impl Model {
    pub fn load(
        device: &wgpu::Device,
        node: gltf::Node,
        materials: &Materials,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self> {
        let transform = node.transform().matrix().into();
        let mesh = node
            .mesh()
            .map(|mesh| Mesh::load(device, mesh, materials, buffers))
            .transpose()?;
        let children = node
            .children()
            .map(|child| Model::load(device, child, materials, buffers))
            .collect::<Result<_>>()?;

        Ok(Self {
            transform,
            mesh,
            children,
        })
    }

    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        view_projection: &Matrix4<f32>,
        parent_transform: &Matrix4<f32>,
    ) {
        let transform = parent_transform.concat(&self.transform);
        if let Some(mesh) = &self.mesh {
            mesh.draw(pass, view_projection, &transform);
        }
        for child in &self.children {
            child.draw(pass, view_projection, &transform);
        }
    }
}
