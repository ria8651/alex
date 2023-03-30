use crate::render_pipeline::load_anvil::load_anvil;
use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        RenderApp, RenderSet,
    },
};

pub struct VoxelWorldPlugin;

impl Plugin for VoxelWorldPlugin {
    fn build(&self, app: &mut App) {
        let render_device = app.world.resource::<RenderDevice>();
        let render_queue = app.world.resource::<RenderQueue>();

        // load world
        let brick_map_depth = 5;
        let brick_texture_size = UVec3::new(512, 512, 256);
        let (octree, texture_data) = load_anvil(brick_map_depth, brick_texture_size);

        let (head, data, tail) = unsafe { octree.align_to::<u8>() };
        assert!(head.is_empty());
        assert!(tail.is_empty());

        // uniforms
        let voxel_uniforms = VoxelUniforms { brick_map_depth };
        let mut uniform_buffer = UniformBuffer::from(voxel_uniforms.clone());
        uniform_buffer.write_buffer(render_device, render_queue);

        // texture
        let bricks = render_device.create_texture_with_data(
            render_queue,
            &TextureDescriptor {
                label: None,
                view_formats: &[TextureFormat::Rgba8Unorm],
                size: Extent3d {
                    width: brick_texture_size.x,
                    height: brick_texture_size.y,
                    depth_or_array_layers: brick_texture_size.z,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D3,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING | TextureUsages::COPY_DST,
            },
            &texture_data,
        );
        let bricks = bricks.create_view(&TextureViewDescriptor::default());

        // storage
        let brickmap = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: data,
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("voxelization bind group layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(VoxelUniforms::SHADER_SIZE.into()),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(4),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadWrite,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.binding().unwrap(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: brickmap.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(&bricks),
                },
            ],
        });

        app.sub_app_mut(RenderApp)
            .insert_resource(voxel_uniforms)
            .insert_resource(VoxelData {
                uniform_buffer,
                bricks,
                brickmap,
                bind_group_layout,
                bind_group,
            })
            .add_system(prepare_uniforms.in_set(RenderSet::Prepare))
            .add_system(queue_bind_group.in_set(RenderSet::Queue));
    }
}

#[derive(Resource)]
pub struct VoxelData {
    pub uniform_buffer: UniformBuffer<VoxelUniforms>,
    pub bricks: TextureView,
    pub brickmap: Buffer,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}

#[derive(Resource, ExtractResource, Clone, ShaderType)]
pub struct VoxelUniforms {
    brick_map_depth: u32,
}

fn prepare_uniforms(
    voxel_uniforms: Res<VoxelUniforms>,
    mut voxel_data: ResMut<VoxelData>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    voxel_data.uniform_buffer.set(voxel_uniforms.clone());
    voxel_data
        .uniform_buffer
        .write_buffer(&render_device, &render_queue);
}

fn queue_bind_group(render_device: Res<RenderDevice>, mut voxel_data: ResMut<VoxelData>) {
    let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &voxel_data.bind_group_layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: voxel_data.uniform_buffer.binding().unwrap(),
            },
            BindGroupEntry {
                binding: 1,
                resource: voxel_data.brickmap.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::TextureView(&voxel_data.bricks),
            },
        ],
    });
    voxel_data.bind_group = bind_group;
}
