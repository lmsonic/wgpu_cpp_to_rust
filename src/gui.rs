#![allow(clippy::shadow_unrelated)]

use std::f32::consts::PI;
use std::time::Duration;

use egui::epaint::Shadow;
use egui::{Context, Ui, Vec2, Visuals};
use egui_wgpu::Renderer;
use egui_wgpu::ScreenDescriptor;

use egui_winit::State;

use glam::{Mat3, Vec3, Vec4};
use wgpu::{CommandEncoder, Device, Queue, TextureFormat, TextureView};
use winit::event::WindowEvent;
use winit::window::Window;

#[derive(Default)]
pub struct GuiState {
    pub clear_color: [f32; 3],
    pub light_direction1: Vec4,
    pub light_color1: [f32; 3],
    pub light_direction2: Vec4,
    pub light_color2: [f32; 3],
    pub hardness: f32,
    pub diffuse: f32,
    pub specular: f32,
    pub normal_strength: f32,
    pub mip_level: f32,
    pub kernel: Mat3,
    pub compute_test: f32,
}

impl GuiState {
    #[allow(clippy::shadow_unrelated)]
    pub fn gui(&mut self, ui: &Context, delta_time: Duration) {
        egui_extras::install_image_loaders(ui);
        egui::Window::new("Image")
            .resizable(true)
            .vscroll(true)
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.set_max_size(Vec2::new(500.0, 250.0));
                    ui.set_max_size(Vec2::new(1000.0, 500.0));
                    ui.image(egui::include_image!("../resources/butterfly.jpg"));
                    ui.image(egui::include_image!("../resources/sobel.png"));
                });
                ui.add(egui::Slider::new(&mut self.compute_test, 0.0..=1.0));
            });
        egui::Window::new("Lighting")
            .resizable(true)
            .vscroll(true)
            .default_open(false)
            .show(ui, |ui| {
                ui.label("Clear Color");
                ui.color_edit_button_rgb(&mut self.clear_color);

                ui.label("Light Direction 1");
                drag_direction(ui, &mut self.light_direction1);

                ui.label("Light Color 1");
                ui.color_edit_button_rgb(&mut self.light_color1);

                ui.label("Light Direction 2");
                drag_direction(ui, &mut self.light_direction2);

                ui.label("Light Color 2");
                ui.color_edit_button_rgb(&mut self.light_color2);

                ui.label("Hardness");
                ui.add(egui::Slider::new(&mut self.hardness, 0.0..=100.0));

                ui.label("Diffuse intensity");
                ui.add(egui::Slider::new(&mut self.diffuse, 0.0..=1.0));

                ui.label("Specular intensity");
                ui.add(egui::Slider::new(&mut self.specular, 0.0..=1.0));

                ui.label("Normal Strenght");
                ui.add(egui::Slider::new(&mut self.normal_strength, 0.0..=1.0));

                ui.label("Mip Level");
                ui.add(egui::Slider::new(&mut self.mip_level, 0.0..=12.0));

                ui.label(format!(
                    "Application average {} ms/frame {:.3}",
                    delta_time.as_millis(),
                    delta_time.as_secs_f32()
                ));
            });
    }
}
fn drag_direction(ui: &mut Ui, v: &mut Vec4) {
    let v3 = v.truncate();
    let mut polar = cartesian_to_polar(v3);
    ui.horizontal(|ui| {
        ui.drag_angle(&mut polar.x);
        ui.drag_angle(&mut polar.y);
    });
    polar.x = polar.x.clamp(-PI * 0.5, PI * 0.5);
    polar.y = polar.y.clamp(-PI * 0.5, PI * 0.5);
    *v = polar_to_cartesian(polar).extend(0.0);
}

fn cartesian_to_polar(cartesian: Vec3) -> Vec2 {
    let length = cartesian.length();
    let normalized = cartesian / length;
    Vec2 {
        x: normalized.y.asin(),                  // latitude
        y: (normalized.x / normalized.z).atan(), // longitude
    }
}

fn polar_to_cartesian(polar: Vec2) -> Vec3 {
    let latitude = polar.x;
    let longitude = polar.y;
    Vec3 {
        x: latitude.cos() * longitude.sin(),
        y: latitude.sin(),
        z: latitude.cos() * longitude.cos(),
    }
}

pub struct EguiRenderer {
    pub context: Context,
    state: State,
    renderer: Renderer,
}

impl EguiRenderer {
    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32,
        window: &Window,
    ) -> Self {
        const BORDER_RADIUS: f32 = 2.0;
        let egui_context = Context::default();
        let id = egui_context.viewport_id();

        let visuals = Visuals {
            window_rounding: egui::Rounding::same(BORDER_RADIUS),
            window_shadow: Shadow::NONE,
            // menu_rounding: todo!(),
            ..Default::default()
        };

        egui_context.set_visuals(visuals);

        let egui_state = egui_winit::State::new(egui_context.clone(), id, &window, None, None);

        // egui_state.set_pixels_per_point(window.scale_factor() as f32);
        let egui_renderer = egui_wgpu::Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
        );

        Self {
            context: egui_context,
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) -> bool {
        let response = self.state.on_window_event(window, event);
        response.consumed
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        window: &Window,
        window_surface_view: &TextureView,
        screen_descriptor: &ScreenDescriptor,
        run_ui: impl FnOnce(&Context),
    ) {
        // self.state.set_pixels_per_point(window.scale_factor() as f32);
        let raw_input = self.state.take_egui_input(window);
        let full_output = self.context.run(raw_input, |_| {
            run_ui(&self.context);
        });

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .context
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, screen_descriptor);
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            label: Some("egui Main Render Pass"),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer.render(&mut rpass, &tris, screen_descriptor);
        drop(rpass);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x);
        }
    }
}
