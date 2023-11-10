use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    prelude::*,
    render::{camera::RenderTarget, render_resource::*},
    window::{PrimaryWindow, WindowResized, WindowScaleFactorChanged},
};
use bevy_atmosphere::prelude::*;
use character::CharacterEntity;
use render_pipeline::{VoxelStreamingCamera, VoxelVolume, VoxelVolumeBundle};

mod character;
mod render_pipeline;
mod ui;

fn main() {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: (1920.0, 1080.0).into(),
                    ..default()
                }),
                ..default()
            }),
            AtmospherePlugin,
            render_pipeline::VoxelPlugin,
            character::CharacterPlugin,
            ui::UiPlugin,
        ))
        .insert_resource(Msaa::Off)
        .add_systems(Startup, setup)
        .add_systems(Update, update_render_texture)
        .run();
}

#[allow(dead_code)]
#[derive(Resource)]
struct CameraData {
    render_texture: Handle<Image>,
    sprite: Entity,
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // we use a render texture to downscale the main pass
    let mut render_texture = Image::new_fill(
        Extent3d {
            width: 100,
            height: 100,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &[0, 230, 0, 255],
        TextureFormat::Rgba16Float,
    );
    render_texture.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let render_texture = images.add(render_texture);

    // add voxel volume
    commands.spawn(VoxelVolumeBundle {
        voxel_volume: VoxelVolume {
            path: "assets/worlds/imperial_city".into(),
        },
        ..default()
    });

    // add camera with character controller
    let character_transform =
        Transform::from_xyz(21.035963, 19.771912, -31.12883).looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: character_transform,
            camera: Camera {
                hdr: true,
                target: RenderTarget::Image(render_texture.clone()),
                order: -10,
                ..default()
            },
            projection: Projection::Perspective(PerspectiveProjection {
                fov: 1.57,
                near: 0.001,
                far: 100.0,
                ..default()
            }),
            // tonemapping: Tonemapping::None,
            ..default()
        },
        VoxelStreamingCamera,
        CharacterEntity {
            look_at: -character_transform.local_z(),
            ..default()
        },
        BloomSettings::default(),
        Fxaa::default(),
        // AtmosphereCamera::default(),
    ));

    // add sprite and camera to render the render texture
    let sprite = commands
        .spawn(SpriteBundle {
            texture: render_texture.clone(),
            ..default()
        })
        .id();
    commands.spawn((Camera2dBundle {
        camera: Camera {
            hdr: true,
            ..default()
        },
        tonemapping: Tonemapping::None,
        ..default()
    },));
    commands.insert_resource(CameraData {
        render_texture,
        sprite,
    });
}

fn update_render_texture(
    mut resize_reader: EventReader<WindowResized>,
    mut scale_factor_reader: EventReader<WindowScaleFactorChanged>,
    mut images: ResMut<Assets<Image>>,
    mut _sprites: Query<&mut Sprite>,
    render_image: Res<CameraData>,
    windows: Query<&Window, With<PrimaryWindow>>,
) {
    let window = windows.single();

    let mut update = |width: f32, height: f32| {
        let new_size = Extent3d {
            width: width as u32,
            height: height as u32,
            depth_or_array_layers: 1,
        };

        let image = images.get_mut(&render_image.render_texture).unwrap();
        image.resize(new_size);
    };

    for _ in resize_reader.read() {
        update(window.width(), window.height());
    }

    for _ in scale_factor_reader.read() {
        update(window.width(), window.height());
    }
}
