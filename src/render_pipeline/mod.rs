use self::{
    main_pass::{MainPassNode, MainPassPlugin},
    voxel_streaming::VoxelStreamingPlugin,
    voxel_world::VoxelWorldPlugin,
};
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{RenderGraphApp, ViewNodeRunner},
        RenderApp,
    },
};
pub use {main_pass::MainPassSettings, voxel_streaming::StreamingSettings};

mod cpu_brickmap;
mod load_anvil;
mod main_pass;
mod voxel_streaming;
mod voxel_world;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderGraphSettings::default())
            .add_plugins(ExtractResourcePlugin::<RenderGraphSettings>::default())
            .add_plugins(VoxelStreamingPlugin)
            .add_plugins(VoxelWorldPlugin)
            .add_plugins(MainPassPlugin);
    }

    fn finish(&self, app: &mut App) {
        let render_app = app.sub_app_mut(RenderApp);

        render_app
            .add_render_sub_graph("voxel")
            .add_render_graph_node::<ViewNodeRunner<MainPassNode>>("voxel", "beam_pass")
            .add_render_graph_node::<ViewNodeRunner<MainPassNode>>("voxel", "main_pass")
            .add_render_graph_edges("voxel", &["beam_pass", "main_pass"]);
    }
}

#[derive(Resource, Clone, ExtractResource)]
pub struct RenderGraphSettings {
    pub clear: bool,
    pub automata: bool,
    pub animation: bool,
    pub voxelization: bool,
    pub rebuild: bool,
    pub physics: bool,
    pub trace: bool,
    pub denoise: bool,
}

impl Default for RenderGraphSettings {
    fn default() -> Self {
        Self {
            clear: true,
            automata: true,
            animation: true,
            voxelization: true,
            rebuild: true,
            physics: true,
            trace: true,
            denoise: false,
        }
    }
}
