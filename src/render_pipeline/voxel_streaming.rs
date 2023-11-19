use super::{
    gpu_brickmap::GpuVoxelWorld,
    voxel_world::{CpuVoxelWorld, VoxelData},
    VoxelVolume, VoxelWorldStatsResource, BRICK_OFFSET, BRICK_SIZE, COUNTER_BITS,
};
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        renderer::RenderQueue,
        Render, RenderApp, RenderSet,
    },
};

#[derive(Resource, ExtractResource, Clone, Reflect)]
pub struct StreamingSettings {
    pub pause_streaming: bool,
    pub streaming_ratio: f32,
}

impl Default for StreamingSettings {
    fn default() -> Self {
        Self {
            pause_streaming: false,
            streaming_ratio: 0.4,
        }
    }
}

pub struct VoxelStreamingPlugin;

impl Plugin for VoxelStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<StreamingSettings>::default())
            .insert_resource(StreamingSettings::default());

        app.sub_app_mut(RenderApp)
            .add_systems(Render, voxel_streaming_system.in_set(RenderSet::Queue));
    }
}

fn voxel_streaming_system(
    voxel_data: Res<VoxelData>,
    render_queue: Res<RenderQueue>,
    cpu_voxel_world: Res<CpuVoxelWorld>,
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
    streaming_settings: Res<StreamingSettings>,
    voxel_stats: Res<VoxelWorldStatsResource>,
    voxel_volume: Query<&VoxelVolume>,
) {
    if streaming_settings.pause_streaming {
        return;
    }

    // collect the nodes that need to be updated
    let mut nodes_to_divide = Vec::new();
    let mut nodes_to_cull = Vec::new();

    let my_span = info_span!("streaming search").entered();
    // --- ray guided streaming ---
    // get brick hit counters
    // let counter_slice = voxel_data.counters.slice(..);
    // counter_slice.map_async(wgpu::MapMode::Read, |_| {});
    // render_device.poll(wgpu::Maintain::Wait);

    // let data = counter_slice.get_mapped_range();
    // let (head, result, tail) = unsafe { data.align_to::<u32>() };
    // assert!(head.is_empty() && tail.is_empty());

    // // this looks slow but it's actually pretty fast
    // for (index, node_counter) in result.iter().enumerate() {
    //     if *node_counter > streaming_settings.streaming_value
    //         && gpu_voxel_world.brickmap[index] > BRICK_OFFSET
    //     {
    //         let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
    //         if cpu_voxel_world.brickmap[cpu_node_index].children != 0 {
    //             nodes_to_divide.push(index);
    //         }
    //     }

    //     if *node_counter == 0 && gpu_voxel_world.brickmap[index] < BRICK_OFFSET {
    //         nodes_to_cull.push(index);
    //     }
    // }

    // drop(data);
    // voxel_data.counters.unmap();

    // --- distance guided streaming ---
    let mut streaming_pos = voxel_volume.single().streaming_pos;
    streaming_pos += (1 << cpu_voxel_world.brickmap_depth - 1) as f32;

    gpu_voxel_world.recursive_search(&mut |index, pos, depth| {
        let node_size = (1 << cpu_voxel_world.brickmap_depth - depth) as f32;
        let distance =
            (pos.as_vec3() + node_size / 2.0 - streaming_pos).length() * BRICK_SIZE as f32;
        let ratio = 100.0 * node_size / distance;

        let children_index = gpu_voxel_world.brickmap[index];
        if ratio > streaming_settings.streaming_ratio {
            if children_index >= BRICK_OFFSET {
                let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
                let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
                if cpu_node.children != 0 {
                    nodes_to_divide.push((index, pos, depth));
                }
            }
        } else {
            if children_index < BRICK_OFFSET {
                nodes_to_cull.push((index, pos, depth));
            }
        }
    });
    drop(my_span);

    let my_span = info_span!("streaming division").entered();
    for (index, _, _) in nodes_to_divide {
        if let Err(e) =
            gpu_voxel_world.divide_node(index, &voxel_data, &cpu_voxel_world, &render_queue)
        {
            warn!("failed to divide node: {}", e);
        }
    }
    drop(my_span);

    let my_span = info_span!("streaming culling").entered();
    for (index, _, _) in nodes_to_cull {
        if let Err(e) =
            gpu_voxel_world.cull_node(index, &voxel_data, &cpu_voxel_world, &render_queue)
        {
            warn!("failed to cull node: {}", e);
        }
    }
    drop(my_span);

    let mut voxel_stats = voxel_stats.lock().unwrap();
    let dim = gpu_voxel_world.color_texture_size / BRICK_SIZE;
    voxel_stats.nodes = gpu_voxel_world.brickmap.len() - gpu_voxel_world.brickmap_holes.len() * 8;
    voxel_stats.bricks = (dim.x * dim.y * dim.z) as usize - gpu_voxel_world.brick_holes.len();

    let (_, data, _) = unsafe { gpu_voxel_world.brickmap.align_to::<u8>() };
    render_queue.write_buffer(&voxel_data.brickmap, 0, data);

    let counters = vec![0; gpu_voxel_world.brickmap.len() * COUNTER_BITS / 8];
    render_queue.write_buffer(&voxel_data.counters, 0, &counters);
}
