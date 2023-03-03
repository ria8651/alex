use super::{MainPassPipelineData, ViewMainPassUniformBuffer};
use crate::render_pipeline::{voxel_world::VoxelData, RenderGraphSettings};
use bevy::{
    prelude::*,
    render::{
        render_graph::{self, SlotInfo, SlotType},
        render_resource::*,
        view::{ExtractedView, ViewTarget},
    },
};

pub struct MainPassNode {
    query:
        QueryState<(&'static ViewTarget, &'static ViewMainPassUniformBuffer), With<ExtractedView>>,
}

impl MainPassNode {
    pub fn new(world: &mut World) -> Self {
        Self {
            query: world.query_filtered(),
        }
    }
}

impl render_graph::Node for MainPassNode {
    fn input(&self) -> Vec<SlotInfo> {
        vec![SlotInfo::new("view", SlotType::Entity)]
    }

    fn update(&mut self, world: &mut World) {
        self.query.update_archetypes(world);
    }

    fn run(
        &self,
        graph: &mut render_graph::RenderGraphContext,
        render_context: &mut bevy::render::renderer::RenderContext,
        world: &World,
    ) -> Result<(), render_graph::NodeRunError> {
        let view_entity = graph.get_input_entity("view")?;
        let pipeline_cache = world.resource::<PipelineCache>();
        let voxel_data = world.get_resource::<VoxelData>().unwrap();
        let pipeline_data = world.get_resource::<MainPassPipelineData>().unwrap();
        let render_graph_settings = world.get_resource::<RenderGraphSettings>().unwrap();

        if !render_graph_settings.trace {
            return Ok(());
        }

        let (target, uniform_buffer) = match self.query.get_manual(world, view_entity) {
            Ok(result) => result,
            Err(_) => panic!("Voxel camera missing component!"),
        };

        let trace_pipeline = match pipeline_cache.get_render_pipeline(pipeline_data.pipeline_id) {
            Some(pipeline) => pipeline,
            None => return Ok(()),
        };

        let bind_group = render_context
            .render_device
            .create_bind_group(&BindGroupDescriptor {
                label: Some("main pass bind group"),
                layout: &pipeline_data.bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.binding().unwrap(),
                }],
            });

        let render_pass_descriptor = RenderPassDescriptor {
            label: Some("main pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target.main_texture(),
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        };

        let mut render_pass = render_context
            .command_encoder
            .begin_render_pass(&render_pass_descriptor);

        render_pass.set_bind_group(0, &voxel_data.bind_group, &[]);
        render_pass.set_bind_group(1, &bind_group, &[]);

        render_pass.set_pipeline(trace_pipeline);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
