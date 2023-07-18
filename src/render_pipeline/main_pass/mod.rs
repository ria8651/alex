use std::sync::{Arc, Mutex};

use super::voxel_world::VoxelData;
use bevy::{
    core_pipeline::fullscreen_vertex_shader::fullscreen_shader_vertex_state,
    prelude::*,
    render::{
        extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        view::{ExtractedView, ViewTarget},
        Render, RenderApp, RenderSet,
    },
};
pub use node::MainPassNode;

mod node;

pub struct MainPassPlugin;

impl Plugin for MainPassPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<MainPassSettings>::default())
            .add_plugins(ExtractComponentPlugin::<BeamTexture>::default())
            .add_plugins(ExtractResourcePlugin::<DefualtBeamTexture>::default())
            .init_resource::<DefualtBeamTexture>()
            .add_systems(PostUpdate, update_textures);

        // setup custom render pipeline
        app.sub_app_mut(RenderApp)
            .init_resource::<MainPassPipelineData>()
            .add_systems(Render, prepare_uniforms.in_set(RenderSet::Prepare));
    }
}

#[derive(Component, ExtractComponent, Clone)]
pub struct BeamTexture {
    image: Handle<Image>,
    filled: Arc<Mutex<bool>>,
}

#[derive(Resource, ExtractResource, Clone, Deref, DerefMut)]
struct DefualtBeamTexture(Handle<Image>);

#[derive(Resource)]
struct MainPassPipelineData {
    pipeline_id: CachedRenderPipelineId,
    bind_group_layout: BindGroupLayout,
}

#[derive(Component, Clone, ExtractComponent, Reflect)]
pub struct MainPassSettings {
    pub show_ray_steps: bool,
    pub indirect_lighting: bool,
    pub shadows: bool,
    pub beam_optimization: bool,
    pub super_pixel_size: u32,
    pub misc_bool: bool,
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
pub struct MainPassUniforms {
    camera: Mat4,
    camera_inverse: Mat4,
    time: f32,
    show_ray_steps: u32,
    indirect_lighting: u32,
    shadows: u32,
    super_pixel_size: u32,
    misc_bool: u32,
    misc_float: f32,
}

#[derive(Component, Deref, DerefMut)]
pub struct ViewMainPassUniformBuffer(UniformBuffer<MainPassUniforms>);

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

        let uniforms = MainPassUniforms {
            camera,
            camera_inverse,
            time: elapsed as f32,
            show_ray_steps: settings.show_ray_steps as u32,
            indirect_lighting: settings.indirect_lighting as u32,
            shadows: settings.shadows as u32,
            super_pixel_size: settings.super_pixel_size,
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
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Buffer {
                            ty: BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: BufferSize::new(MainPassUniforms::SHADER_SIZE.into()),
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::StorageTexture {
                            access: StorageTextureAccess::ReadOnly,
                            format: TextureFormat::Rgba16Float,
                            view_dimension: TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
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
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::STORAGE_BINDING;
        let beam_texture = images.add(beam_texture);
        let beam_texture = BeamTexture {
            image: beam_texture,
            filled: Arc::new(Mutex::new(false)),
        };

        commands.entity(entity).insert(beam_texture);
    }

    for (mut texture, camera, main_pass_settings) in textures.iter_mut() {
        let size = camera.physical_viewport_size().unwrap() / main_pass_settings.super_pixel_size;
        let texture = images.get_mut(&mut texture.image).unwrap();

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

impl FromWorld for DefualtBeamTexture {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.get_resource_mut::<Assets<Image>>().unwrap();

        let mut defualt_texture = Image::new_fill(
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[255, 255, 255, 255],
            TextureFormat::Rgba16Float,
        );
        defualt_texture.texture_descriptor.usage =
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::STORAGE_BINDING;
        let defualt_texture = images.add(defualt_texture);

        DefualtBeamTexture(defualt_texture)
    }
}
