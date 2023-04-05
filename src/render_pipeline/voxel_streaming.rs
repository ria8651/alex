use super::{
    voxel_world::{CpuVoxelWorld, GpuVoxelWorld, VoxelData},
    MainPassSettings,
};
use bevy::{
    prelude::*,
    render::{renderer::RenderQueue, view::ExtractedView, RenderApp, RenderSet},
};
use std::num::NonZeroU32;
use wgpu::ImageCopyTexture;

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
        node_index: usize,
        pos: UVec3,
        depth: u32,
        brickmap_depth: u32,
        streaming_pos: Vec3,
        nodes_to_divide: &mut Vec<(usize, UVec3, u32)>,
        nodes_to_cull: &mut Vec<usize>,
        streaming_ratio: f32,
        streaming_range: f32,
    ) {
        let node_size = (1 << brickmap_depth - depth) as f32;
        let distance = (pos.as_vec3() + node_size / 2.0 - streaming_pos).length();
        let ratio = node_size / distance;

        let children_index = 8 * (brickmap[node_index] as usize & 0xFFFF);
        if children_index == 0 {
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
            nodes_to_cull.push(node_index);
            return;
        }

        for i in 0..8 {
            let half_size = 1 << brickmap_depth - depth - 1;
            let pos = pos + UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * half_size;
            let index = children_index + i as usize;
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
            i as usize,
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

    // subdivision
    let mut divide_node = |index: usize, pos: UVec3, depth: u32| {
        let node = gpu_voxel_world.brickmap[index];
        if node & 0xFFFF != 0 {
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
            gpu_voxel_world.brickmap[hole.unwrap() * 8 + i] = 0;

            let cpu_child_node = cpu_voxel_world.brickmap[8 * cpu_node.children as usize + i];
            let cpu_child_brick_index = cpu_child_node.brick;
            if cpu_child_brick_index != 0 {
                let brick_index = gpu_voxel_world.brick_holes.pop_front();
                if brick_index.is_none() {
                    warn!("ran out of space in brickmap");
                    return;
                }

                let dim = gpu_voxel_world.brick_texture_size / 16;
                let brick_pos = UVec3::new(
                    brick_index.unwrap() as u32 / (dim.x * dim.y),
                    brick_index.unwrap() as u32 / dim.x % dim.y,
                    brick_index.unwrap() as u32 % dim.x,
                ) * 16;

                let cpu_brick = &cpu_voxel_world.bricks[cpu_child_brick_index as usize];
                render_queue.write_texture(
                    ImageCopyTexture {
                        texture: &voxel_data.bricks,
                        origin: wgpu::Origin3d {
                            x: brick_pos.x,
                            y: brick_pos.y,
                            z: brick_pos.z,
                        },
                        mip_level: 0,
                        aspect: wgpu::TextureAspect::All,
                    },
                    cpu_brick.to_gpu(),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(NonZeroU32::new(16 * 4).unwrap()),
                        rows_per_image: Some(NonZeroU32::new(16).unwrap()),
                    },
                    wgpu::Extent3d {
                        width: 16,
                        height: 16,
                        depth_or_array_layers: 16,
                    },
                );

                gpu_voxel_world.brickmap[hole.unwrap() * 8 + i] |=
                    (brick_index.unwrap() as u32) << 16;
            }
        }

        gpu_voxel_world.brickmap[index] |= hole.unwrap() as u32;
    };

    for (index, pos, depth) in nodes_to_divide {
        divide_node(index, pos, depth);
    }

    // culling
    fn cull_node(index: usize, gpu_voxel_world: &mut GpuVoxelWorld) {
        let node = gpu_voxel_world.brickmap[index];
        if node & 0xFFFF == 0 {
            warn!("node {} already culled", index);
            return;
        }

        let children_index = 8 * (node & 0xFFFF) as usize;
        for i in 0..8 {
            let child_node = gpu_voxel_world.brickmap[children_index + i];
            let child_node_brick_index = child_node >> 16;
            if child_node_brick_index != 0 {
                gpu_voxel_world
                    .brick_holes
                    .push_back(child_node_brick_index as usize);
            }
            let child_node_children_index = 8 * (child_node & 0xFFFF) as usize;
            if child_node_children_index != 0 {
                cull_node(child_node_children_index, gpu_voxel_world);
            }
        }

        gpu_voxel_world.brickmap[index] &= 0xFFFF0000;
        gpu_voxel_world.brickmap_holes.push_back(children_index / 8);
    }

    for node_index in nodes_to_cull {
        cull_node(node_index, &mut gpu_voxel_world);
    }

    let (_, data, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
    render_queue.write_buffer(&voxel_data.brickmap, 0, data);

    // println!("brickmap holes: {:?}", gpu_voxel_world.brickmap_holes.len());
    // println!("brick holes: {:?}", gpu_voxel_world.brick_holes.len());
}
