use super::{
    voxel_world::{CpuVoxelWorld, GpuVoxelWorld, VoxelData},
    MainPassSettings,
};
use bevy::{
    prelude::*,
    render::{
        renderer::{RenderDevice, RenderQueue},
        view::ExtractedView,
        RenderApp, RenderSet,
    },
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
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
    cpu_voxel_world: Res<CpuVoxelWorld>,
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
    character: Query<&ExtractedView, With<MainPassSettings>>,
) {
    let character = character.single();
    let streaming_pos =
        character.transform.translation() + (1 << cpu_voxel_world.brickmap_depth - 1) as f32;

    // load up the brickmap for editing
    let span = info_span!("waiting for gpu", name = "waiting for gpu").entered();
    let brickmap_slice = voxel_data.brickmap.slice(..);
    brickmap_slice.map_async(wgpu::MapMode::Write, |_| {});
    render_device.poll(wgpu::Maintain::Wait);
    drop(span);

    let mut data = brickmap_slice.get_mapped_range_mut();
    let (head, brickmap, tail) = unsafe { data.align_to_mut::<u32>() };
    assert!(head.is_empty());
    assert!(tail.is_empty());

    // find the nodes that need subdividing
    fn recursive_search(
        brickmap: &mut [u32],
        cpu_voxel_world: &CpuVoxelWorld,
        node_index: usize,
        pos: UVec3,
        depth: u32,
        brickmap_depth: u32,
        streaming_pos: Vec3,
        nodes_to_subdivide: &mut Vec<(usize, UVec3, u32)>,
    ) {
        let children_index = 8 * (brickmap[node_index] as usize & 0xFFFF);
        if children_index == 0 {
            let node_size = (1 << brickmap_depth - depth) as f32;
            let distance = (pos.as_vec3() + node_size / 2.0 - streaming_pos).length();
            let ratio = node_size / distance;
            if ratio < 0.25 {
                return;
            }

            let (cpu_node_index, _, _) = cpu_voxel_world.get_node(pos, Some(depth));
            let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
            if cpu_node & 0xFFFF != 0 {
                nodes_to_subdivide.push((node_index, pos, depth));
            }
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
                nodes_to_subdivide,
            );
        }
    }

    let mut nodes_to_subdivide = Vec::new();
    for i in 0..8 {
        let pos =
            UVec3::new(i >> 2 & 1, i >> 1 & 1, i & 1) * (1 << cpu_voxel_world.brickmap_depth - 1);
        recursive_search(
            brickmap,
            &cpu_voxel_world,
            i as usize,
            pos,
            1,
            cpu_voxel_world.brickmap_depth,
            streaming_pos,
            &mut nodes_to_subdivide,
        );
    }

    // subdivision
    let mut subdivide_node = |index: usize, pos: UVec3, depth: u32| {
        let node = brickmap[index];
        if node & 0xFFFF != 0 {
            warn!("node {} already subdivided", index);
            return;
        }

        let (cpu_node_index, _, cpu_node_depth) = cpu_voxel_world.get_node(pos, Some(depth));
        let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        if cpu_node_depth != depth {
            warn!("tried to subdivide node that doesn't exist on cpu");
            return;
        }
        if cpu_node & 0xFFFF == 0 {
            warn!("tried to subdivide node with no children on cpu");
            return;
        }

        let hole = gpu_voxel_world.brickmap_holes.pop_front();
        if hole.is_none() {
            warn!("ran out of space in brickmap");
            return;
        }

        for i in 0..8 {
            let cpu_child_node = cpu_voxel_world.brickmap[8 * (cpu_node & 0xFFFF) as usize + i];
            let cpu_child_brick_index = cpu_child_node >> 16;
            if cpu_child_brick_index != 0 {
                let brick_index = gpu_voxel_world.brickmap_holes.pop_front();
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

                brickmap[hole.unwrap() * 8 + i] |= (brick_index.unwrap() as u32) << 16;
            }
        }

        brickmap[index] |= hole.unwrap() as u32;
    };

    // println!("{:?}", nodes_to_subdivide);

    for (index, pos, depth) in nodes_to_subdivide {
        subdivide_node(index, pos, depth);

        // brickmap[index] &= 0xFFFF;
        // brickmap[index] |= 100 << 16;

        // // let brick_index = gpu_voxel_world.brickmap_holes.pop_front();
        // if brick_index.is_none() {
        //     warn!("ran out of space in brickmap");
        //     return;
        // }

        // let dim = gpu_voxel_world.brick_texture_size / 16;
        // let brick_pos = UVec3::new(
        //     brick_index.unwrap() as u32 / (dim.x * dim.y),
        //     brick_index.unwrap() as u32 / dim.x % dim.y,
        //     brick_index.unwrap() as u32 % dim.x,
        // ) * 16;

        // let (cpu_node_index, _, _) = cpu_voxel_world.get_node(pos, Some(depth));
        // let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
        // let cpu_brick = &cpu_voxel_world.bricks[(cpu_node >> 16) as usize];
        // render_queue.write_texture(
        //     ImageCopyTexture {
        //         texture: &voxel_data.bricks,
        //         origin: wgpu::Origin3d {
        //             x: brick_pos.x,
        //             y: brick_pos.y,
        //             z: brick_pos.z,
        //         },
        //         mip_level: 0,
        //         aspect: wgpu::TextureAspect::All,
        //     },
        //     cpu_brick.to_gpu(),
        //     wgpu::ImageDataLayout {
        //         offset: 0,
        //         bytes_per_row: Some(NonZeroU32::new(16 * 4).unwrap()),
        //         rows_per_image: Some(NonZeroU32::new(16).unwrap()),
        //     },
        //     wgpu::Extent3d {
        //         width: 16,
        //         height: 16,
        //         depth_or_array_layers: 16,
        //     },
        // );

        // brickmap[index] &= 0xFFFF;
        // brickmap[index] |= (brick_index.unwrap() as u32) << 16;
    }

    // let (index, pos, depth) = nodes_to_subdivide[0];
    // subdivide_node(index, pos, depth);

    drop(data);
    voxel_data.brickmap.unmap();
}
