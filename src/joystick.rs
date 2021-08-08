#![allow(dead_code)]

use std::time::Instant;

use cgmath::{vec2, AbsDiffEq, Angle, Deg, InnerSpace, Rad, Vector2};

use crate::{
    config::{settings::Settings, types::RingMode},
    mapping::{Buttons, VirtualKey},
    mouse::Mouse,
};

pub trait Stick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        settings: &Settings,
        bindings: &mut Buttons,
        mouse: &mut Mouse,
        now: Instant,
    );
}

pub struct CameraStick {
    current_speed: f64,
}

impl Default for CameraStick {
    fn default() -> Self {
        CameraStick { current_speed: 0. }
    }
}

impl Stick for CameraStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        _now: Instant,
    ) {
        let s = &settings.stick_settings;
        // TODO: use dt instead of fixed rate 66Hz
        let amp = stick.magnitude();
        let amp_zones = (amp - s.deadzone) / (s.fullzone - s.deadzone);
        if amp_zones >= 1. {
            self.current_speed = (self.current_speed + s.aim_stick.acceleration_rate / 66.)
                .min(s.aim_stick.acceleration_cap);
        } else {
            self.current_speed = 0.;
        }
        let amp_clamped = amp_zones.max(0.).min(1.);
        let amp_exp = amp_clamped.powf(s.aim_stick.power);
        mouse.mouse_move_relative(
            &settings.mouse,
            s.aim_stick.sens_dps / 66. * (1. + self.current_speed) * stick.normalize_to(amp_exp),
        );
    }
}

#[derive(Debug)]
enum FlickStickState {
    Center,
    Flicking {
        flick_start: Instant,
        last: Deg<f64>,
        target: Deg<f64>,
    },
    Rotating {
        old_rotation: Deg<f64>,
    },
}

#[derive(Debug)]
pub struct FlickStick {
    state: FlickStickState,
    do_rotate: bool,
    do_flick: bool,
}

impl Default for FlickStick {
    fn default() -> Self {
        FlickStick {
            state: FlickStickState::Center,
            do_rotate: true,
            do_flick: true,
        }
    }
}

impl FlickStick {
    pub fn new(flick: bool, rotate: bool) -> Self {
        Self {
            state: FlickStickState::Center,
            do_rotate: rotate,
            do_flick: flick,
        }
    }
}

impl Stick for FlickStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        now: Instant,
    ) {
        let s = &settings.stick_settings;
        let offset = match self.state {
            FlickStickState::Center | FlickStickState::Rotating { .. }
                if stick.magnitude() < s.fullzone =>
            {
                self.state = FlickStickState::Center;
                None
            }
            FlickStickState::Center => {
                let target = stick.angle(Vector2::unit_y()).into();
                self.state = if self.do_flick {
                    FlickStickState::Flicking {
                        flick_start: now,
                        last: Deg(0.),
                        target,
                    }
                } else {
                    FlickStickState::Rotating {
                        old_rotation: target,
                    }
                };
                None
            }
            FlickStickState::Flicking {
                flick_start,
                ref mut last,
                target,
            } => {
                let elapsed = now.duration_since(flick_start).as_secs_f64();
                let max = s.flick_stick.flick_time.as_secs_f64() * target.0.abs() / 180.;
                let dt_factor = elapsed / max;
                let current_angle = target * dt_factor.min(1.);
                let delta = current_angle - *last;
                if dt_factor > 1. {
                    self.state = FlickStickState::Rotating {
                        old_rotation: current_angle,
                    };
                } else {
                    *last = current_angle;
                }
                Some(delta.normalize_signed())
            }
            FlickStickState::Rotating {
                ref mut old_rotation,
            } => {
                if self.do_rotate {
                    let angle = stick.angle(Vector2::unit_y()).into();
                    let delta = angle - *old_rotation;
                    *old_rotation = angle;
                    Some(delta.normalize_signed())
                } else {
                    None
                }
            }
        };
        if let Some(offset) = offset {
            mouse.mouse_move_relative(&settings.mouse, vec2(offset.0, 0.));
        }
    }
}

pub struct ButtonStick {
    left: bool,
    angle: Deg<f64>,
    ring_mode: RingMode,
}

impl ButtonStick {
    pub fn left(ring_mode: RingMode) -> Self {
        Self {
            left: true,
            angle: Deg(30.),
            ring_mode,
        }
    }

    pub fn right(ring_mode: RingMode) -> Self {
        Self {
            left: false,
            angle: Deg(30.),
            ring_mode,
        }
    }
}

impl Stick for ButtonStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        settings: &Settings,
        bindings: &mut Buttons,
        _mouse: &mut Mouse,
        _now: Instant,
    ) {
        let settings = &settings.stick_settings;
        let amp = stick.magnitude();
        let amp_zones = (amp - settings.deadzone) / (settings.fullzone - settings.deadzone);
        let amp_clamped = amp_zones.max(0.).min(1.);
        let stick = stick.normalize_to(amp_clamped);
        let now = std::time::Instant::now();

        let epsilon = Rad::from(Deg(90.) - self.angle).0;

        let angle_r = stick.angle(Vector2::unit_x());
        let angle_l = stick.angle(-Vector2::unit_x());
        let angle_u = stick.angle(Vector2::unit_y());
        let angle_d = stick.angle(-Vector2::unit_y());

        if amp_clamped > 0. {
            bindings.key(
                if self.left {
                    VirtualKey::LRing
                } else {
                    VirtualKey::RRing
                },
                match self.ring_mode {
                    RingMode::Inner => amp_clamped < 1.,
                    RingMode::Outer => amp_clamped >= 1.,
                },
                now,
            );
            bindings.key(
                if self.left {
                    VirtualKey::LRight
                } else {
                    VirtualKey::RRight
                },
                angle_r.abs_diff_eq(&Rad(0.), epsilon),
                now,
            );
            bindings.key(
                if self.left {
                    VirtualKey::LLeft
                } else {
                    VirtualKey::RLeft
                },
                angle_l.abs_diff_eq(&Rad(0.), epsilon),
                now,
            );
            bindings.key(
                if self.left {
                    VirtualKey::LUp
                } else {
                    VirtualKey::RUp
                },
                angle_u.abs_diff_eq(&Rad(0.), epsilon),
                now,
            );
            bindings.key(
                if self.left {
                    VirtualKey::LDown
                } else {
                    VirtualKey::RDown
                },
                angle_d.abs_diff_eq(&Rad(0.), epsilon),
                now,
            );
        } else if self.left {
            bindings.key_up(VirtualKey::LLeft, now);
            bindings.key_up(VirtualKey::LRight, now);
            bindings.key_up(VirtualKey::LUp, now);
            bindings.key_up(VirtualKey::LDown, now);
        } else {
            bindings.key_up(VirtualKey::RLeft, now);
            bindings.key_up(VirtualKey::RRight, now);
            bindings.key_up(VirtualKey::RUp, now);
            bindings.key_up(VirtualKey::RDown, now);
        }
    }
}
