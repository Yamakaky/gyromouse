use std::time::Duration;

use egui::{
    plot::{Line, Plot, Value, Values},
    CtxRef, ScrollArea,
};
use egui_backend::{gl, EguiInputState, Painter};
use egui_sdl2_gl as egui_backend;
use sdl2::{
    event::Event,
    video::{GLContext, GLProfile, Window},
    VideoSubsystem,
};

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;

pub struct Gui {
    egui_input_state: EguiInputState,
    egui_ctx: CtxRef,
    native_pixels_per_point: f32,
    painter: Painter,
    window: Window,
    _ctx: GLContext,
    sens: f64,
    accel: bool,
    max_sens: f64,
    max_thre: f64,
    min_sens: f64,
    min_thre: f64,
    cut: bool,
    cut_speed: f64,
    cut_recov: f64,
}

impl Gui {
    pub fn new(video_subsystem: &VideoSubsystem) -> Self {
        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(GLProfile::Core);

        // OpenGL 3.2 is the minimum that we will support.
        gl_attr.set_context_version(3, 2);

        let window = video_subsystem
            .window(
                "Demo: Egui backend for SDL2 + GL",
                SCREEN_WIDTH,
                SCREEN_HEIGHT,
            )
            .opengl()
            .build()
            .unwrap();

        // Create a window context
        let ctx = window.gl_create_context().unwrap();

        let painter = egui_backend::Painter::new(&video_subsystem, SCREEN_WIDTH, SCREEN_HEIGHT);
        let egui_ctx = egui::CtxRef::default();

        debug_assert_eq!(gl_attr.context_profile(), GLProfile::Core);
        debug_assert_eq!(gl_attr.context_version(), (3, 2));

        let native_pixels_per_point = 96f32 / video_subsystem.display_dpi(0).unwrap().0;

        let egui_input_state = egui_backend::EguiInputState::new(egui::RawInput {
            screen_rect: None,
            pixels_per_point: Some(native_pixels_per_point),
            ..Default::default()
        });

        Self {
            egui_input_state,
            egui_ctx,
            native_pixels_per_point,
            painter,
            window,
            _ctx: ctx,
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
        egui_backend::input_to_egui(event, &mut self.egui_input_state);
    }

    pub fn tick(&mut self, dt: Duration) {
        self.egui_input_state.input.time = Some(dt.as_secs_f64());
        self.egui_ctx
            .begin_frame(self.egui_input_state.input.take());

        //In egui 0.10.0 we seem to be losing the value to pixels_per_point,
        //so setting it every frame now.
        //TODO: Investigate if this is the right way.
        self.egui_input_state.input.pixels_per_point = Some(self.native_pixels_per_point);

        //An example of how OpenGL can be used to draw custom stuff with egui
        //overlaying it:
        //First clear the background to something nice.
        unsafe {
            // Clear the screen to black
            gl::ClearColor(0.3, 0.6, 0.3, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }

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

        //Handle cut, copy text from egui
        if !egui_output.copied_text.is_empty() {
            egui_backend::copy_to_clipboard(&mut self.egui_input_state, egui_output.copied_text);
        }

        let paint_jobs = self.egui_ctx.tessellate(paint_cmds);

        //Note: passing a bg_color to paint_jobs will clear any previously drawn stuff.
        //Use this only if egui is being used for all drawing and you aren't mixing your own Open GL
        //drawing calls with it.
        //Since we are custom drawing an OpenGL Triangle we don't need egui to clear the background.
        self.painter.paint_jobs(
            None,
            paint_jobs,
            &self.egui_ctx.texture(),
            self.native_pixels_per_point,
        );

        self.window.gl_swap_window();
    }
}
