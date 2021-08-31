use std::time::Duration;

use egui::{vec2, Color32, CtxRef, Image, Pos2, Rect, TextureId};
use egui_backend::{gl, EguiInputState, Painter};
use egui_sdl2_gl as egui_backend;
use sdl2::{
    event::Event,
    video::{GLContext, GLProfile, Window},
};

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;

const PIC_WIDTH: i32 = 320;
const PIC_HEIGHT: i32 = 192;

pub struct Gui {
    egui_input_state: EguiInputState,
    egui_ctx: CtxRef,
    native_pixels_per_point: f32,
    sine_shift: f32,
    test_str: String,
    painter: Painter,
    chip8_tex_id: TextureId,
    window: Window,
    amplitude: f32,
    _ctx: GLContext,
}

impl Gui {
    pub fn new(sdl: &sdl2::Sdl) -> Self {
        let video_subsystem = sdl.video().unwrap();

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

        let mut painter = egui_backend::Painter::new(&video_subsystem, SCREEN_WIDTH, SCREEN_HEIGHT);
        let egui_ctx = egui::CtxRef::default();

        debug_assert_eq!(gl_attr.context_profile(), GLProfile::Core);
        debug_assert_eq!(gl_attr.context_version(), (3, 2));

        let native_pixels_per_point = 96f32 / video_subsystem.display_dpi(0).unwrap().0;

        let (width, height) = window.size();

        let egui_input_state = egui_backend::EguiInputState::new(egui::RawInput {
            screen_rect: Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                vec2(width as f32, height as f32) / native_pixels_per_point,
            )),
            pixels_per_point: Some(native_pixels_per_point),
            ..Default::default()
        });
        let mut srgba: Vec<Color32> = Vec::new();

        //For now we will just set everything to black, because
        //we will be updating it dynamically later. However, this could just as
        //easily have been some actual picture data loaded in.
        for _ in 0..PIC_HEIGHT {
            for _ in 0..PIC_WIDTH {
                srgba.push(Color32::BLACK);
            }
        }

        //The user texture is what allows us to mix Egui and GL rendering contexts.
        //Egui just needs the texture id, as the actual texture is managed by the backend.
        let chip8_tex_id =
            painter.new_user_texture((PIC_WIDTH as usize, PIC_HEIGHT as usize), &srgba, false);

        //Some variables to help draw a sine wave
        let sine_shift = 0f32;

        let test_str: String =
            "A text box to write in. Cut, copy, paste commands are available.".to_owned();
        let amplitude: f32 = 50f32;

        Self {
            egui_input_state,
            egui_ctx,
            native_pixels_per_point,
            chip8_tex_id,
            sine_shift,
            test_str,
            painter,
            window,
            amplitude,
            _ctx: ctx,
        }
    }

    pub fn event(&mut self, event: Event) {
        egui_backend::input_to_egui(event, &mut self.egui_input_state);
    }

    pub fn tick(&mut self, dt: Duration) -> bool {
        dbg!(dt);
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

        let mut srgba: Vec<Color32> = Vec::new();
        let mut angle = 0f32;
        //Draw a cool sine wave in a buffer.
        for y in 0..PIC_HEIGHT {
            for x in 0..PIC_WIDTH {
                srgba.push(Color32::BLACK);
                if y == PIC_HEIGHT - 1 {
                    let y = self.amplitude * (angle * 3.142f32 / 180f32 + self.sine_shift).sin();
                    let y = PIC_HEIGHT as f32 / 2f32 - y;
                    srgba[(y as i32 * PIC_WIDTH + x) as usize] = Color32::YELLOW;
                    angle += 360f32 / PIC_WIDTH as f32;
                }
            }
        }
        self.sine_shift += 0.1f32;

        //This updates the previously initialized texture with new data.
        //If we weren't updating the texture, this call wouldn't be required.
        self.painter
            .update_user_texture_data(self.chip8_tex_id, &srgba);

        let mut quit = false;
        let ctx = self.egui_ctx.clone();
        egui::Window::new("Egui with SDL2 and GL").show(&ctx, |ui| {
            //Image just needs a texture id reference, so we just pass it the texture id that was returned to us
            //when we previously initialized the texture.
            ui.add(Image::new(
                self.chip8_tex_id,
                vec2(PIC_WIDTH as f32, PIC_HEIGHT as f32),
            ));
            ui.separator();
            ui.label(
    "A simple sine wave plotted onto a GL texture then blitted to an egui managed Image.",
            );
            ui.label(" ");
            ui.text_edit_multiline(&mut self.test_str);
            ui.label(" ");

            ui.add(egui::Slider::new(&mut self.amplitude, 0.0..=50.0).text("Amplitude"));
            ui.label(" ");
            if ui.button("Quit").clicked() {
                quit = true;
            }
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

        return quit;
    }
}
