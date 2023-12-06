pub use self::{voxel_streaming::StreamingSettings, voxel_world::VoxelWorldStatsResource};

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

mod cpu_brickmap;
mod gpu_brickmap;
mod load_anvil;
mod voxel_render;
mod voxel_streaming;
mod voxel_world;

pub const BRICK_SIZE: u32 = 16;
pub const BRICK_OFFSET: u32 = 1 << 31;
pub const COUNTER_BITS: usize = 32;

/// A voxel volume that can be rendered. `streaming_pos` has to be kept updated
/// to the camera position for streaming to work.
#[derive(Component, ExtractComponent, Clone, Reflect)]
pub struct VoxelVolume {
    pub streaming_pos: Vec3,
    pub sort: bool,
    pub sort_reverse: bool,
}

impl Default for VoxelVolume {
    fn default() -> Self {
        Self {
            streaming_pos: Default::default(),
            sort: true,
            sort_reverse: false,
        }
    }
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
