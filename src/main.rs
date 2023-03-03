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
        }))
        .add_plugin(render_pipeline::RenderPlugin)
        .add_plugin(character::CharacterPlugin)
        .add_plugin(ui::UiPlugin)
        .add_startup_system(setup)
        .run();
}

fn setup(mut commands: Commands) {
    let character_transform = Transform::from_xyz(2.0, 2.0, -1.0).looking_at(Vec3::ZERO, Vec3::Y);
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
                ..default()
            }),
            ..default()
        },
        MainPassSettings::default(),
        CharacterEntity {
            velocity: Vec3::ZERO,
            grounded: false,
            in_spectator: true,
            look_at: -character_transform.local_z(),
            up: Vec3::Y,
        },
    ));

    use fastanvil::{CurrentJavaChunk, Region};
    use fastnbt::from_bytes;

    let file = std::fs::File::open(
        "/Users/brian/Desktop/Server stuff/Minecraft/1.18.2 Testing Server/world/region/r.0.0.mca",
    )
    .unwrap();

    let mut region = Region::from_stream(file).unwrap();
    let data = region.read_chunk(0, 0).unwrap().unwrap();

    let chunk: CurrentJavaChunk = from_bytes(data.as_slice()).unwrap();
    let section_tower = chunk.sections.unwrap();
    let section = section_tower.get_section_for_y(60).unwrap();
    let block_states = &section.block_states;

    println!();
    for x in 0..16 {
        for y in 0..16 {
            match block_states.at(x, y, 0) {
                Some(block) => print!(
                    "{}",
                    match block.name() {
                        "minecraft:stone" => "#",
                        "minecraft:air" => " ",
                        _ => "?",
                    }
                ),
                None => print!("."),
            }
        }
        println!();
    }
}
