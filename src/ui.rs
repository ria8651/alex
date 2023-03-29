use crate::{character::CharacterEntity, render_pipeline::MainPassSettings};
use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    diagnostic::{Diagnostics, FrameTimeDiagnosticsPlugin},
    prelude::*,
    window::PrimaryWindow,
};
use bevy_egui::{
    egui::{self, Slider},
    EguiContext, EguiPlugin,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin)
            .add_plugin(FrameTimeDiagnosticsPlugin::default())
            .add_system(ui_system);
    }
}

fn ui_system(
    mut egui_context: ResMut<EguiContext>,
    mut camera_settings_query: Query<(
        &mut MainPassSettings,
        Option<&mut BloomSettings>,
        Option<&mut Tonemapping>,
        Option<&mut Fxaa>,
        Option<&mut Projection>,
    )>,
    window: Query<Entity, With<PrimaryWindow>>,
    diagnostics: Res<Diagnostics>,
    mut character: Query<&mut Transform, With<CharacterEntity>>,
) {
    let mut character = character.single_mut();

    egui::Window::new("Settings")
        .anchor(egui::Align2::RIGHT_TOP, [-5.0, 5.0])
        .show(egui_context.ctx_for_window_mut(window.single()), |ui| {
            if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
                if let Some(average) = fps.average() {
                    ui.label(format!("FPS: {:.0}", average));
                }
            }

            for (i, (mut trace_settings, bloom_settings, tonemapping, fxaa, projection)) in
                camera_settings_query.iter_mut().enumerate()
            {
                egui::CollapsingHeader::new(format!("Camera Settings {}", i))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.checkbox(&mut trace_settings.show_ray_steps, "Show ray steps");
                        ui.checkbox(&mut trace_settings.indirect_lighting, "Indirect lighting");
                        ui.checkbox(&mut trace_settings.shadows, "Shadows");
                        ui.checkbox(&mut trace_settings.misc_bool, "Misc");
                        ui.add(Slider::new(&mut trace_settings.misc_float, 0.0..=1.0).text("Misc"));
                        if let Some(bloom_settings) = bloom_settings {
                            ui.add(
                                Slider::new(&mut bloom_settings.into_inner().intensity, 0.0..=1.0)
                                    .text("Bloom"),
                            );
                        }
                        if let Some(mut tonemapping) = tonemapping {
                            egui::ComboBox::from_label("")
                                .selected_text(format!("{:?}", tonemapping.as_mut()))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::AcesFitted,
                                        "AcesFitted",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::AgX,
                                        "AgX",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::BlenderFilmic,
                                        "BlenderFilmic",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::Reinhard,
                                        "Reinhard",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::ReinhardLuminance,
                                        "ReinhardLuminance",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::SomewhatBoringDisplayTransform,
                                        "SomewhatBoringDisplayTransform",
                                    );
                                    ui.selectable_value(
                                        tonemapping.as_mut(),
                                        Tonemapping::None,
                                        "None",
                                    );
                                });
                        }
                        if let Some(fxaa) = fxaa {
                            ui.checkbox(&mut fxaa.into_inner().enabled, "FXAA");
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
                character.translation = Vec3::new(-0.5151253, -0.124093756, 0.77565575);
                character.rotation =
                    Quat::from_array([0.041948874, 0.17606115, -0.0075151, 0.9834563]);
            }
            if ui.button("go to pos2").clicked() {
                character.translation = Vec3::new(-0.26596254, 0.31184837, 0.95114636);
                character.rotation =
                    Quat::from_array([-0.24431482, 0.2940709, 0.07802672, 0.9207303]);
            }
        });
}
