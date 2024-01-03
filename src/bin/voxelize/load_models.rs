use crate::{receive_voxels, voxelization::VoxelizationMaterial, AppState, BlockModel};
use bevy::{
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::*,
        renderer::RenderDevice,
    },
    utils::HashMap,
};
use minecraft_assets::{
    api::{AssetPack, EnumerateResources, FileSystemResourceProvider, ModelResolver, ResourceKind},
    schemas::{
        models::{self, BlockFace},
        Model,
    },
};
use std::{mem::swap, path::Path};

pub struct LoadModelsPlugin;

impl Plugin for LoadModelsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractResourcePlugin::<LoadedBlock>::default())
            .init_resource::<LoadedBlock>()
            .add_systems(OnEnter(AppState::Loading), setup)
            .add_systems(Update, check_textures.run_if(in_state(AppState::Loading)))
            .add_systems(OnEnter(AppState::Finished), make_atlas)
            .add_systems(
                Update,
                next_block
                    .run_if(in_state(AppState::Finished))
                    .after(receive_voxels),
            );
    }
}

#[derive(Resource, Deref, DerefMut)]
struct Models(Vec<(String, Model)>);

#[derive(Resource)]
struct TextureHandles(HashMap<String, Handle<Image>>);

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = Path::new("/Users/brian/Downloads/minecraft-assets");

    let assets = AssetPack::at_path(root);
    let resource_provider = FileSystemResourceProvider::new(root);
    let block_paths = resource_provider
        .enumerate_resources("minecraft", ResourceKind::BlockModel)
        .unwrap();

    let mut models = Vec::new();
    for block in block_paths.iter() {
        let new_models = assets.load_block_model_recursive(block.path()).unwrap();
        let new_model = ModelResolver::resolve_model(&new_models);

        if new_model.elements.is_none() {
            continue;
        }

        models.push((block.as_str().to_string(), new_model));
    }

    commands.insert_resource(AmbientLight {
        brightness: 0.3,
        ..default()
    });

    let mut texture_handles = HashMap::new();
    for (_, model) in models.iter() {
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

#[derive(Resource, Deref, DerefMut)]
struct Atlas(TextureAtlas);

#[derive(Resource, Deref, DerefMut)]
struct ImageSizes(Vec<Vec2>);

fn make_atlas(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    texture_handles: Res<TextureHandles>,
) {
    let mut texture_atlas_builder = TextureAtlasBuilder::default();
    let mut image_sizes = Vec::new();
    for handle in texture_handles.0.values() {
        let texture = images.get(handle).expect("image not found");
        texture_atlas_builder.add_texture(handle.id(), texture);
        image_sizes.push(texture.size_f32());
    }

    let texture_atlas = texture_atlas_builder.finish(&mut images).unwrap();

    commands.insert_resource(Atlas(texture_atlas));
    commands.insert_resource(ImageSizes(image_sizes));
}

#[derive(Component)]
struct Block(String);

#[derive(Resource, Clone, Default, ExtractResource)]
pub struct LoadedBlock {
    pub index: usize,
    pub identifier: String,
}

fn next_block(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<VoxelizationMaterial>>,
    mut loaded_block: ResMut<LoadedBlock>,
    blocks: Query<Entity, With<Block>>,
    texture_handles: Res<TextureHandles>,
    atlas: Res<Atlas>,
    image_sizes: Res<ImageSizes>,
    models: Res<Models>,
    render_device: Res<RenderDevice>,
) {
    if loaded_block.index >= models.len() {
        return;
    }

    // remove old blocks
    for block in blocks.iter() {
        commands.entity(block).despawn();
    }

    let block_material = materials.add(VoxelizationMaterial {
        color_texture: Some(atlas.texture.clone()),
        // would rather use Vec<u32> here but bevy doesn't add MAP_READ to the buffer usages
        output_buffer: render_device.create_buffer(&BufferDescriptor {
            label: Some("voxels"),
            size: 16 * 16 * 16 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        }),
    });

    let (name, model) = &models[loaded_block.index];
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
                info!("missing texture {}", face.texture.0);
                continue;
            };
            let Some(index) = atlas.get_texture_index(handle) else {
                info!("texture not in atlas {}", face.texture.0);
                continue;
            };

            let uv_size = image_sizes[index];
            let uv_rect = atlas.textures[index];

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

            uv_bottom_left = (uv_rect.size() * uv_bottom_left / uv_size + uv_rect.min) / atlas.size;
            uv_top_right = (uv_rect.size() * uv_top_right / uv_size + uv_rect.min) / atlas.size;

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

    commands.spawn((
        MaterialMeshBundle {
            mesh: meshes.add(block_model.to_mesh()),
            material: block_material.clone(),
            transform: Transform::from_scale(Vec3::splat(1.0 / 16.01)),
            ..default()
        },
        Block(name.clone()),
    ));

    loaded_block.index += 1;
    loaded_block.identifier = name.clone();
}
