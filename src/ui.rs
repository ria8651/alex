use crate::{character::CharacterEntity, render_pipeline::StreamingSettings};
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

        ui.push_id(5, |ui| {
            ui_for_value(streaming_settings.into_inner(), ui, &type_registry.read());
        });
    });
}
