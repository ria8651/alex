use super::voxel_world::{CpuVoxelWorld, GpuVoxelWorld, VoxelData};
use crate::render_pipeline::cpu_brickmap::{Brick, BRICK_SIZE, COUNTER_BITS};
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        renderer::{RenderDevice, RenderQueue},
        RenderApp, RenderSet,
    },
};
use std::num::NonZeroU32;
use wgpu::ImageCopyTexture;

pub const BRICK_OFFSET: u32 = 1 << 31;

#[derive(Resource, ExtractResource, Clone)]
pub struct StreamingSettings {
    pub pause_streaming: bool,
    pub streaming_value: u32,
}

impl Default for StreamingSettings {
    fn default() -> Self {
        Self {
            pause_streaming: false,
            streaming_value: 50,
        }
    }
}

pub struct VoxelStreamingPlugin;

impl Plugin for VoxelStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ExtractResourcePlugin::<StreamingSettings>::default())
            .insert_resource(StreamingSettings::default());
        
        app.sub_app_mut(RenderApp)
            .add_system(voxel_streaming_system.in_set(RenderSet::Prepare));
    }
}

fn voxel_streaming_system(
    voxel_data: Res<VoxelData>,
    render_queue: Res<RenderQueue>,
    render_device: Res<RenderDevice>,
    cpu_voxel_world: Res<CpuVoxelWorld>,
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
    streaming_settings: Res<StreamingSettings>,
) {
    if streaming_settings.pause_streaming {
        return;
    }

    let counter_slice = voxel_data.counters.slice(..);
    counter_slice.map_async(wgpu::MapMode::Read, |_| {});
    render_device.poll(wgpu::Maintain::Wait);

    let data = counter_slice.get_mapped_range();
    let (head, result, tail) = unsafe { data.align_to::<u32>() };
    assert!(head.is_empty() && tail.is_empty());

    // collect the nodes that need to be updated
    let mut nodes_to_divide = Vec::new();
    let mut nodes_to_cull = Vec::new();

    for (index, node_counter) in result.iter().enumerate() {
        if *node_counter > streaming_settings.streaming_value {
            if gpu_voxel_world.brickmap[index] > BRICK_OFFSET {
                let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
                if cpu_voxel_world.brickmap[cpu_node_index].children != 0 {
                    nodes_to_divide.push(index);
                }
            }
        }

        if *node_counter == 0 {
            if gpu_voxel_world.brickmap[index] < BRICK_OFFSET {
                nodes_to_cull.push(index);
            }
        }
    }

    drop(data);
    voxel_data.counters.unmap();

    let allocate_brick =
        |brick: &Brick, gpu_voxel_world: &mut GpuVoxelWorld| -> Result<usize, ()> {
            let brick_index = gpu_voxel_world.brick_holes.pop_front();
            if brick_index.is_none() {
                warn!("ran out of space in brickmap");
                return Err(());
            }

            render_queue.write_buffer(
                &voxel_data.bricks,
                (brick_index.unwrap() * 4 * Brick::brick_ints()) as u64,
                &brick.get_bitmask(),
            );

            let dim = gpu_voxel_world.color_texture_size / BRICK_SIZE;
            let brick_pos = UVec3::new(
                brick_index.unwrap() as u32 / (dim.x * dim.y),
                brick_index.unwrap() as u32 / dim.x % dim.y,
                brick_index.unwrap() as u32 % dim.x,
            ) * BRICK_SIZE;
            render_queue.write_texture(
                ImageCopyTexture {
                    texture: &voxel_data.color,
                    origin: wgpu::Origin3d {
                        x: brick_pos.x,
                        y: brick_pos.y,
                        z: brick_pos.z,
                    },
                    mip_level: 0,
                    aspect: wgpu::TextureAspect::All,
                },
                unsafe { brick.to_gpu() },
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(NonZeroU32::new(BRICK_SIZE * 4).unwrap()),
                    rows_per_image: Some(NonZeroU32::new(BRICK_SIZE).unwrap()),
                },
                wgpu::Extent3d {
                    width: BRICK_SIZE,
                    height: BRICK_SIZE,
                    depth_or_array_layers: BRICK_SIZE,
                },
            );

            Ok(brick_index.unwrap())
        };

    // subdivision
    let mut divide_node = |index: usize| {
        let node = gpu_voxel_world.brickmap[index];
        if node < BRICK_OFFSET {
            warn!("node {} already divided", index);
            return;
        }

        let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        if cpu_node.children == 0 {
            warn!("tried to divide node with no children on cpu");
            return;
        }

        let hole = gpu_voxel_world.brickmap_holes.pop_front();
        if hole.is_none() {
            warn!("ran out of space in brickmap");
            return;
        }

        for i in 0..8 {
            gpu_voxel_world.brickmap[hole.unwrap() * 8 + i] = BRICK_OFFSET;

            let cpu_child_node_index = cpu_node.children as usize * 8 + i;
            let cpu_child_node = cpu_voxel_world.brickmap[cpu_child_node_index];
            let cpu_child_brick_index = cpu_child_node.brick;
            if cpu_child_brick_index != 0 {
                let brick_index = match allocate_brick(
                    &cpu_voxel_world.bricks[cpu_child_brick_index as usize],
                    &mut gpu_voxel_world,
                ) {
                    Ok(index) => index,
                    Err(_) => return,
                };
                gpu_voxel_world.brickmap[hole.unwrap() * 8 + i] = BRICK_OFFSET + brick_index as u32;
                gpu_voxel_world.gpu_to_cpu[hole.unwrap() * 8 + i] = cpu_child_node_index as u32;
            }
        }

        gpu_voxel_world.brickmap[index] = hole.unwrap() as u32;
    };

    for index in nodes_to_divide {
        divide_node(index);
    }

    // culling
    let mut cull_node = |index: usize| {
        let node = gpu_voxel_world.brickmap[index];
        if node >= BRICK_OFFSET {
            warn!("node {} already culled", index);
            return;
        }

        let children_index = 8 * node as usize;
        for i in 0..8 {
            let child_node = gpu_voxel_world.brickmap[children_index + i];
            if child_node > BRICK_OFFSET {
                gpu_voxel_world
                    .brick_holes
                    .push_back((child_node - BRICK_OFFSET) as usize);
            }
        }

        let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];

        let brick_index = match allocate_brick(
            &cpu_voxel_world.bricks[cpu_node.brick as usize],
            &mut gpu_voxel_world,
        ) {
            Ok(index) => index,
            Err(_) => return,
        };

        gpu_voxel_world.brickmap[index] = BRICK_OFFSET + brick_index as u32;
        gpu_voxel_world.brickmap_holes.push_back(children_index / 8);
    };

    for index in nodes_to_cull {
        cull_node(index);
    }

    // println!(
    //     "{} brick holes, {} node holes",
    //     gpu_voxel_world.brickmap_holes.len(),
    //     gpu_voxel_world.brickmap_holes.len()
    // );

    let (_, data, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
    render_queue.write_buffer(&voxel_data.brickmap, 0, data);

    let counters = vec![0; gpu_voxel_world.brickmap.len() * COUNTER_BITS / 8];
    render_queue.write_buffer(&voxel_data.counters, 0, &counters);
}
