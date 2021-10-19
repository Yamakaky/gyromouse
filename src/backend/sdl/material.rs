use std::{collections::HashMap, ops::Index, sync::Arc};

use anyhow::{bail, Result};
use cgmath::Vector4;
use crevice::std430::{AsStd430, Std430};
use gltf::{
    texture::{MagFilter, MinFilter},
    Document,
};
use image::ImageFormat;
use wgpu::util::DeviceExt;

use crate::backend::sdl::texture;

pub type MaterialId = usize;

pub struct Materials {
    materials: HashMap<MaterialId, Arc<Material>>,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl Materials {
    pub fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffers: &[gltf::buffer::Data],
        document: &Document,
    ) -> Result<Self> {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("materials"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Sampler {
                        filtering: true,
                        comparison: false,
                    },
                    count: None,
                },
            ],
        });
        let materials = document
            .materials()
            .map(|mat| {
                Ok((
                    mat.index().expect("default material not supported"),
                    Arc::new(Material::load(
                        device,
                        queue,
                        mat,
                        buffers,
                        &bind_group_layout,
                    )?),
                ))
            })
            .collect::<Result<_>>()?;
        Ok(Self {
            materials,
            bind_group_layout,
        })
    }
}

impl Index<MaterialId> for Materials {
    type Output = Arc<Material>;

    fn index(&self, index: MaterialId) -> &Self::Output {
        &self.materials[&index]
    }
}

#[derive(Debug, Clone, Copy, crevice::std430::AsStd430)]
struct MaterialData {
    base_color: mint::Vector4<f32>,
    use_base_color_texture: u32,
}

pub struct Material {
    #[allow(unused)]
    name: Option<String>,
    #[allow(unused)]
    base_color: Vector4<f32>,
    #[allow(unused)]
    base_color_texture: Option<texture::Texture>,
    #[allow(unused)]
    option_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl Material {
    fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mat: gltf::Material,
        buffers: &[gltf::buffer::Data],
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Result<Material> {
        let name = mat.name();
        let pbr = mat.pbr_metallic_roughness();
        let base_color = Vector4::from(pbr.base_color_factor());
        let base_color_texture = pbr
            .base_color_texture()
            .map(|info| {
                assert_eq!(info.tex_coord(), 0);
                let info_tex = info.texture();
                let label = info_tex.name();
                texture::Texture::from_image(
                    device,
                    queue,
                    &Self::load_image(info_tex.source(), buffers)?,
                    Some(device.create_sampler(&Self::convert_sampler(info_tex.sampler(), label))),
                    label,
                )
            })
            .transpose()?;

        let data = MaterialData {
            base_color: base_color.into(),
            use_base_color_texture: base_color_texture.is_some().into(),
        };
        let data_label = name.map(|s| format!("Material '{}' > Data", s));
        let data_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: data_label.as_deref(),
            contents: data.as_std430().as_bytes(),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let mut bind_group_entries = vec![wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &data_buffer,
                offset: 0,
                size: None,
            }),
        }];
        if let Some(texture) = &base_color_texture {
            bind_group_entries.extend_from_slice(&[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&texture.sampler),
                },
            ]);
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: name.as_deref(),
            layout: bind_group_layout,
            entries: &bind_group_entries,
        });

        Ok(Self {
            name: name.map(String::from),
            base_color,
            base_color_texture,
            option_buffer: data_buffer,
            bind_group,
        })
    }

    fn load_image(
        texture: gltf::Image,
        buffers: &[gltf::buffer::Data],
    ) -> Result<image::DynamicImage> {
        match texture.source() {
            gltf::image::Source::View { view, mime_type } => {
                let parent_buffer_data = &buffers[view.buffer().index()].0;
                let begin = view.offset();
                let end = begin + view.length();
                let data = &parent_buffer_data[begin..end];
                Ok(match mime_type {
                    "image/jpeg" => image::load_from_memory_with_format(data, ImageFormat::Jpeg)?,
                    "image/png" => image::load_from_memory_with_format(data, ImageFormat::Png)?,
                    _ => bail!(
                        "unsupported image type (image: {}, mime_type: {})",
                        texture.index(),
                        mime_type
                    ),
                })
            }
            gltf::image::Source::Uri { uri, mime_type } => todo!(),
        }
    }

    fn convert_sampler<'a>(
        sampler: gltf::texture::Sampler,
        label: Option<&'a str>,
    ) -> wgpu::SamplerDescriptor<'a> {
        use wgpu::*;

        fn address_mode(mode: gltf::texture::WrappingMode) -> AddressMode {
            match mode {
                gltf::texture::WrappingMode::ClampToEdge => AddressMode::ClampToEdge,
                gltf::texture::WrappingMode::MirroredRepeat => AddressMode::MirrorRepeat,
                gltf::texture::WrappingMode::Repeat => AddressMode::Repeat,
            }
        }

        let mag_filter = match sampler.mag_filter().unwrap_or(MagFilter::Linear) {
            MagFilter::Nearest => FilterMode::Nearest,
            MagFilter::Linear => FilterMode::Linear,
        };

        let (min_filter, mipmap_filter) = match sampler.min_filter().unwrap_or(MinFilter::Linear) {
            MinFilter::Nearest => (FilterMode::Nearest, FilterMode::Linear),
            MinFilter::Linear => (FilterMode::Linear, FilterMode::Linear),
            MinFilter::NearestMipmapNearest => (FilterMode::Nearest, FilterMode::Nearest),
            MinFilter::NearestMipmapLinear => (FilterMode::Nearest, FilterMode::Linear),
            MinFilter::LinearMipmapNearest => (FilterMode::Linear, FilterMode::Nearest),
            MinFilter::LinearMipmapLinear => (FilterMode::Linear, FilterMode::Linear),
        };

        SamplerDescriptor {
            label,
            address_mode_u: address_mode(sampler.wrap_s()),
            address_mode_v: address_mode(sampler.wrap_t()),
            address_mode_w: AddressMode::Repeat,
            mag_filter,
            min_filter,
            mipmap_filter,
            ..Default::default()
        }
    }

    pub fn set_bind_group<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>, index: u32) {
        pass.set_bind_group(index, &self.bind_group, &[])
    }
}
