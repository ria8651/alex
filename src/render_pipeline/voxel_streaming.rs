use std::num::NonZeroU32;

use super::{
    voxel_world::{CpuVoxelWorld, GpuVoxelWorld, VoxelData},
    MainPassSettings,
};
use crate::render_pipeline::cpu_brickmap::{Brick, BRICK_SIZE};
use bevy::{
    prelude::*,
    render::{renderer::RenderQueue, view::ExtractedView, RenderApp, RenderSet},
};
use wgpu::ImageCopyTexture;

pub const BRICK_OFFSET: u32 = 1 << 31;

pub struct VoxelStreamingPlugin;

impl Plugin for VoxelStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.sub_app_mut(RenderApp)
            .add_system(voxel_streaming_system.in_set(RenderSet::Prepare));
    }
}

fn voxel_streaming_system(
    voxel_data: Res<VoxelData>,
    render_queue: Res<RenderQueue>,
    cpu_voxel_world: Res<CpuVoxelWorld>,
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
    character: Query<(&ExtractedView, &MainPassSettings)>,
) {
    let (cam_pos, main_pass_settings) = character.single();
    let streaming_pos =
        cam_pos.transform.translation() + (1 << cpu_voxel_world.brickmap_depth - 1) as f32;

    // find the nodes that need updating
    fn recursive_search(
        brickmap: &mut [u32],
        cpu_voxel_world: &CpuVoxelWorld,
        node_index: u32,
        pos: UVec3,
        depth: u32,
        brickmap_depth: u32,
        streaming_pos: Vec3,
        nodes_to_divide: &mut Vec<(u32, UVec3, u32)>,
        nodes_to_cull: &mut Vec<(u32, UVec3, u32)>,
        streaming_ratio: f32,
        streaming_range: f32,
    ) {
        let node_size = (1 << brickmap_depth - depth) as f32;
        let distance =
            (pos.as_vec3() + node_size / 2.0 - streaming_pos).length() * BRICK_SIZE as f32;
        let ratio = 100.0 * node_size / distance;

        let children_index = brickmap[node_index as usize];
        if children_index >= BRICK_OFFSET {
            if ratio > streaming_ratio + streaming_range {
                let (cpu_node_index, _, _) = cpu_voxel_world.get_node(pos, Some(depth));
                let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
                if cpu_node.children != 0 {
                    nodes_to_divide.push((node_index, pos, depth));
                }
            }
            return;
        }
        if ratio < streaming_ratio - streaming_range {
            nodes_to_cull.push((node_index, pos, depth));
            // return;
        }

        for i in 0..8 {
            let half_size = 1 << brickmap_depth - depth - 1;
            let pos = pos + UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * half_size;
            let index = 8 * children_index + i;
            recursive_search(
                brickmap,
                cpu_voxel_world,
                index,
                pos,
                depth + 1,
                brickmap_depth,
                streaming_pos,
                nodes_to_divide,
                nodes_to_cull,
                streaming_ratio,
                streaming_range,
            );
        }
    }

    let mut nodes_to_divide = Vec::new();
    let mut nodes_to_cull = Vec::new();
    for i in 0..8 {
        let pos =
            UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * (1 << cpu_voxel_world.brickmap_depth - 1);
        recursive_search(
            &mut gpu_voxel_world.brickmap,
            &cpu_voxel_world,
            i,
            pos,
            1,
            cpu_voxel_world.brickmap_depth,
            streaming_pos,
            &mut nodes_to_divide,
            &mut nodes_to_cull,
            main_pass_settings.streaming_ratio,
            main_pass_settings.streaming_range,
        );
    }

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
    let mut divide_node = |index: usize, pos: UVec3, depth: u32| {
        let node = gpu_voxel_world.brickmap[index];
        if node < BRICK_OFFSET {
            warn!("node {} already divided", index);
            return;
        }

        let (cpu_node_index, _, cpu_node_depth) = cpu_voxel_world.get_node(pos, Some(depth));
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        if cpu_node_depth != depth {
            warn!("tried to divide node that doesn't exist on cpu");
            return;
        }
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

            let cpu_child_node = cpu_voxel_world.brickmap[8 * cpu_node.children as usize + i];
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
            }
        }

        gpu_voxel_world.brickmap[index] = hole.unwrap() as u32;
    };

    for (index, pos, depth) in nodes_to_divide {
        divide_node(index as usize, pos, depth);
    }

    // culling
    let mut cull_node = |index: usize, pos: UVec3, depth: u32| {
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

        let (cpu_node_index, _, cpu_node_depth) = cpu_voxel_world.get_node(pos, Some(depth));
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        if cpu_node_depth != depth {
            warn!("tried to divide node that doesn't exist on cpu");
            return;
        }

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

    for (index, pos, depth) in nodes_to_cull {
        cull_node(index as usize, pos, depth);
    }

    // println!(
    //     "{} brick holes, {} node holes",
    //     gpu_voxel_world.brickmap_holes.len(),
    //     gpu_voxel_world.brickmap_holes.len()
    // );

    let (_, data, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
    render_queue.write_buffer(&voxel_data.brickmap, 0, data);
}
