use bevy::{
    pbr::RenderMaterials,
    prelude::*,
    render::{
        camera::{RenderTarget, ScalingMode},
        render_resource::*,
        renderer::RenderDevice,
        Render, RenderApp, RenderSet,
    },
};

pub struct VoxelizationPlugin;

impl Plugin for VoxelizationPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<VoxelizationMaterial> {
            prepass_enabled: false,
            ..default()
        })
        .add_systems(Startup, setup);

        app.sub_app_mut(RenderApp)
            .add_systems(Render, read_back_buffer_data.in_set(RenderSet::Cleanup));
    }
}

#[derive(Component)]
struct VoxelizationCamera;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct VoxelizationMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub color_texture: Option<Handle<Image>>,
    #[storage(2, buffer)]
    pub output_voxels: Buffer,
}

impl Material for VoxelizationMaterial {
    fn fragment_shader() -> ShaderRef {
        "voxelization.wgsl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline<Self>,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &bevy::render::mesh::MeshVertexBufferLayout,
        _key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = Some(Face::Back);

        Ok(())
    }
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // image that is the size of the output texture to create the correct ammount of fragments
    let size = Extent3d {
        width: 16,
        height: 16,
        ..default()
    };
    let mut render_image = Image {
        texture_descriptor: TextureDescriptor {
            label: None,
            size,
            dimension: TextureDimension::D2,
            format: TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count: 1,
            usage: TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        },
        ..default()
    };
    render_image.resize(size);
    let render_image_handle = images.add(render_image);

    // debug cube
    commands.spawn(MaterialMeshBundle {
        transform: Transform::from_xyz(-2.0, 0.0, -2.0),
        mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
        material: materials.add(StandardMaterial {
            base_color_texture: Some(render_image_handle.clone()),
            perceptual_roughness: 1.0,
            ..default()
        }),
        ..default()
    });

    // priorities of -3, -2 and -1 so that they are rendered before the main pass
    for i in 0..3 {
        let transform = match i {
            0 => Transform::from_translation(0.5 * (Vec3::Y + Vec3::Z))
                .looking_at(0.5 * (Vec3::Y + Vec3::Z) + Vec3::X, Vec3::Y),
            1 => Transform::from_translation(0.5 * (Vec3::X + Vec3::Z))
                .looking_at(0.5 * (Vec3::X + Vec3::Z) + Vec3::Y, Vec3::Z),
            2 => Transform::from_translation(0.5 * (Vec3::X + Vec3::Y))
                .looking_at(0.5 * (Vec3::X + Vec3::Y) + Vec3::Z, Vec3::Y),
            _ => panic!("Too many voxelization cameras"),
        };

        commands.spawn((
            Camera3dBundle {
                transform,
                camera: Camera {
                    target: RenderTarget::Image(render_image_handle.clone()),
                    order: -3 + i,
                    ..default()
                },
                projection: Projection::Orthographic(OrthographicProjection {
                    near: 0.0,
                    far: 1.0,
                    scaling_mode: ScalingMode::Fixed {
                        width: 1.0,
                        height: 1.0,
                    },
                    ..default()
                }),
                ..default()
            },
            VoxelizationCamera,
        ));
    }
}

fn read_back_buffer_data(
    render_materials: ResMut<RenderMaterials<VoxelizationMaterial>>,
    render_device: Res<RenderDevice>,
) {
    for prepared_material in render_materials.values() {
        for (_, binding) in prepared_material.bindings.iter() {
            if let OwnedBindingResource::Buffer(buffer) = binding {
                let slice = buffer.slice(..);
                slice.map_async(MapMode::Read, |_| {});

                // note: this waits for the gpu to finish execution - this is not ideal and removes pipelined execution
                // it is also not supported by bevy or web wgpu backends
                render_device.poll(wgpu::Maintain::Wait);

                let data = slice.get_mapped_range();
                let result: Vec<u32> = bytemuck::cast_slice(&data).to_vec();

                // unmap the buffer so that it can be used again
                drop(data);
                buffer.unmap();

                println!("{:?}", result);
            }
        }
    }
}
