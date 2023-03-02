use bevy::{prelude::*, render::camera::CameraRenderGraph};
use character::CharacterEntity;
use render_pipeline::MainPassSettings;

mod character;
mod render_pipeline;
mod ui;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(render_pipeline::RenderPlugin)
        .add_plugin(character::CharacterPlugin)
        .add_plugin(ui::UiPlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn((
        Camera3dBundle {
            camera_render_graph: CameraRenderGraph::new("voxel"),
            camera: Camera {
                hdr: true,
                ..default()
            },
            ..default()
        },
        MainPassSettings::default(),
        CharacterEntity {
            velocity: Vec3::ZERO,
            grounded: false,
            in_spectator: true,
            look_at: Vec3::Z,
            up: Vec3::Y,
        },
    ));
}
