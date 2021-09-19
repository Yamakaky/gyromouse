use std::time::Duration;

use egui::{
    plot::{Line, Plot, Value, Values},
    CtxRef, ScrollArea,
};
use egui_sdl2_gl::EguiInputState;
use egui_wgpu_backend::{RenderPass, ScreenDescriptor};
use pollster::block_on;
use sdl2::{event::Event, video::Window, VideoSubsystem};

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;

pub struct Gui {
    egui_input_state: EguiInputState,
    egui_ctx: CtxRef,
    egui_rpass: RenderPass,
    config: wgpu::SurfaceConfiguration,
    queue: wgpu::Queue,
    device: wgpu::Device,
    native_pixels_per_point: f32,
    window: Window,
    sens: f64,
    accel: bool,
    max_sens: f64,
    max_thre: f64,
    min_sens: f64,
    min_thre: f64,
    cut: bool,
    cut_speed: f64,
    cut_recov: f64,
    surface: wgpu::Surface,
}

impl Gui {
    pub fn new(video_subsystem: &VideoSubsystem, wgpu_instance: &wgpu::Instance) -> Self {
        let window = video_subsystem
            .window(
                "Demo: Egui backend for SDL2 + GL",
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
            )
            .build()
            .unwrap();

        let surface = unsafe { wgpu_instance.create_surface(&window) };

        let adapter = block_on(wgpu_instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
        }))
        .unwrap();

        let (device, queue) = block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::default(),
                limits: wgpu::Limits::default(),
                label: None,
            },
            None,
        ))
        .unwrap();

        let (width, height) = window.size();
        let surface_format = surface.get_preferred_format(&adapter).unwrap();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        surface.configure(&device, &surface_config);

        let egui_rpass = RenderPass::new(&device, surface_format, 1);
        let egui_ctx = egui::CtxRef::default();

        let native_pixels_per_point = 96f32 / video_subsystem.display_dpi(0).unwrap().0;

        let egui_input_state = EguiInputState::new(egui::RawInput {
            screen_rect: None,
            pixels_per_point: Some(native_pixels_per_point),
            ..Default::default()
        });

        Self {
            egui_input_state,
            egui_ctx,
            queue,
            device,
            surface,
            config: surface_config,
            native_pixels_per_point,
            egui_rpass,
            window,
            sens: 1.,
            accel: true,
            min_sens: 1.,
            min_thre: 5.,
            max_sens: 2.,
            max_thre: 75.,
            cut: true,
            cut_speed: 0.,
            cut_recov: 5.,
        }
    }

    pub fn event(&mut self, event: Event) {
        match event {
            // https://github.com/ArjunNair/egui_sdl2_gl/issues/11
            Event::Window { window_id, .. } if window_id != self.window.id() => {}
            _ => egui_sdl2_gl::input_to_egui(event, &mut self.egui_input_state),
        }
    }

    pub fn tick(&mut self, dt: Duration) {
        let output_frame = match self.surface.get_current_frame() {
            Ok(frame) => frame,
            Err(e) => {
                eprintln!("Dropped frame with error: {}", e);
                return;
            }
        };
        let output_view = output_frame
            .output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.egui_input_state.input.time = Some(dt.as_secs_f64());
        self.egui_ctx
            .begin_frame(self.egui_input_state.input.take());

        //In egui 0.10.0 we seem to be losing the value to pixels_per_point,
        //so setting it every frame now.
        //TODO: Investigate if this is the right way.
        self.egui_input_state.input.pixels_per_point = Some(self.native_pixels_per_point);

        let ctx = self.egui_ctx.clone();
        egui::CentralPanel::default().show(&ctx, |ui| {
            ScrollArea::auto_sized().show(ui, |ui| {
                let mut values = vec![];
                let sens = if self.accel { self.min_sens } else { self.sens };
                if self.cut {
                    values.extend([
                        Value::new(0., 0.),
                        Value::new(self.cut_speed, 0.),
                        Value::new(self.cut_recov, sens),
                    ]);
                } else {
                    values.push(Value::new(0., sens));
                }
                if self.accel {
                    values.extend([
                        Value::new(self.min_thre, self.min_sens),
                        Value::new(self.max_thre, self.max_sens),
                        Value::new(100., self.max_sens),
                    ]);
                } else {
                    values.push(Value::new(100., self.sens));
                }
                let line = Line::new(Values::from_values(values));
                ui.add(
                    Plot::new("sens_graph")
                        .line(line)
                        .allow_drag(false)
                        .allow_zoom(false)
                        .include_y(0.)
                        .view_aspect(2.)
                        .width(700.),
                );

                ui.add(
                    egui::Slider::new(&mut self.sens, 0.1..=10.0)
                        .text("Sensitivity")
                        .fixed_decimals(1),
                )
                .on_hover_text("Sensitivity of the gyro");
                self.sens = self.sens.max(0.);
                ui.checkbox(&mut self.cut, "Enable cuttoff");
                ui.group(|ui| {
                    ui.set_enabled(self.cut);
                    ui.add(
                        egui::Slider::new(&mut self.cut_speed, 0.0..=20.0)
                            .text("Cuttoff speed")
                            .integer(),
                    )
                    .on_hover_text("Rotation speeds below this threshold are ignored");
                    self.cut_speed = self.cut_speed.clamp(0., self.cut_recov);
                    ui.add(
                        egui::Slider::new(&mut self.cut_recov, 1.0..=40.0)
                            .text("Cuttoff recovery (dps)")
                            .integer(),
                    )
                    .on_hover_text("Rotation speeds above this threshold use the usual settings");
                    self.cut_recov = self.cut_recov.max(1.);
                });
                ui.checkbox(&mut self.accel, "Enable acceleration");
                ui.group(|ui| {
                    ui.set_enabled(self.accel);
                    ui.add(
                        egui::Slider::new(&mut self.min_sens, 0.1..=self.max_sens)
                            .text("Slow sensitivity")
                            .fixed_decimals(1),
                    )
                    .on_hover_text("Min sensitivity of the gyro");
                    self.min_sens = self.min_sens.clamp(0.1, self.max_sens);
                    ui.add(
                        egui::Slider::new(&mut self.min_thre, 1.0..=self.max_thre)
                            .text("Slow threshold (dps)")
                            .integer(),
                    )
                    .on_hover_text("Threshold for slow (degree per second)");
                    self.min_thre = self.min_thre.clamp(1.0, self.max_thre);
                    if self.cut {
                        self.min_thre = self.min_thre.max(self.cut_recov);
                    }
                    ui.add(
                        egui::Slider::new(&mut self.max_sens, 0.1..=20.0)
                            .text("Fast sensitivity")
                            .fixed_decimals(1),
                    )
                    .on_hover_text("Max sensitivity of the gyro");
                    self.max_sens = self.max_sens.max(self.min_sens);
                    ui.add(
                        egui::Slider::new(&mut self.max_thre, 1.0..=100.0)
                            .text("Fast threshold (dps)")
                            .integer(),
                    )
                    .on_hover_text("Threshold for max speed (degree per second)");
                    self.max_thre = self.max_thre.max(self.min_thre);
                });
            });
        });

        let (egui_output, paint_cmds) = self.egui_ctx.end_frame();

        if !egui_output.copied_text.is_empty() {
            egui_sdl2_gl::copy_to_clipboard(&mut self.egui_input_state, egui_output.copied_text);
        }
        sdl2::mouse::Cursor::from_system(egui_sdl2_gl::translate_cursor(egui_output.cursor_icon))
            .unwrap()
            .set();

        let paint_jobs = self.egui_ctx.tessellate(paint_cmds);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("encoder"),
            });

        // Upload all resources for the GPU.
        let screen_descriptor = ScreenDescriptor {
            physical_width: self.config.width,
            physical_height: self.config.height,
            scale_factor: self.native_pixels_per_point,
        };
        self.egui_rpass
            .update_texture(&self.device, &self.queue, &self.egui_ctx.texture());
        self.egui_rpass
            .update_user_textures(&self.device, &self.queue);
        self.egui_rpass.update_buffers(
            &mut self.device,
            &mut self.queue,
            &paint_jobs,
            &screen_descriptor,
        );

        // Record all render passes.
        self.egui_rpass
            .execute(
                &mut encoder,
                &output_view,
                &paint_jobs,
                &screen_descriptor,
                Some(wgpu::Color::BLACK),
            )
            .unwrap();

        // Submit the commands.
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}
