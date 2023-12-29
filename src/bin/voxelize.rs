use bevy::{
    core_pipeline::bloom::BloomSettings,
    pbr::ScreenSpaceAmbientOcclusionBundle,
    prelude::*,
    render::mesh::{Indices, PrimitiveTopology},
    utils::HashMap,
};
use character::CharacterEntity;
use minecraft_assets::{
    api::{AssetPack, EnumerateResources, FileSystemResourceProvider, ModelResolver, ResourceKind},
    schemas::{
        models::{self, BlockFace},
        Model,
    },
};
use std::{mem::swap, path::Path};

#[path = "../character.rs"]
mod character;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (1920.0, 1080.0).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            character::CharacterPlugin,
        ))
        .add_state::<AppState>()
        .insert_resource(Msaa::Off)
        .add_systems(OnEnter(AppState::Loading), setup)
        .add_systems(Update, check_textures.run_if(in_state(AppState::Loading)))
        .add_systems(OnEnter(AppState::Finished), spawn_blocks)
        .run();
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Finished,
}

#[derive(Resource)]
struct Models(Vec<Model>);

#[derive(Resource)]
struct TextureHandles(HashMap<String, Handle<Image>>);

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // circular base
    commands.spawn(PbrBundle {
        mesh: meshes.add(shape::Circle::new(4.0).into()),
        material: materials.add(Color::WHITE.into()),
        transform: Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, -2.0, 0.0)),
        ..default()
    });
    let mut mesh = BlockModel::new();
    for face in [
        BlockFace::Down,
        BlockFace::Up,
        BlockFace::North,
        BlockFace::South,
        BlockFace::East,
        BlockFace::West,
    ] {
        mesh.push_face(
            Vec3::ZERO,
            Vec3::ONE,
            face,
            Vec2::ZERO,
            Vec2::ONE,
            Quat::IDENTITY,
            Vec3::ZERO,
        );
    }
    let mesh = meshes.add(mesh.to_mesh());
    // cube
    commands.spawn(PbrBundle {
        mesh,
        material: materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load("test.png")),
            perceptual_roughness: 1.0,
            ..default()
        }),
        transform: Transform::from_xyz(0.0, -1.5, 0.0),
        ..default()
    });
    // light
    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            illuminance: 16000.0,
            shadows_enabled: true,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    // character
    let character_transform = Transform::from_xyz(3.0, 2.0, 1.0).looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            camera: Camera {
                hdr: true,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 1.57,
                near: 0.001,
                far: 100.0,
                ..default()
            }),
            transform: character_transform,
            ..default()
        },
        CharacterEntity {
            look_at: -character_transform.local_z(),
            ..default()
        },
        ScreenSpaceAmbientOcclusionBundle::default(),
        BloomSettings::default(),
    ));

    // models
    let root = Path::new("/Users/brian/Downloads/minecraft-assets");

    let assets = AssetPack::at_path(root);
    let resource_provider = FileSystemResourceProvider::new(root);
    let block_paths = resource_provider
        .enumerate_resources("minecraft", ResourceKind::BlockModel)
        .unwrap();

    let mut models: Vec<Model> = Vec::new();
    for block in block_paths.iter() {
        let new_models = assets.load_block_model_recursive(block.path()).unwrap();
        let new_model = ModelResolver::resolve_model(&new_models);

        if new_model.elements.is_none() {
            continue;
        }

        models.push(new_model);
    }

    commands.insert_resource(AmbientLight {
        brightness: 0.3,
        ..default()
    });

    let mut texture_handles = HashMap::new();
    for model in models.iter() {
        for texture in model.textures.as_ref().expect("no textures").values() {
            if texture.0.starts_with("#") {
                continue;
            }

            if !texture_handles.contains_key(&texture.0) {
                let texture_path = &texture.0.trim_start_matches("minecraft:");
                let texture_path =
                    root.join(format!("assets/minecraft/textures/{texture_path}.png"));

                let texture_handle: Handle<Image> = asset_server.load(texture_path);
                texture_handles.insert(texture.0.clone(), texture_handle);
            }
        }
    }

    // we now have to wait for all the textures to be loaded
    commands.insert_resource(Models(models));
    commands.insert_resource(TextureHandles(texture_handles));
}

fn check_textures(
    mut events: EventReader<AssetEvent<Image>>,
    texture_handles: Res<TextureHandles>,
    mut loaded: Local<Vec<String>>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for event in events.read() {
        for (name, texture_handle) in texture_handles.0.iter() {
            if event.is_loaded_with_dependencies(texture_handle) {
                loaded.push(name.clone());
                // info!("loaded {}", name);
            }
        }
    }

    if loaded.len() == texture_handles.0.len() {
        next_state.set(AppState::Finished);
        // info!("finished loading!");
    }
}

fn spawn_blocks(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    texture_handles: Res<TextureHandles>,
    models: Res<Models>,
) {
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    let mut image_sizes = Vec::new();
    for handle in texture_handles.0.values() {
        let texture = images.get(handle).expect("image not found");
        texture_atlas_builder.add_texture(handle.id(), texture);
        image_sizes.push(texture.size_f32());
    }

    let texture_atlas = texture_atlas_builder.finish(&mut images).unwrap();
    let block_material = materials.add(StandardMaterial {
        base_color_texture: Some(texture_atlas.texture.clone()),
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 1.0,
        ..default()
    });

    let side_length = (models.0.len() as f32).sqrt().ceil() as usize;
    for (i, model) in models.0.iter().enumerate() {
        let mut block_model = BlockModel::new();
        for element in model.elements.as_ref().expect("no elements") {
            let axis = match element.rotation.axis {
                models::Axis::X => Vec3::new(1.0, 0.0, 0.0),
                models::Axis::Y => Vec3::new(0.0, 1.0, 0.0),
                models::Axis::Z => Vec3::new(0.0, 0.0, 1.0),
            };
            let rotation = Quat::from_axis_angle(axis, element.rotation.angle);
            let rotate_around = Vec3::from(element.rotation.origin);

            let bottom_left = Vec3::from(element.from);
            let top_right = Vec3::from(element.to);

            for (block_face, face) in element.faces.iter() {
                let Some(handle) = texture_handles.0.get(&face.texture.0) else {
                    info!("missing texture");
                    continue;
                };
                let Some(index) = texture_atlas.get_texture_index(handle) else {
                    info!("missing texture");
                    continue;
                };

                let uv_size = image_sizes[index];
                let uv_rect = texture_atlas.textures[index];

                let (mut uv_bottom_left, mut uv_top_right) = match face.uv {
                    Some(uv) => (Vec2::new(uv[0], uv[1]), Vec2::new(uv[2], uv[3])),
                    None => match block_face {
                        BlockFace::Up | BlockFace::Down => (bottom_left.xz(), top_right.xz()),
                        BlockFace::North | BlockFace::South => (bottom_left.xy(), top_right.xy()),
                        BlockFace::East | BlockFace::West => (bottom_left.yz(), top_right.yz()),
                    },
                };

                match face.rotation {
                    0 => {}
                    90 => {
                        swap(&mut uv_bottom_left.x, &mut uv_top_right.y);
                        swap(&mut uv_bottom_left.y, &mut uv_top_right.x);
                    }
                    180 => {
                        swap(&mut uv_bottom_left.y, &mut uv_top_right.y);
                        swap(&mut uv_bottom_left.x, &mut uv_top_right.x);
                    }
                    270 => {
                        swap(&mut uv_bottom_left.x, &mut uv_top_right.y);
                        swap(&mut uv_bottom_left.y, &mut uv_top_right.x);
                    }
                    _ => unreachable!("invalid rotation"),
                }

                uv_bottom_left =
                    (uv_rect.size() * uv_bottom_left / uv_size + uv_rect.min) / texture_atlas.size;
                uv_top_right =
                    (uv_rect.size() * uv_top_right / uv_size + uv_rect.min) / texture_atlas.size;

                block_model.push_face(
                    bottom_left,
                    top_right,
                    *block_face,
                    uv_bottom_left,
                    uv_top_right,
                    rotation,
                    rotate_around,
                );
            }
        }

        let pos = Vec3::new((i % side_length) as f32, 0.0, (i / side_length) as f32);
        commands.spawn(PbrBundle {
            mesh: meshes.add(block_model.to_mesh()),
            material: block_material.clone(),
            transform: Transform::from_translation(pos).with_scale(Vec3::splat(1.0 / 16.0)),
            ..default()
        });
    }
}

struct BlockModel {
    positions: Vec<Vec3>,
    normals: Vec<Vec3>,
    uvs: Vec<Vec2>,
    indices: Vec<u32>,
}

impl BlockModel {
    fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            uvs: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn to_mesh(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList)
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
            .with_indices(Some(Indices::U32(self.indices)))
    }

    fn push_face(
        &mut self,
        c1: Vec3,
        c2: Vec3,
        face: BlockFace,
        uv1: Vec2,
        uv2: Vec2,
        rot: Quat,
        rot_pos: Vec3,
    ) {
        let b = self.positions.len() as u32;
        self.indices
            .extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 3, b]);
        match face {
            BlockFace::North => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                    Vec3::new(0.0, 0.0, 1.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                ]);
            }
            BlockFace::South => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                    Vec3::new(0.0, 0.0, -1.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                ]);
            }
            BlockFace::East => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(1.0, 0.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                ]);
            }
            BlockFace::West => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv1.x, uv2.y),
                ]);
            }
            BlockFace::Up => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c2.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c2.y, c2.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                    Vec3::new(0.0, 1.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv1.x, uv1.y),
                    Vec2::new(uv2.x, uv1.y),
                ]);
            }
            BlockFace::Down => {
                self.positions.extend_from_slice(&[
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c2.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c1.x, c1.y, c1.z) - rot_pos),
                    rot_pos + rot * (Vec3::new(c2.x, c1.y, c1.z) - rot_pos),
                ]);
                self.normals.extend_from_slice(&[
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                    Vec3::new(0.0, -1.0, 0.0),
                ]);
                self.uvs.extend_from_slice(&[
                    Vec2::new(uv1.x, uv2.y),
                    Vec2::new(uv2.x, uv2.y),
                    Vec2::new(uv2.x, uv1.y),
                    Vec2::new(uv1.x, uv1.y),
                ]);
            }
        }
    }
}
