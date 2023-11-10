use super::{
    voxel_world::{GpuVoxelWorld, SetVoxelDataBindGroup, VoxelData},
    VoxelVolume,
};
use bevy::{
    core_pipeline::core_3d::Opaque3d,
    ecs::system::{lifetimeless::*, SystemParamItem},
    pbr::{
        MeshPipeline, MeshPipelineKey, RenderMeshInstances, SetMeshBindGroup, SetMeshViewBindGroup,
    },
    prelude::*,
    render::RenderApp,
    render::{
        mesh::{GpuBufferInfo, MeshVertexBufferLayout},
        render_asset::RenderAssets,
        render_phase::{
            AddRenderCommand, DrawFunctions, PhaseItem, RenderCommand, RenderCommandResult,
            RenderPhase, SetItemPipeline, TrackedRenderPass,
        },
        render_resource::*,
        renderer::RenderDevice,
        view::ExtractedView,
        Render, RenderSet,
    },
};
use bytemuck::{Pod, Zeroable};

pub struct VoxelRenderPlugin;

impl Plugin for VoxelRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CubeHandle>()
            .add_systems(PostUpdate, add_mesh_handles);
        app.sub_app_mut(RenderApp)
            .add_render_command::<Opaque3d, DrawVoxel>()
            .init_resource::<SpecializedMeshPipelines<VoxelPipeline>>()
            .add_systems(
                Render,
                (
                    queue_custom.in_set(RenderSet::QueueMeshes),
                    (prepare_instance_buffers.in_set(RenderSet::PrepareResources),).chain(),
                ),
            );
    }

    fn finish(&self, app: &mut App) {
        app.sub_app_mut(RenderApp).init_resource::<VoxelPipeline>();
    }
}

#[derive(Resource, Deref, DerefMut)]
struct CubeHandle(Handle<Mesh>);

impl FromWorld for CubeHandle {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        CubeHandle(meshes.add(Mesh::from(shape::Box::from_corners(Vec3::ZERO, Vec3::ONE))))
    }
}

fn add_mesh_handles(
    voxel_volumes: Query<Entity, With<VoxelVolume>>,
    mut commands: Commands,
    cube_handle: Res<CubeHandle>,
) {
    for entity in voxel_volumes.iter() {
        commands.entity(entity).insert(cube_handle.clone());
    }
}

#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BrickInstance {
    position: Vec3,
    scale: f32,
    brick: u32,
}

#[derive(Component)]
pub struct InstanceBuffer {
    buffer: Buffer,
    length: usize,
}

fn prepare_instance_buffers(
    mut commands: Commands,
    query: Query<Entity, With<VoxelVolume>>,
    render_device: Res<RenderDevice>,
    gpu_voxel_world: Res<GpuVoxelWorld>,
) {
    let mut brick_istance_data = Vec::new();

    gpu_voxel_world.recursive_search(&mut |index, pos, depth| {
        let position = pos.as_vec3();
        let scale = (1 << gpu_voxel_world.brickmap_depth - depth) as f32;
        brick_istance_data.push(BrickInstance {
            position,
            scale,
            brick: index as u32,
        });
    });

    let length = brick_istance_data.len();
    let brick_instance_data = bytemuck::cast_slice(brick_istance_data.as_slice());
    for entity in query.iter() {
        let buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
            label: Some("instance data buffer"),
            contents: brick_instance_data,
            usage: BufferUsages::VERTEX,
        });
        commands
            .entity(entity)
            .insert(InstanceBuffer { buffer, length });
    }
}

fn queue_custom(
    opaque_3d_draw_functions: Res<DrawFunctions<Opaque3d>>,
    custom_pipeline: Res<VoxelPipeline>,
    msaa: Res<Msaa>,
    mut pipelines: ResMut<SpecializedMeshPipelines<VoxelPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    meshes: Res<RenderAssets<Mesh>>,
    render_mesh_instances: Res<RenderMeshInstances>,
    material_meshes: Query<Entity, With<VoxelVolume>>,
    mut views: Query<(&ExtractedView, &mut RenderPhase<Opaque3d>)>,
) {
    let draw_custom = opaque_3d_draw_functions.read().id::<DrawVoxel>();

    let msaa_key = MeshPipelineKey::from_msaa_samples(msaa.samples());

    for (view, mut transparent_phase) in &mut views {
        let view_key = msaa_key | MeshPipelineKey::from_hdr(view.hdr);
        let rangefinder = view.rangefinder3d();
        for entity in &material_meshes {
            let Some(mesh_instance) = render_mesh_instances.get(&entity) else {
                continue;
            };
            let Some(mesh) = meshes.get(mesh_instance.mesh_asset_id) else {
                continue;
            };
            let key = view_key | MeshPipelineKey::from_primitive_topology(mesh.primitive_topology);
            let pipeline = pipelines
                .specialize(&pipeline_cache, &custom_pipeline, key, &mesh.layout)
                .unwrap();
            transparent_phase.add(Opaque3d {
                entity,
                pipeline,
                draw_function: draw_custom,
                distance: rangefinder
                    .distance_translation(&mesh_instance.transforms.transform.translation),
                batch_range: 0..1,
                dynamic_offset: None,
            });
        }
    }
}

#[derive(Resource)]
pub struct VoxelPipeline {
    shader: Handle<Shader>,
    mesh_pipeline: MeshPipeline,
    voxel_data_bind_group_layout: BindGroupLayout,
}

impl FromWorld for VoxelPipeline {
    fn from_world(world: &mut World) -> Self {
        let asset_server = world.resource::<AssetServer>();
        let mesh_pipeline = world.resource::<MeshPipeline>().clone();
        let voxel_data = world.resource::<VoxelData>();

        let shader = asset_server.load("instancing.wgsl");
        let voxel_data_bind_group_layout = voxel_data.bind_group_layout.clone();

        VoxelPipeline {
            shader,
            mesh_pipeline,
            voxel_data_bind_group_layout,
        }
    }
}

impl SpecializedMeshPipeline for VoxelPipeline {
    type Key = MeshPipelineKey;

    fn specialize(
        &self,
        key: Self::Key,
        layout: &MeshVertexBufferLayout,
    ) -> Result<RenderPipelineDescriptor, SpecializedMeshPipelineError> {
        let mut descriptor = self.mesh_pipeline.specialize(key, layout)?;

        descriptor.layout = vec![
            self.mesh_pipeline.get_view_layout(key.into()).clone(),
            self.mesh_pipeline.mesh_layouts.model_only.clone(),
            self.voxel_data_bind_group_layout.clone(),
        ];

        // meshes typically live in bind group 2. because we are using bindgroup 1
        // we need to add MESH_BINDGROUP_1 shader def so that the bindings are correctly
        // linked in the shader
        descriptor
            .vertex
            .shader_defs
            .push("MESH_BINDGROUP_1".into());
        descriptor
            .fragment
            .as_mut()
            .unwrap()
            .shader_defs
            .push("MESH_BINDGROUP_1".into());

        descriptor.vertex.shader = self.shader.clone();
        descriptor.vertex.buffers.push(VertexBufferLayout {
            array_stride: std::mem::size_of::<BrickInstance>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: vec![
                VertexAttribute {
                    format: VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 3, // shader locations 0-2 are taken up by Position, Normal and UV attributes
                },
                VertexAttribute {
                    format: VertexFormat::Uint32,
                    offset: 16,
                    shader_location: 4,
                },
            ],
        });

        descriptor.fragment.as_mut().unwrap().shader = self.shader.clone();

        descriptor.primitive.cull_mode = None;

        Ok(descriptor)
    }
}

type DrawVoxel = (
    SetItemPipeline,
    SetMeshViewBindGroup<0>,
    SetMeshBindGroup<1>,
    SetVoxelDataBindGroup<2>,
    DrawVoxelPhase,
);

pub struct DrawVoxelPhase;

impl<P: PhaseItem> RenderCommand<P> for DrawVoxelPhase {
    type Param = (SRes<RenderAssets<Mesh>>, SRes<RenderMeshInstances>);
    type ViewWorldQuery = ();
    type ItemWorldQuery = Read<InstanceBuffer>;

    #[inline]
    fn render<'w>(
        item: &P,
        _view: (),
        instance_buffer: &'w InstanceBuffer,
        (meshes, render_mesh_instances): SystemParamItem<'w, '_, Self::Param>,
        pass: &mut TrackedRenderPass<'w>,
    ) -> RenderCommandResult {
        let Some(mesh_instance) = render_mesh_instances.get(&item.entity()) else {
            return RenderCommandResult::Failure;
        };
        let gpu_mesh = match meshes.into_inner().get(mesh_instance.mesh_asset_id) {
            Some(gpu_mesh) => gpu_mesh,
            None => return RenderCommandResult::Failure,
        };

        pass.set_vertex_buffer(0, gpu_mesh.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.buffer.slice(..));

        match &gpu_mesh.buffer_info {
            GpuBufferInfo::Indexed {
                buffer,
                index_format,
                count,
            } => {
                pass.set_index_buffer(buffer.slice(..), 0, *index_format);
                pass.draw_indexed(0..*count, 0, 0..instance_buffer.length as u32);
            }
            GpuBufferInfo::NonIndexed => {
                pass.draw(0..gpu_mesh.vertex_count, 0..instance_buffer.length as u32);
            }
        }
        RenderCommandResult::Success
    }
}
