use bevy::{
    core_pipeline::bloom::BloomSettings,
    pbr::ScreenSpaceAmbientOcclusionBundle,
    prelude::*,
    render::extract_resource::{ExtractResource, ExtractResourcePlugin},
    utils::HashMap,
    window::PresentMode,
};
use block_model::BlockModel;
use character::CharacterEntity;
use crossbeam::channel::{Receiver, Sender};
use dot_vox::{DotVoxData, Model, Size, Voxel};
use std::fs::File;

mod block_model;
#[path = "../../character.rs"]
mod character;
mod load_models;
mod voxelization;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: (1920.0, 1080.0).into(),
                        present_mode: PresentMode::AutoNoVsync,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_nearest()),
            character::CharacterPlugin,
            voxelization::VoxelizationPlugin,
            load_models::LoadModelsPlugin,
            ExtractResourcePlugin::<VoxelReturnChannel>::default(),
        ))
        .add_state::<AppState>()
        .insert_resource(Msaa::Off)
        .add_systems(OnEnter(AppState::Loading), setup)
        .add_systems(Update, receive_voxels.run_if(in_state(AppState::Finished)))
        .run();
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
enum AppState {
    #[default]
    Loading,
    Finished,
}

#[derive(Resource, ExtractResource, Clone)]
pub struct VoxelReturnChannel {
    sender: Sender<(String, Vec<(UVec3, [u8; 4])>)>,
    receiver: Receiver<(String, Vec<(UVec3, [u8; 4])>)>,
}

fn setup(mut commands: Commands) {
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

    // return channel
    let (sender, receiver) = crossbeam::channel::unbounded();
    commands.insert_resource(VoxelReturnChannel { sender, receiver });
}

#[derive(Component)]
pub struct LilBlock;

pub fn receive_voxels(
    // mut commands: Commands,
    // mut meshes: ResMut<Assets<Mesh>>,
    // mut materials: ResMut<Assets<StandardMaterial>>,
    // lil_blocks: Query<Entity, With<LilBlock>>,
    voxel_return_channel: Res<VoxelReturnChannel>,
) {
    for (identifier, blocks) in voxel_return_channel.receiver.try_iter() {
        // create the palette
        let mut palette_map = HashMap::new();
        for (_, color) in blocks.iter() {
            if !palette_map.contains_key(color) {
                palette_map.insert(*color, palette_map.len() as u8);
            }
        }

        if palette_map.len() > 255 {
            panic!("Too many colors in palette for {}", identifier);
        }

        let mut pallete = vec![[0u8; 4]; 256];
        for (color, index) in palette_map.iter() {
            pallete[*index as usize] = *color;
        }

        let path = format!("assets/block_models/{}.vox", identifier);
        std::fs::create_dir_all(std::path::Path::new(&path).parent().unwrap()).unwrap();
        DotVoxData {
            version: 150,
            models: vec![Model {
                size: Size {
                    x: 16,
                    y: 16,
                    z: 16,
                },
                voxels: blocks
                    .iter()
                    .map(|(position, color)| Voxel {
                        x: position.x as u8,
                        y: position.z as u8,
                        z: position.y as u8,
                        i: palette_map[color],
                    })
                    .collect(),
            }],
            palette: pallete
                .iter()
                .map(|color| dot_vox::Color {
                    r: color[0],
                    g: color[1],
                    b: color[2],
                    a: color[3],
                })
                .collect(),
            materials: vec![],
            scenes: vec![],
            layers: vec![],
        }
        .write_vox(&mut File::create(path.clone()).unwrap())
        .unwrap();

        info!("Wrote vox file {}", path);
    }

    // let received_blocks = voxel_return_channel.receiver.try_iter().collect::<Vec<_>>();
    // if received_blocks.len() > 1 {
    //     for entity in lil_blocks.iter() {
    //         commands.entity(entity).despawn();
    //     }

    //     let (_, blocks) = received_blocks.last().unwrap();
    //     for (position, color) in blocks {
    //         commands.spawn((
    //             MaterialMeshBundle {
    //                 mesh: meshes.add(Mesh::from(shape::Box::from_corners(
    //                     Vec3::ZERO,
    //                     Vec3::ONE / 16.0,
    //                 ))),
    //                 material: materials
    //                     .add(Color::rgba_u8(color[0], color[1], color[2], color[3]).into()),
    //                 transform: Transform::from_translation(
    //                     position.as_vec3() / 16.0 + Vec3::X * 3.0,
    //                 ),
    //                 ..default()
    //             },
    //             LilBlock,
    //         ));
    //     }
    // }
}
