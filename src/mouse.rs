use std::ops::AddAssign;

use cgmath::{vec2, Deg, Vector2, Zero};
use enigo::{Enigo, MouseControllable};

use crate::config::settings::MouseSettings;

#[derive(Debug, Clone, Copy)]
pub struct MouseMovement {
    /// Horizontal axis, + to the right
    x: Deg<f64>,
    /// Vertical axis, + to the top
    y: Deg<f64>,
}

impl MouseMovement {
    pub fn new(x: Deg<f64>, y: Deg<f64>) -> Self {
        Self { x, y }
    }
    pub fn zero() -> Self {
        Self::new(Deg(0.), Deg(0.))
    }
    /// Convert a Vector2 with degree movement values
    pub fn from_vec_deg(vec: Vector2<f64>) -> Self {
        Self {
            x: Deg(vec.x),
            y: Deg(vec.y),
        }
    }
}

impl AddAssign for MouseMovement {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

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
        #[allow(unused_mut)]
        let mut enigo = Enigo::new();
        // Lower delay for xdo, see #1
        #[cfg(target_os = "linux")]
        enigo.set_delay(100);
        Mouse {
            enigo,
            error_accumulator: Vector2::zero(),
        }
    }

    // mouse movement is pixel perfect, so we keep track of the error.
    pub fn mouse_move_relative(&mut self, settings: &MouseSettings, offset: MouseMovement) {
        let offset_pixel =
            vec2(offset.x.0, -offset.y.0) * settings.real_world_calibration * settings.in_game_sens;
        self.mouse_move_relative_pixel(offset_pixel);
    }

    pub fn mouse_move_relative_pixel(&mut self, offset: Vector2<f64>) {
        let sum = offset + self.error_accumulator;
        let rounded = vec2(sum.x.round(), sum.y.round());
        self.error_accumulator = sum - rounded;
        if let Some(rounded) = rounded.cast::<i32>() {
            if rounded != Vector2::zero() {
                // In enigo, +y is toward the bottom
                self.enigo.mouse_move_relative(rounded.x, rounded.y);
            }
        }
    }

    pub fn mouse_move_absolute_pixel(&mut self, offset: Vector2<i32>) {
        self.enigo.mouse_move_to(offset.x, offset.y);
    }

    pub fn enigo(&mut self) -> &mut Enigo {
        &mut self.enigo
    }
}
