#![feature(const_trait_impl, const_for)]

use bevy::{prelude::*, render::camera::CameraRenderGraph};
use character::CharacterEntity;
use render_pipeline::MainPassSettings;

mod character;
mod render_pipeline;
mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(AssetPlugin {
            watch_for_changes: true,
            ..default()
        }).set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (1920.0 / 2.0, 1080.0 / 2.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugin(render_pipeline::RenderPlugin)
        .add_plugin(character::CharacterPlugin)
        .add_plugin(ui::UiPlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands) {
    let character_transform =
        Transform::from_xyz(21.035963, 19.771912, -31.12883).looking_at(Vec3::ZERO, Vec3::Y);
    commands.spawn((
        Camera3dBundle {
            transform: character_transform,
            camera_render_graph: CameraRenderGraph::new("voxel"),
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
            ..default()
        },
        MainPassSettings { ..default() },
        CharacterEntity {
            look_at: -character_transform.local_z(),
            ..default()
        },
    ));
}
