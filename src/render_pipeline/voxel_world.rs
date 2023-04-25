use super::{cpu_brickmap::CpuBrickmap, load_anvil::load_anvil};
use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        RenderApp, RenderSet,
    },
};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Resource, Deref, DerefMut)]
pub struct CpuVoxelWorld(CpuBrickmap);

#[derive(Resource)]
pub struct GpuVoxelWorld {
    pub brickmap: Vec<u32>,
    pub brickmap_holes: VecDeque<usize>,
    pub brick_holes: VecDeque<usize>,
    pub brick_texture_size: UVec3,
}

pub struct VoxelWorldPlugin;

impl Plugin for VoxelWorldPlugin {
    fn build(&self, app: &mut App) {
        let render_device = app.world.resource::<RenderDevice>();
        let render_queue = app.world.resource::<RenderQueue>();

        // brickmap settings
        let brickmap_depth = 6;
        let brick_texture_size = UVec3::splat(640);
        let brickmap_max_nodes = 1 << 12;

        // load world (slooowwww)
        let path = PathBuf::from("assets/worlds/hermitcraft7");
        let mut cpu_brickmap = load_anvil(path, brickmap_depth);
        cpu_brickmap.recreate_mipmaps();

        // setup gpu brickmap
        let brickmap = vec![0; 4 * 8 * brickmap_max_nodes];
        let dim = brick_texture_size / 16;
        let brick_count = (dim.x * dim.y * dim.z) as usize;
        let gpu_voxel_world = GpuVoxelWorld {
            brickmap,
            brickmap_holes: (1..brickmap_max_nodes).collect::<VecDeque<usize>>(),
            brick_holes: (1..brick_count).collect::<VecDeque<usize>>(),
            brick_texture_size,
        };

        // let (brickmap, bricks) = cpu_brickmap.to_gpu(brick_texture_size);
        // let (head, brickmap, tail) = unsafe { brickmap.align_to::<u8>() };
        // assert!(head.is_empty());
        // assert!(tail.is_empty());

        // uniforms
        let voxel_uniforms = VoxelUniforms {
            brickmap_depth: brickmap_depth,
        };
        let mut uniform_buffer = UniformBuffer::from(voxel_uniforms.clone());
        uniform_buffer.write_buffer(render_device, render_queue);

        // storage
        let (_, brickmap, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
        let brickmap = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: brickmap,
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // texture
        let texture_size = brick_texture_size.x * brick_texture_size.y * brick_texture_size.z;
        let bricks = vec![0; 4 * texture_size as usize];
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
                usage: TextureUsages::STORAGE_BINDING,
            },
            &bricks,
        );
        let bricks_view = bricks.create_view(&TextureViewDescriptor::default());

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
                    resource: BindingResource::TextureView(&bricks_view),
                },
            ],
        });

        app.sub_app_mut(RenderApp)
            .insert_resource(voxel_uniforms)
            .insert_resource(VoxelData {
                uniform_buffer,
                bricks,
                bricks_view,
                brickmap,
                bind_group_layout,
                bind_group,
            })
            .insert_resource(CpuVoxelWorld(cpu_brickmap))
            .insert_resource(gpu_voxel_world)
            .add_system(prepare_uniforms.in_set(RenderSet::Prepare))
            .add_system(queue_bind_group.in_set(RenderSet::Queue));
    }
}

#[derive(Resource)]
pub struct VoxelData {
    pub uniform_buffer: UniformBuffer<VoxelUniforms>,
    pub bricks: Texture,
    pub bricks_view: TextureView,
    pub brickmap: Buffer,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}

#[derive(Resource, ExtractResource, Clone, ShaderType)]
pub struct VoxelUniforms {
    brickmap_depth: u32,
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
                resource: BindingResource::TextureView(&voxel_data.bricks_view),
            },
        ],
    });
    voxel_data.bind_group = bind_group;
}
