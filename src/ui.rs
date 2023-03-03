use crate::render_pipeline::MainPassSettings;
use bevy::{
    core_pipeline::{bloom::BloomSettings, fxaa::Fxaa, tonemapping::Tonemapping},
    prelude::*,
};
use bevy_egui::{
    egui::{self, Slider},
    EguiContext, EguiPlugin,
};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(EguiPlugin).add_system(ui_system);
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
) {
    egui::Window::new("Settings")
        .anchor(egui::Align2::RIGHT_TOP, [-5.0, 5.0])
        .show(egui_context.ctx_mut(), |ui| {
            for (i, (mut trace_settings, bloom_settings, tonemapping, fxaa, projection)) in
                camera_settings_query.iter_mut().enumerate()
            {
                egui::CollapsingHeader::new(format!("Camera Settings {}", i)).default_open(true).show(ui, |ui| {
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
                    if let Some(tonemapping) = tonemapping {
                        let mut state = match tonemapping.as_ref() {
                            Tonemapping::Enabled { .. } => true,
                            Tonemapping::Disabled => false,
                        };
                        ui.checkbox(&mut state, "Tonemapping");
                        match state {
                            true => {
                                *tonemapping.into_inner() = Tonemapping::Enabled {
                                    deband_dither: true,
                                };
                            }
                            false => {
                                *tonemapping.into_inner() = Tonemapping::Disabled;
                            }
                        }
                    }
                    if let Some(fxaa) = fxaa {
                        ui.checkbox(&mut fxaa.into_inner().enabled, "FXAA");
                    }
                    if let Some(projection) = projection {
                        match projection.into_inner() {
                            Projection::Orthographic(orthographic_projection) => {
                                ui.add(
                                    Slider::new(&mut orthographic_projection.scale, 0.0..=1000.0)
                                        .text("Orthographic scale"),
                                );
                            }
                            Projection::Perspective(perspective_projection) => {
                                ui.add(
                                    Slider::new(&mut perspective_projection.fov, 0.0..=3.1415)
                                        .logarithmic(true)
                                        .text("Perspective fov"),
                                );
                            }
                        }
                    }
                });
            }
        });
}
