use crate::{
    character::CharacterEntity,
    render_pipeline::{MainPassSettings, StreamingSettings},
};
use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_egui::{
    egui::{self, DragValue},
    EguiContexts, EguiPlugin,
};
use bevy_inspector_egui::{reflect_inspector::ui_for_value, DefaultInspectorConfigPlugin};
use std::collections::VecDeque;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin)
            .add_plugins(DefaultInspectorConfigPlugin)
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .insert_resource(FpsData(VecDeque::new()))
            .add_systems(Update, ui_system);
    }
}

#[derive(Resource, Deref, DerefMut)]
struct FpsData(VecDeque<f64>);

fn ui_system(
    mut contexts: EguiContexts,
    mut camera_settings_query: Query<(&mut MainPassSettings, Option<&mut Projection>)>,
    mut post_camera_settings_query: Query<
        (
            Option<&mut BloomSettings>,
            Option<&mut Tonemapping>,
            Option<&mut Fxaa>,
        ),
        With<Camera2d>,
    >,
    window: Query<Entity, With<PrimaryWindow>>,
    diagnostics: Res<DiagnosticsStore>,
    mut character: Query<(&mut Transform, &mut CharacterEntity)>,
    mut fps_data: ResMut<FpsData>,
    streaming_settings: ResMut<StreamingSettings>,
    type_registry: ResMut<AppTypeRegistry>,
) {
    let (mut character, mut character_entity) = character.single_mut();

    egui::Window::new("Settings").show(contexts.ctx_for_window_mut(window.single()), |ui| {
        // add a text field to change the speed of the character
        ui.add(DragValue::new(&mut character_entity.speed));

        if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
            if let Some(measurement) = fps.measurement() {
                fps_data.push_back(measurement.value);
                if fps_data.len() > 100 {
                    fps_data.pop_front();
                }

                let average = fps_data.iter().sum::<f64>() / fps_data.len() as f64;
                let five_percent = fps_data
                    .iter()
                    .take(20)
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap();
                let one_percent = fps_data
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .unwrap();

                ui.label(format!(
                    "average {:.0}, 5% {:.0}, 1% {:.0}",
                    average, five_percent, one_percent
                ));
            }
        }

        let (mut trace_settings, projection) = camera_settings_query.single_mut();
        let (bloom_settings, tonemapping, fxaa) = post_camera_settings_query.single_mut();
        egui::CollapsingHeader::new(format!("Camera Settings"))
            .default_open(true)
            .show(ui, |mut ui| {
                ui_for_value(trace_settings.as_mut(), &mut ui, &type_registry.read());
                if let Some(tonemapping) = tonemapping {
                    ui.push_id(1, |mut ui| {
                        ui_for_value(tonemapping.into_inner(), &mut ui, &type_registry.read());
                    });
                }
                if let Some(bloom_settings) = bloom_settings {
                    ui.push_id(2, |mut ui| {
                        ui_for_value(bloom_settings.into_inner(), &mut ui, &type_registry.read());
                    });
                }
                if let Some(fxaa) = fxaa {
                    ui.push_id(3, |mut ui| {
                        ui_for_value(fxaa.into_inner(), &mut ui, &type_registry.read());
                    });
                }
                if let Some(projection) = projection {
                    ui.push_id(4, |mut ui| {
                        ui_for_value(projection.into_inner(), &mut ui, &type_registry.read());
                    });
                }
            });

        ui.push_id(5, |mut ui| {
            ui_for_value(
                streaming_settings.into_inner(),
                &mut ui,
                &type_registry.read(),
            );
        });

        if ui.button("print pos rot").clicked() {
            println!("{:?}, {:?}", character.translation, character.rotation);
        }
        if ui.button("go to pos1").clicked() {
            character.translation = Vec3::new(-12.808739, 5.79611, 10.124223);
            character.rotation =
                Quat::from_array([-0.28589484, -0.37392232, -0.12235297, 0.8737712]);
        }
        if ui.button("go to pos2").clicked() {
            character.translation = Vec3::new(-2.7467077, 23.573212, -1.1159008);
            character.rotation = Quat::from_array([-0.498245, -0.5017268, -0.49896964, 0.5010505]);
        }
    });
}
