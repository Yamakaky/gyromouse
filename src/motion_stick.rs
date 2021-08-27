use cgmath::{vec2, ElementWise, InnerSpace, Rad};

use crate::{
    config::settings::Settings,
    joystick::{Stick, StickSide},
};

pub struct MotionStick {
    stick: Box<dyn Stick>,
}

impl MotionStick {
    pub fn new(settings: &Settings) -> Self {
        Self {
            stick: settings.new_motion_stick(),
        }
    }
}

impl MotionStick {
    pub fn handle(
        &mut self,
        up_vector: cgmath::Vector3<f64>,
        settings: &Settings,
        bindings: &mut crate::mapping::Buttons,
        mouse: &mut crate::mouse::Mouse,
        now: std::time::Instant,
        dt: std::time::Duration,
    ) {
        let up_vector = up_vector.normalize();
        let mut stick = vec2(-up_vector.x.asin(), up_vector.z.asin())
            .mul_element_wise(settings.stick.motion.axis.cast().expect("cannot fail"));

        //let deadzone = Rad::from(settings.stick.motion.deadzone).0;
        let deadzone = 0.;
        let fullzone = Rad::from(settings.stick.motion.fullzone).0;
        let amp = stick.magnitude();
        let amp_zones = (amp - deadzone) / (fullzone - deadzone);
        let amp_clamped = amp_zones.max(0.).min(1.);
        if amp_clamped > 0. {
            stick = stick.normalize_to(amp_clamped);
        }

        // TODO: Fix motion stick deadzone usage
        // `stick.handle` will apply its own deadzone setting to our calibrated input,
        // thinking it's a raw value.
        self.stick
            .handle(stick, StickSide::Motion, settings, bindings, mouse, now, dt)
    }
}
