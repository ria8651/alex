use super::{
    gpu_brickmap::GpuVoxelWorld,
    voxel_world::{CpuVoxelWorld, VoxelData},
};
use crate::render_pipeline::cpu_brickmap::{COUNTER_BITS, BRICK_OFFSET};
use bevy::{
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        renderer::RenderQueue,
        view::ExtractedView,
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
            streaming_ratio: 1.0,
        }
    }
}

pub struct VoxelStreamingPlugin;

impl Plugin for VoxelStreamingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ExtractResourcePlugin::<StreamingSettings>::default(),
            ExtractComponentPlugin::<VoxelStreamingCamera>::default(),
        ))
        .insert_resource(StreamingSettings::default());

        app.sub_app_mut(RenderApp)
            .add_systems(Render, voxel_streaming_system.in_set(RenderSet::Prepare));
    }
}

#[derive(Component, Clone, ExtractComponent)]
pub struct VoxelStreamingCamera;

fn voxel_streaming_system(
    voxel_data: Res<VoxelData>,
    render_queue: Res<RenderQueue>,
    // render_device: Res<RenderDevice>,
    cpu_voxel_world: Res<CpuVoxelWorld>,
    mut gpu_voxel_world: ResMut<GpuVoxelWorld>,
    streaming_settings: Res<StreamingSettings>,
    character: Query<&ExtractedView, With<VoxelStreamingCamera>>,
    mut done: Local<bool>,
) {
    if streaming_settings.pause_streaming {
        return;
    }

    if !*done {
        for index in 0..8 {
            if let Err(e) =
                gpu_voxel_world.divide_node(index, &voxel_data, &cpu_voxel_world, &render_queue)
            {
                warn!("failed to cull node: {}", e);
            }
        }

        for i in 0..64 {
            let val = gpu_voxel_world.brickmap[i];
            if val >= BRICK_OFFSET {
                println!("{}: ({})", i, gpu_voxel_world.brickmap[i] - BRICK_OFFSET);
            } else {
                println!("{}: {}", i, gpu_voxel_world.brickmap[i]);
            }
            if i % 8 == 7 {
                println!();
            }
        }

        *done = true;
    }

    // get brick hit counters
    // let counter_slice = voxel_data.counters.slice(..);
    // counter_slice.map_async(wgpu::MapMode::Read, |_| {});
    // render_device.poll(wgpu::Maintain::Wait);

    // let data = counter_slice.get_mapped_range();
    // let (head, result, tail) = unsafe { data.align_to::<u32>() };
    // assert!(head.is_empty() && tail.is_empty());

    // let extracted_view = character.single();
    // let streaming_pos =
    //     extracted_view.transform.translation() + (1 << cpu_voxel_world.brickmap_depth - 1) as f32;

    // collect the nodes that need to be updated
    // let mut nodes_to_divide = Vec::new();
    // let mut nodes_to_cull = Vec::new();
    // gpu_voxel_world.recursive_search(&mut |index, pos, depth| {
    //     let node_size = (1 << cpu_voxel_world.brickmap_depth - depth) as f32;
    //     let distance =
    //         (pos.as_vec3() + node_size / 2.0 - streaming_pos).length() * BRICK_SIZE as f32;
    //     let ratio = 100.0 * node_size / distance;

    //     let children_index = gpu_voxel_world.brickmap[index];
    //     if children_index >= BRICK_OFFSET {
    //         if ratio > streaming_settings.streaming_ratio {
    //             let cpu_node_index = gpu_voxel_world.gpu_to_cpu[index] as usize;
    //             let cpu_node = cpu_voxel_world.brickmap[cpu_node_index];
    //             if cpu_node.children != 0 {
    //                 nodes_to_divide.push((index, pos, depth));
    //             }
    //         }
    //         return;
    //     }
    //     if ratio < streaming_settings.streaming_ratio {
    //         nodes_to_cull.push((index, pos, depth));
    //     }
    // });

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

    // let my_span = info_span!("division").entered();
    // for (index, _, _) in nodes_to_divide {
    //     if let Err(e) =
    //         gpu_voxel_world.divide_node(index, &voxel_data, &cpu_voxel_world, &render_queue)
    //     {
    //         warn!("failed to divide node: {}", e);
    //     }
    // }
    // drop(my_span);

    // let my_span = info_span!("culling").entered();
    // for (index, _, _) in nodes_to_cull {
    //     if let Err(e) =
    //         gpu_voxel_world.cull_node(index, &voxel_data, &cpu_voxel_world, &render_queue)
    //     {
    //         warn!("failed to cull node: {}", e);
    //     }
    // }
    // drop(my_span);

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
