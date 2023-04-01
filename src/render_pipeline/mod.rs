use self::{
    main_pass::{MainPassNode, MainPassPlugin},
    voxel_streaming::VoxelStreamingPlugin,
    voxel_world::VoxelWorldPlugin,
};
use bevy::{
    core_pipeline::{
        bloom::BloomNode, fxaa::FxaaNode, tonemapping::TonemappingNode, upscaling::UpscalingNode,
    },
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_graph::{RenderGraph, SlotInfo, SlotType},
        RenderApp,
    },
    ui::UiPassNode,
};
pub use main_pass::MainPassSettings;

mod cpu_brickmap;
mod load_anvil;
mod main_pass;
mod voxel_streaming;
mod voxel_world;

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderGraphSettings::default())
            .add_plugin(ExtractResourcePlugin::<RenderGraphSettings>::default())
            .add_plugin(VoxelStreamingPlugin)
            .add_plugin(VoxelWorldPlugin)
            .add_plugin(MainPassPlugin);

        let render_app = match app.get_sub_app_mut(RenderApp) {
            Ok(render_app) => render_app,
            Err(_) => return,
        };

        // build voxel render graph
        let mut voxel_graph = RenderGraph::default();
        let input_node_id =
            voxel_graph.set_input(vec![SlotInfo::new("view_entity", SlotType::Entity)]);

        // render graph
        let main_pass = MainPassNode::new(&mut render_app.world);
        let bloom = BloomNode::new(&mut render_app.world);
        let tonemapping = TonemappingNode::new(&mut render_app.world);
        let fxaa = FxaaNode::new(&mut render_app.world);
        let ui = UiPassNode::new(&mut render_app.world);
        let upscaling = UpscalingNode::new(&mut render_app.world);

        voxel_graph.add_node("main_pass", main_pass);
        voxel_graph.add_node("bloom", bloom);
        voxel_graph.add_node("tonemapping", tonemapping);
        voxel_graph.add_node("fxaa", fxaa);
        voxel_graph.add_node("ui", ui);
        voxel_graph.add_node("upscaling", upscaling);
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "main_pass", "view");
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "bloom", "view");
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "tonemapping", "view");
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "fxaa", "view");
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "ui", "view");
        voxel_graph.add_slot_edge(input_node_id, "view_entity", "upscaling", "view");
        voxel_graph.add_node_edge("main_pass", "bloom");
        voxel_graph.add_node_edge("bloom", "tonemapping");
        voxel_graph.add_node_edge("tonemapping", "fxaa");
        voxel_graph.add_node_edge("fxaa", "ui");
        voxel_graph.add_node_edge("ui", "upscaling");

        // insert the voxel graph into the main render graph
        let mut graph = render_app.world.resource_mut::<RenderGraph>();
        graph.add_sub_graph("voxel", voxel_graph);
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
