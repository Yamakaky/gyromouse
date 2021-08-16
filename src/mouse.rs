use cgmath::{vec2, Vector2, Zero};
use enigo::{Enigo, MouseControllable};

use crate::config::settings::MouseSettings;

#[derive(Debug)]
pub struct Mouse {
    enigo: Enigo,
    error_accumulator: Vector2<f64>,
}

impl Clone for Mouse {
    fn clone(&self) -> Self {
        Self { ..Self::new() }
    }
}

impl Mouse {
    pub fn new() -> Self {
        Mouse {
            enigo: Enigo::new(),
            error_accumulator: Vector2::zero(),
        }
    }

    // mouse movement is pixel perfect, so we keep track of the error.
    pub fn mouse_move_relative(&mut self, settings: &MouseSettings, mut offset: Vector2<f64>) {
        offset *= settings.real_world_calibration * settings.in_game_sens;
        let sum = offset + self.error_accumulator;
        let rounded = vec2(sum.x.round(), sum.y.round());
        self.error_accumulator = sum - rounded;
        if let Some(rounded) = rounded.cast::<i32>() {
            if rounded != Vector2::zero() {
                self.enigo
                    .mouse_move_relative(rounded.x as i32, -rounded.y as i32);
            }
        }
    }

    pub fn enigo(&mut self) -> &mut Enigo {
        &mut self.enigo
    }
}
