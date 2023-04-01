use super::voxel_world::{VoxelData, GpuVoxelWorld};
use bevy::{
    prelude::*,
    render::{
        renderer::{RenderDevice, RenderQueue},
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
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
) {
    // load up the brickmap for editing
    let brickmap_slice = voxel_data.brickmap.slice(..);
    brickmap_slice.map_async(wgpu::MapMode::Write, |_| {});
    render_device.poll(wgpu::Maintain::Wait);

    let mut data = brickmap_slice.get_mapped_range_mut();
    let (head, result, tail) = unsafe { data.align_to_mut::<u32>() };
    assert!(head.is_empty());
    assert!(tail.is_empty());

    // subdivision
    let mut subdivide_node = |index: usize| {
        let node = result[index];
        if node & 0xFFFF != 0 {
            warn!("node {} already subdivided", index);
            return;
        }

        let hole = gpu_voxel_world.brickmap_holes.pop_front();
        if hole.is_none() {
            warn!("ran out of space in brickmap");
            return;
        }

        result[index] |= hole.unwrap() as u32;
    };

    subdivide_node(3);

    // testing editing the brick texture
    render_queue.write_texture(
        ImageCopyTexture {
            texture: &voxel_data.bricks,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 16 },
            mip_level: 0,
            aspect: wgpu::TextureAspect::All,
        },
        &vec![255; 16 * 16 * 16 * 4],
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

    drop(data);
    voxel_data.brickmap.unmap();
}
