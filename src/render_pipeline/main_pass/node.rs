use super::{
    BeamTexture, DefualtBeamTexture, MainPassPipelineData, MainPassSettings,
    ViewMainPassUniformBuffer,
};
use crate::render_pipeline::{voxel_world::VoxelData, RenderGraphSettings};
use bevy::{
    prelude::*,
    render::{
        render_asset::RenderAssets,
        render_graph::{self, SlotInfo, SlotType},
        render_resource::*,
        view::{ExtractedView, ViewTarget},
    },
};

pub struct MainPassNode {
    query: QueryState<
        (
            &'static ViewTarget,
            &'static ViewMainPassUniformBuffer,
            &'static MainPassSettings,
            &'static BeamTexture,
        ),
        With<ExtractedView>,
    >,
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
        let voxel_data = world.resource::<VoxelData>();
        let pipeline_data = world.resource::<MainPassPipelineData>();
        let render_graph_settings = world.resource::<RenderGraphSettings>();
        let gpu_images = world.resource::<RenderAssets<Image>>();
        let beam_texture_defualt = world.resource::<DefualtBeamTexture>();

        if !render_graph_settings.trace {
            return Ok(());
        }

        let (target, uniform_buffer, main_pass_settings, beam_texture) =
            match self.query.get_manual(world, view_entity) {
                Ok(result) => result,
                Err(_) => panic!("Voxel camera missing component!"),
            };
        
        let beam_texture_filled = *beam_texture.filled.lock().unwrap();
        if !main_pass_settings.beam_optimization && !beam_texture_filled {
            *beam_texture.filled.lock().unwrap() = true;
            return Ok(());
        }

        let trace_pipeline = match pipeline_cache.get_render_pipeline(pipeline_data.pipeline_id) {
            Some(pipeline) => pipeline,
            None => return Ok(()),
        };

        let (beam_texture, target) = if !beam_texture_filled {
            *beam_texture.filled.lock().unwrap() = true;
            (
                &gpu_images
                    .get(&beam_texture_defualt)
                    .unwrap()
                    .texture_view,
                &gpu_images.get(&beam_texture.image).unwrap().texture_view,
            )
        } else {
            *beam_texture.filled.lock().unwrap() = false;
            (
                &gpu_images.get(&beam_texture.image).unwrap().texture_view,
                target.main_texture(),
            )
        };

        let bind_group = render_context
            .render_device()
            .create_bind_group(&BindGroupDescriptor {
                label: Some("main pass bind group"),
                layout: &pipeline_data.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.binding().unwrap(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(&beam_texture),
                    },
                ],
            });

        let render_pass_descriptor = RenderPassDescriptor {
            label: Some("main pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &target,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        };

        let mut render_pass = render_context
            .command_encoder()
            .begin_render_pass(&render_pass_descriptor);

        render_pass.set_bind_group(0, &voxel_data.bind_group, &[]);
        render_pass.set_bind_group(1, &bind_group, &[]);

        render_pass.set_pipeline(trace_pipeline);
        render_pass.draw(0..3, 0..1);

        Ok(())
    }
}
