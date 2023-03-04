use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        RenderApp, RenderStage,
    },
    utils::HashMap,
};

pub struct VoxelWorldPlugin;

impl Plugin for VoxelWorldPlugin {
    fn build(&self, app: &mut App) {
        let render_device = app.world.resource::<RenderDevice>();
        let render_queue = app.world.resource::<RenderQueue>();

        // uniforms
        // let voxel_uniforms = VoxelUniforms {
        //     pallete: gh.pallete.into(),
        //     portals: [ExtractedPortal::default(); 32],
        //     levels,
        //     offsets,
        //     texture_size,
        // };
        // let mut uniform_buffer = UniformBuffer::from(voxel_uniforms.clone());
        // uniform_buffer.write_buffer(render_device, render_queue);

        // initialize bricks texture
        let mut texture_data = vec![0; 256 * 256 * 256 * 4];

        // load mc palette
        let file = std::fs::File::open("palette/blockstates.json");
        let mut json: HashMap<String, [u8; 4]> = serde_json::from_reader(file.unwrap()).unwrap();
        json.insert("minecraft:grass".to_string(), [62, 204, 18, 255]);
        json.insert("minecraft:grass_block".to_string(), [62, 204, 18, 255]);
        json.insert("minecraft:water".to_string(), [20, 105, 201, 255]);
        json.insert("minecraft:cave_air".to_string(), [0, 0, 0, 0]);
        json.insert("minecraft:lava".to_string(), [255, 123, 0, 0]);
        json.insert("minecraft:seagrass".to_string(), [62, 204, 18, 0]);
        json.insert("minecraft:tall_grass".to_string(), [0, 255, 0, 0]);
        json.insert("minecraft:deepslate".to_string(), [77, 77, 77, 0]);
        json.insert("minecraft:oak_log".to_string(), [112, 62, 8, 0]);
        json.insert("minecraft:oak_stairs".to_string(), [112, 62, 8, 0]);

        // load chunks into the texture
        use fastanvil::{CurrentJavaChunk, Region};
        use fastnbt::from_bytes;

        let file = std::fs::File::open("/Users/brian/Documents/MultiMC/Fabric 1.19.3/.minecraft/saves/anvil test world/region/r.0.0.mca").unwrap();
        let mut region = Region::from_stream(file).unwrap();

        for chunk_x in 0..16 {
            for chunk_z in 0..16 {
                let data = region.read_chunk(chunk_x, chunk_z).unwrap().unwrap();
                let chunk: CurrentJavaChunk = from_bytes(data.as_slice()).unwrap();
                let section_tower = chunk.sections.unwrap();

                for chunk_y in 0..16 {
                    let section = section_tower.get_section_for_y(chunk_y * 16 - 64).unwrap();
                    let block_states = &section.block_states;

                    for x in 0..16 {
                        for y in 0..16 {
                            for z in 0..16 {
                                let tx = chunk_x as usize * 16 + x;
                                let ty = chunk_y as usize * 16 + y;
                                let tz = chunk_z as usize * 16 + z;

                                let block = block_states.at(x, y, z);
                                if block.unwrap().name() == "minecraft:air" {
                                    continue;
                                }

                                // let block_encoded = block.unwrap().encoded_description();
                                // let block_name =
                                //     block_encoded.strip_suffix("|").unwrap_or(block_encoded);
                                let block_name = block.unwrap().name();
                                let colour = json.get(block_name).unwrap_or(&[255, 0, 0, 0]);

                                match json.get(block_name) {
                                    Some(_) => {},
                                    None => {
                                        println!("{} not found", block_name);
                                    }
                                }

                                let index = 4 * (tz * 65536 + ty * 256 + tx);
                                texture_data[index] = colour[0];
                                texture_data[index + 1] = colour[1];
                                texture_data[index + 2] = colour[2];
                                texture_data[index + 3] = colour[3];
                            }
                        }
                    }
                }
            }
        }

        // texture
        let bricks = render_device.create_texture_with_data(
            render_queue,
            &TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: 256,
                    height: 256,
                    depth_or_array_layers: 256,
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
            contents: &vec![0; 10],
            label: None,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        let bind_group_layout =
            render_device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("voxelization bind group layout"),
                entries: &[
                    // BindGroupLayoutEntry {
                    //     binding: 0,
                    //     visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                    //     ty: BindingType::Buffer {
                    //         ty: BufferBindingType::Uniform,
                    //         has_dynamic_offset: false,
                    //         min_binding_size: BufferSize::new(VoxelUniforms::SHADER_SIZE.into()),
                    //     },
                    //     count: None,
                    // },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT | ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadWrite,
                            format: TextureFormat::Rgba8Unorm,
                            view_dimension: TextureViewDimension::D3,
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
                ],
            });

        let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &bind_group_layout,
            entries: &[
                // BindGroupEntry {
                //     binding: 0,
                //     resource: uniform_buffer.binding().unwrap(),
                // },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&bricks),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: brickmap.as_entire_binding(),
                },
            ],
        });

        app.sub_app_mut(RenderApp)
            .insert_resource(VoxelData {
                bricks,
                brickmap,
                bind_group_layout,
                bind_group,
            })
            .add_system_to_stage(RenderStage::Queue, queue_bind_group);
    }
}

#[derive(Resource)]
pub struct VoxelData {
    // pub uniform_buffer: UniformBuffer<VoxelUniforms>,
    pub bricks: TextureView,
    pub brickmap: Buffer,
    pub bind_group_layout: BindGroupLayout,
    pub bind_group: BindGroup,
}

// #[derive(Resource, ExtractResource, Clone, ShaderType)]
// pub struct VoxelUniforms {
//     pub pallete: [PalleteEntry; 256],
//     pub portals: [ExtractedPortal; 32],
//     pub levels: [UVec4; 8],
//     pub offsets: [UVec4; 8],
//     pub texture_size: u32,
// }

// fn prepare_uniforms(
//     voxel_uniforms: Res<VoxelUniforms>,
//     mut voxel_data: ResMut<VoxelData>,
//     render_device: Res<RenderDevice>,
//     render_queue: Res<RenderQueue>,
// ) {
//     voxel_data.uniform_buffer.set(voxel_uniforms.clone());
//     voxel_data
//         .uniform_buffer
//         .write_buffer(&render_device, &render_queue);
// }

fn queue_bind_group(render_device: Res<RenderDevice>, mut voxel_data: ResMut<VoxelData>) {
    let bind_group = render_device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &voxel_data.bind_group_layout,
        entries: &[
            // BindGroupEntry {
            //     binding: 0,
            //     resource: voxel_data.uniform_buffer.binding().unwrap(),
            // },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(&voxel_data.bricks),
            },
            BindGroupEntry {
                binding: 2,
                resource: voxel_data.brickmap.as_entire_binding(),
            },
        ],
    });
    voxel_data.bind_group = bind_group;
}
