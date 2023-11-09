use super::{
    cpu_brickmap::{Brick, CpuBrickmap, BRICK_SIZE, COUNTER_BITS},
    load_anvil::load_anvil,
    voxel_streaming::BRICK_OFFSET,
};
use bevy::{
    prelude::*,
    render::{
        extract_resource::ExtractResource,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        Render, RenderApp, RenderSet,
    },
};
use std::{collections::VecDeque, path::PathBuf};

#[derive(Resource, Deref, DerefMut)]
pub struct CpuVoxelWorld(CpuBrickmap);

#[derive(Resource)]
pub struct GpuVoxelWorld {
    pub brickmap: Vec<u32>,
    pub gpu_to_cpu: Vec<u32>,
    pub brickmap_holes: VecDeque<usize>,
    pub brick_holes: VecDeque<usize>,
    pub color_texture_size: UVec3,
}

pub struct VoxelWorldPlugin;

impl Plugin for VoxelWorldPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        let render_device = app.world.resource::<RenderDevice>();
        let render_queue = app.world.resource::<RenderQueue>();

        // brickmap settings
        let world_depth = 10;
        let color_texture_size = UVec3::splat(640);
        let brickmap_max_nodes = 1 << 16;

        // load world (slooowwww)
        let path = PathBuf::from("assets/worlds/imperial_city");
        let mut cpu_brickmap = load_anvil(path, world_depth);
        cpu_brickmap.recreate_mipmaps();

        // setup gpu brickmap
        let mut brickmap = vec![BRICK_OFFSET; 8 * brickmap_max_nodes];
        let mut gpu_to_cpu = vec![0; 8 * brickmap_max_nodes];
        for i in 0..8 {
            brickmap[i] = BRICK_OFFSET + 1;
            gpu_to_cpu[i] = i as u32;
        }
        let dim = color_texture_size / BRICK_SIZE;
        let brick_count = (dim.x * dim.y * dim.z) as usize;
        let gpu_voxel_world = GpuVoxelWorld {
            brickmap,
            gpu_to_cpu,
            brickmap_holes: (1..brickmap_max_nodes).collect::<VecDeque<usize>>(),
            brick_holes: (1..brick_count).collect::<VecDeque<usize>>(),
            color_texture_size,
        };

        // uniforms
        let voxel_uniforms = VoxelUniforms {
            brickmap_depth: world_depth - BRICK_SIZE.trailing_zeros(),
            brick_size: BRICK_SIZE.trailing_zeros(),
            brick_ints: Brick::brick_ints() as u32,
        };
        let mut uniform_buffer = UniformBuffer::from(voxel_uniforms.clone());
        uniform_buffer.write_buffer(render_device, render_queue);

        // brickmap
        let (_, brickmap, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
        let brickmap = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: brickmap,
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // counters
        let counters_data = vec![0; brickmap_max_nodes * COUNTER_BITS]; // * 8 / 8
        let counters_gpu = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: &counters_data,
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        });
        let counters_cpu = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: &counters_data,
            label: None,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        });

        // bricks
        let bricks = vec![0; 4 * Brick::brick_ints() * brick_count];
        let bricks = render_device.create_buffer_with_data(&BufferInitDescriptor {
            contents: &bricks,
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // color
        let texture_length = color_texture_size.x * color_texture_size.y * color_texture_size.z;
        let color = vec![0; 4 * texture_length as usize];
        let color = render_device.create_texture_with_data(
            render_queue,
            &TextureDescriptor {
                label: None,
                view_formats: &[TextureFormat::Rgba8Unorm],
                size: Extent3d {
                    width: color_texture_size.x,
                    height: color_texture_size.y,
                    depth_or_array_layers: color_texture_size.z,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D3,
                format: TextureFormat::Rgba8Unorm,
                usage: TextureUsages::STORAGE_BINDING,
            },
            &color,
        );

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
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(4),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: false },
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(4),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 3,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(512),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 4,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadOnly,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D3,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group = render_device.create_bind_group(
            Some("voxel bind group"),
            &bind_group_layout,
            &[
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
                    resource: counters_gpu.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: bricks.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(
                        &color.create_view(&TextureViewDescriptor::default()),
                    ),
                },
            ],
        );

        app.sub_app_mut(RenderApp)
            .insert_resource(voxel_uniforms)
            .insert_resource(VoxelData {
                uniform_buffer,
                brickmap,
                counters_gpu,
                counters_cpu,
                bricks,
                color,
                bind_group_layout,
                bind_group,
            })
            .insert_resource(CpuVoxelWorld(cpu_brickmap))
            .insert_resource(gpu_voxel_world)
            .add_systems(Render, prepare_uniforms.in_set(RenderSet::Prepare))
            .add_systems(Render, queue_bind_group.in_set(RenderSet::Queue));
    }
}

#[derive(Resource)]
pub struct VoxelData {
    pub uniform_buffer: UniformBuffer<VoxelUniforms>,
    pub brickmap: Buffer,
    pub counters_gpu: Buffer,
    pub counters_cpu: Buffer,
    pub bricks: Buffer,
    pub color: Texture,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}

#[derive(Resource, ExtractResource, Clone, ShaderType)]
pub struct VoxelUniforms {
    brickmap_depth: u32,
    brick_size: u32,
    brick_ints: u32,
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
    let bind_group = render_device.create_bind_group(
        Some("voxel bind group"),
        &voxel_data.bind_group_layout,
        &[
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
                resource: voxel_data.counters_gpu.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 3,
                resource: voxel_data.bricks.as_entire_binding(),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::TextureView(
                    &voxel_data
                        .color
                        .create_view(&TextureViewDescriptor::default()),
                ),
            },
        ],
    );
    voxel_data.bind_group = bind_group;
}
