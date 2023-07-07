use super::voxel_world::VoxelData;
use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        view::{ExtractedView, ViewTarget},
        RenderApp, RenderSet,
    },
};
use bevy_inspector_egui::prelude::*;
pub use node::MainPassNode;

mod node;

pub struct MainPassPlugin;

impl Plugin for MainPassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(ExtractComponentPlugin::<MainPassSettings>::default())
            .add_system(update_textures.in_base_set(CoreSet::PostUpdate))
            .register_type::<MainPassSettings>();

        // setup custom render pipeline
        app.sub_app_mut(RenderApp)
            .init_resource::<MainPassPipelineData>()
            .add_system(prepare_uniforms.in_set(RenderSet::Prepare));
    }
}

#[derive(Component, Deref, DerefMut)]
struct BeamTexture(Handle<Image>);

#[derive(Resource)]
struct MainPassPipelineData {
    pipeline_id: CachedRenderPipelineId,
    bind_group_layout: BindGroupLayout,
}

#[derive(Component, Clone, ExtractComponent, Reflect, InspectorOptions)]
#[reflect(InspectorOptions)]
pub struct MainPassSettings {
    pub show_ray_steps: bool,
    pub indirect_lighting: bool,
    pub shadows: bool,
    pub beam_optimization: bool,
    #[inspector(min = 1)]
    pub super_pixel_size: u32,
    pub misc_bool: bool,
    #[inspector(speed = 0.01)]
    pub misc_float: f32,
}

impl Default for MainPassSettings {
    fn default() -> Self {
        Self {
            show_ray_steps: false,
            indirect_lighting: true,
            shadows: true,
            beam_optimization: true,
            super_pixel_size: 8,
            misc_bool: false,
            misc_float: 1.0,
        }
    }
}

#[derive(Clone, ShaderType)]
pub struct TraceUniforms {
    pub camera: Mat4,
    pub camera_inverse: Mat4,
    pub time: f32,
    pub show_ray_steps: u32,
    pub indirect_lighting: u32,
    pub shadows: u32,
    pub misc_bool: u32,
    pub misc_float: f32,
}

#[derive(Component, Deref, DerefMut)]
struct ViewMainPassUniformBuffer(UniformBuffer<TraceUniforms>);

fn prepare_uniforms(
    mut commands: Commands,
    query: Query<(Entity, &MainPassSettings, &ExtractedView)>,
    time: Res<Time>,
    render_device: Res<RenderDevice>,
    render_queue: Res<RenderQueue>,
) {
    let elapsed = time.elapsed_seconds_f64();

    for (entity, settings, view) in query.iter() {
        let projection = view.projection;
        let inverse_projection = projection.inverse();
        let view = view.transform.compute_matrix();
        let inverse_view = view.inverse();

        let camera = projection * inverse_view;
        let camera_inverse = view * inverse_projection;

        let uniforms = TraceUniforms {
            camera,
            camera_inverse,
            time: elapsed as f32,
            show_ray_steps: settings.show_ray_steps as u32,
            indirect_lighting: settings.indirect_lighting as u32,
            shadows: settings.shadows as u32,
            misc_bool: settings.misc_bool as u32,
            misc_float: settings.misc_float,
        };

        let mut uniform_buffer = UniformBuffer::from(uniforms);
        uniform_buffer.write_buffer(&render_device, &render_queue);

        commands
            .entity(entity)
            .insert(ViewMainPassUniformBuffer(uniform_buffer));
    }
}

impl FromWorld for MainPassPipelineData {
    fn from_world(render_world: &mut World) -> Self {
        let voxel_data = render_world.get_resource::<VoxelData>().unwrap();
        let asset_server = render_world.get_resource::<AssetServer>().unwrap();

        let voxel_bind_group_layout = voxel_data.bind_group_layout.clone();
        let bind_group_layout = render_world
            .resource::<RenderDevice>()
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("trace bind group layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: BufferSize::new(TraceUniforms::SHADER_SIZE.into()),
                    },
                    count: None,
                }],
            });

        let trace_shader = asset_server.load("shader.wgsl");

        let trace_pipeline_descriptor = RenderPipelineDescriptor {
            label: Some("trace pipeline".into()),
            layout: vec![voxel_bind_group_layout.clone(), bind_group_layout.clone()],
            vertex: fullscreen_shader_vertex_state(),
            fragment: Some(FragmentState {
                shader: trace_shader,
                shader_defs: Vec::new(),
                entry_point: "fragment".into(),
                targets: vec![Some(ColorTargetState {
                    format: ViewTarget::TEXTURE_FORMAT_HDR,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            push_constant_ranges: Vec::new(),
        };

        let cache = render_world.resource::<PipelineCache>();
        let pipeline_id = cache.queue_render_pipeline(trace_pipeline_descriptor);

        MainPassPipelineData {
            pipeline_id,
            bind_group_layout,
        }
    }
}

fn update_textures(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut query: Query<Entity, (With<MainPassSettings>, Without<BeamTexture>)>,
    mut textures: Query<(&mut BeamTexture, &Camera, &MainPassSettings)>,
) {
    for entity in query.iter_mut() {
        info!("Adding beam texture to {:?}", entity);

        let mut beam_texture = Image::new_fill(
            Extent3d {
                width: 100,
                height: 100,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 230, 0, 255],
            TextureFormat::Rgba16Float,
        );
        beam_texture.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
        let beam_texture = images.add(beam_texture);
        let beam_texture = BeamTexture(beam_texture);

        commands.entity(entity).insert(beam_texture);
    }

    for (mut texture, camera, main_pass_settings) in textures.iter_mut() {
        let size = camera.physical_viewport_size().unwrap() / main_pass_settings.super_pixel_size;
        let texture = images.get_mut(&mut texture.0).unwrap();

        if size != texture.size().as_uvec2() {
            info!("Resizing beam texture to ({}, {})", size.x, size.y);

            let size = Extent3d {
                width: size.x,
                height: size.y,
                depth_or_array_layers: 1,
            };
            texture.resize(size);
        }
    }
}
