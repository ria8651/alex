pub use self::{
    voxel_streaming::{StreamingSettings, VoxelStreamingCamera},
    voxel_world::VoxelWorldStatsResource,
};

use self::{
    voxel_render::VoxelRenderPlugin, voxel_streaming::VoxelStreamingPlugin,
    voxel_world::VoxelWorldPlugin,
};
use bevy::{
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        view::NoFrustumCulling,
    },
};
use std::path::PathBuf;

mod cpu_brickmap;
mod gpu_brickmap;
mod load_anvil;
mod voxel_render;
mod voxel_streaming;
mod voxel_world;

pub const BRICK_SIZE: u32 = 16;
pub const BRICK_OFFSET: u32 = 1 << 31;
pub const COUNTER_BITS: usize = 32;

#[derive(Component, ExtractComponent, Clone, Default)]
pub struct VoxelVolume {
    pub path: PathBuf,
}

#[derive(Bundle, Default)]
pub struct VoxelVolumeBundle {
    pub voxel_volume: VoxelVolume,
    pub visibility: Visibility,
    pub inherited_visibility: InheritedVisibility,
    pub view_visibility: ViewVisibility,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub no_frustum_culling: NoFrustumCulling,
}

pub struct VoxelPlugin;

impl Plugin for VoxelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            VoxelWorldPlugin,
            VoxelRenderPlugin,
            VoxelStreamingPlugin,
            ExtractComponentPlugin::<VoxelVolume>::default(),
        ));
    }
}
