use crate::{character::CharacterEntity, render_pipeline::MainPassSettings};
use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    reflect::TypeRegistryInternal,
    window::PrimaryWindow,
};
use bevy_egui::{
    egui::{self, DragValue, Slider},
    EguiContexts, EguiPlugin,
};
use bevy_inspector_egui::{reflect_inspector::ui_for_value, DefaultInspectorConfigPlugin};
use std::collections::VecDeque;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
            .add_plugin(DefaultInspectorConfigPlugin)
            .add_plugin(FrameTimeDiagnosticsPlugin::default())
            .insert_resource(FpsData(VecDeque::new()))
            .add_system(ui_system);
    }
}

#[derive(Resource, Deref, DerefMut)]
struct FpsData(VecDeque<f64>);

fn ui_system(
    mut contexts: EguiContexts,
    mut camera_settings_query: Query<(
        &mut MainPassSettings,
        Option<&mut BloomSettings>,
        Option<&mut Tonemapping>,
        Option<&mut Fxaa>,
        Option<&mut Projection>,
    )>,
    window: Query<Entity, With<PrimaryWindow>>,
    diagnostics: Res<Diagnostics>,
    mut character: Query<(&mut Transform, &mut CharacterEntity)>,
    mut fps_data: ResMut<FpsData>,
) {
    let (mut character, mut character_entity) = character.single_mut();

    egui::Window::new("Settings")
        .anchor(egui::Align2::RIGHT_TOP, [-5.0, 5.0])
        .show(contexts.ctx_for_window_mut(window.single()), |ui| {
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

            for (i, (mut trace_settings, bloom_settings, tonemapping, fxaa, projection)) in
                camera_settings_query.iter_mut().enumerate()
            {
                egui::CollapsingHeader::new(format!("Camera Settings {}", i))
                    .default_open(true)
                    .show(ui, |mut ui| {
                        ui.checkbox(&mut trace_settings.show_ray_steps, "Show ray steps");
                        ui.checkbox(&mut trace_settings.indirect_lighting, "Indirect lighting");
                        ui.checkbox(&mut trace_settings.shadows, "Shadows");
                        ui.checkbox(&mut trace_settings.show_brick_texture, "Show brick texture");
                        ui.add(
                            Slider::new(&mut trace_settings.alpha_cutoff, 0.0..=1.0)
                                .text("Alpha cutoff"),
                        );
                        ui.add(
                            Slider::new(&mut trace_settings.streaming_ratio, 0.0..=3.0)
                                .text("Streaming ratio"),
                        );
                        ui.add(
                            Slider::new(&mut trace_settings.streaming_range, 0.0..=1.0)
                                .text("Streaming range"),
                        );
                        ui.checkbox(&mut trace_settings.misc_bool, "Misc");
                        ui.add(Slider::new(&mut trace_settings.misc_float, 0.0..=1.0).text("Misc"));

                        let registry = &TypeRegistryInternal::default();
                        if let Some(tonemapping) = tonemapping {
                            ui_for_value(tonemapping.into_inner(), &mut ui, registry);
                        }
                        if let Some(bloom_settings) = bloom_settings {
                            ui_for_value(bloom_settings.into_inner(), &mut ui, registry);
                        }
                        if let Some(fxaa) = fxaa {
                            ui_for_value(fxaa.into_inner(), &mut ui, registry);
                        }
                        if let Some(projection) = projection {
                            match projection.into_inner() {
                                Projection::Orthographic(orthographic_projection) => {
                                    ui.add(
                                        Slider::new(
                                            &mut orthographic_projection.scale,
                                            0.0..=1000.0,
                                        )
                                        .text("Orthographic scale"),
                                    );
                                }
                                Projection::Perspective(perspective_projection) => {
                                    ui.add(
                                        Slider::new(&mut perspective_projection.fov, 0.01..=3.0)
                                            .logarithmic(true)
                                            .text("Perspective fov"),
                                    );
                                }
                            }
                        }
                    });
            }

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
                character.rotation =
                    Quat::from_array([-0.498245, -0.5017268, -0.49896964, 0.5010505]);
            }
        });
}
