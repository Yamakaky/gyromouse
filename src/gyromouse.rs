use cgmath::{ElementWise, InnerSpace, Vector2, Zero};
use std::{collections::VecDeque, time::Duration};

use crate::{config::settings::GyroSettings, mouse::MouseMovement};

#[derive(Debug, Default)]
pub struct GyroMouse {
    smooth_buffer: VecDeque<Vector2<f64>>,
}

impl GyroMouse {
    //    #/// Nothing applied.
    //    pub fn blank() -> GyroMouse {
    //        GyroMouse {
    //            apply_smoothing: false,
    //            smooth_threshold: 5.,
    //            smooth_buffer: VecDeque::new(),
    //
    //            apply_tightening: false,
    //            tightening_threshold: 5.,
    //
    //            apply_acceleration: false,
    //            acceleration_slow_sens: 8.,
    //            acceleration_slow_threshold: 5.,
    //            acceleration_fast_sens: 16.,
    //            acceleration_fast_threshold: 75.,
    //
    //            sensitivity: 1.,
    //        }
    //    }
    //    /// Good default values for a 2D mouse.
    //    pub fn d2() -> GyroMouse {
    //        GyroMouse {
    //            apply_smoothing: true,
    //            smooth_threshold: 5.,
    //            smooth_buffer: [Vector2::zero(); 25].iter().cloned().collect(),
    //
    //            apply_tightening: true,
    //            tightening_threshold: 5.,
    //
    //            apply_acceleration: true,
    //            acceleration_slow_sens: 16.,
    //            acceleration_slow_threshold: 5.,
    //            acceleration_fast_sens: 32.,
    //            acceleration_fast_threshold: 75.,
    //
    //            sensitivity: 32.,
    //        }
    //    }
    //
    //    /// Good default values for a 3D mouse.
    //    pub fn d3() -> GyroMouse {
    //        GyroMouse {
    //            apply_smoothing: false,
    //            smooth_threshold: 0.,
    //            smooth_buffer: VecDeque::new(),
    //
    //            apply_tightening: false,
    //            tightening_threshold: 0.,
    //
    //            apply_acceleration: true,
    //            acceleration_slow_sens: 1.,
    //            acceleration_slow_threshold: 0.,
    //            acceleration_fast_sens: 2.,
    //            acceleration_fast_threshold: 75.,
    //
    //            sensitivity: 1.,
    //        }
    //    }
    //

    /// Process a new gyro sample.
    ///
    /// Parameter is pitch + yaw.
    ///
    /// Updates `self.orientation` and returns the applied change.
    ///
    /// `orientation` and return value have origin in bottom left.
    pub fn process(
        &mut self,
        settings: &GyroSettings,
        mut rot: Vector2<f64>,
        dt: Duration,
    ) -> MouseMovement {
        if settings.smooth_threshold > 0. {
            rot = self.tiered_smooth(settings, rot, dt);
        }
        if settings.cutoff_recovery > 0. {
            #[allow(clippy::float_cmp)]
            {
                assert_eq!(settings.cutoff_speed, 0.);
            }
            rot = self.tight(settings, rot);
        }
        let sens = self.get_sens(settings, rot);
        MouseMovement::from_vec_deg(rot.mul_element_wise(sens) * dt.as_secs_f64())
    }

    fn tiered_smooth(
        &mut self,
        settings: &GyroSettings,
        rot: Vector2<f64>,
        dt: Duration,
    ) -> Vector2<f64> {
        let thresh_high = settings.smooth_threshold;
        let thresh_low = thresh_high / 2.;
        let magnitude = (rot.x.powf(2.) + rot.y.powf(2.)).sqrt();
        let weight = ((magnitude - thresh_low) / (thresh_high - thresh_low))
            .max(0.)
            .min(1.);
        let smoothed = self.smooth(settings, rot * (1. - weight), dt);
        rot * weight + smoothed
    }

    fn smooth(&mut self, settings: &GyroSettings, rot: Vector2<f64>, dt: Duration) -> Vector2<f64> {
        self.smooth_buffer.push_front(rot);
        while dt * self.smooth_buffer.len() as u32 > settings.smooth_time {
            self.smooth_buffer.pop_back();
        }
        let sum = self
            .smooth_buffer
            .iter()
            .fold(Vector2::zero(), |acc, x| acc + x);
        sum / self.smooth_buffer.len() as f64
    }

    fn tight(&mut self, settings: &GyroSettings, rot: Vector2<f64>) -> Vector2<f64> {
        let magnitude = (rot.x.powf(2.) + rot.y.powf(2.)).sqrt();
        if magnitude < settings.cutoff_recovery {
            let scale = magnitude / settings.cutoff_recovery;
            rot * scale
        } else {
            rot
        }
    }

    fn get_sens(&self, settings: &GyroSettings, rot: Vector2<f64>) -> Vector2<f64> {
        if settings.slow_sens.magnitude2() > 0. && settings.slow_sens.magnitude2() > 0. {
            let magnitude = (rot.x.powf(2.) + rot.y.powf(2.)).sqrt();
            let factor = ((magnitude - settings.slow_threshold)
                / (settings.fast_threshold - settings.slow_threshold))
                .max(0.)
                .min(1.);
            settings.slow_sens * (1. - factor) + settings.fast_sens * factor
        } else {
            settings.sens
        }
    }
}
