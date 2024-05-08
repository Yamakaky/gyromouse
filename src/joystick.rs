#![allow(dead_code)]

use std::time::{Duration, Instant};

use cgmath::{vec2, AbsDiffEq, Angle, Deg, ElementWise, InnerSpace, Rad, Vector2, Zero};
use enigo::{Axis, Mouse as _};

use crate::{
    config::{settings::Settings, types::RingMode},
    mapping::{Buttons, VirtualKey},
    mouse::{Mouse, MouseMovement},
};

pub trait Stick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        side: StickSide,
        settings: &Settings,
        bindings: &mut Buttons,
        mouse: &mut Mouse,
        now: Instant,
        dt: Duration,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StickSide {
    Left,
    Right,
    Motion,
}

impl StickSide {
    fn left(self) -> VirtualKey {
        match self {
            StickSide::Left => VirtualKey::LLeft,
            StickSide::Right => VirtualKey::RLeft,
            StickSide::Motion => VirtualKey::MLeft,
        }
    }

    fn right(self) -> VirtualKey {
        match self {
            StickSide::Left => VirtualKey::LRight,
            StickSide::Right => VirtualKey::RRight,
            StickSide::Motion => VirtualKey::MRight,
        }
    }

    fn up(self) -> VirtualKey {
        match self {
            StickSide::Left => VirtualKey::LUp,
            StickSide::Right => VirtualKey::RUp,
            StickSide::Motion => VirtualKey::MUp,
        }
    }

    fn down(self) -> VirtualKey {
        match self {
            StickSide::Left => VirtualKey::LDown,
            StickSide::Right => VirtualKey::RDown,
            StickSide::Motion => VirtualKey::MDown,
        }
    }

    fn ring(self) -> VirtualKey {
        match self {
            StickSide::Left => VirtualKey::LRing,
            StickSide::Right => VirtualKey::RRing,
            StickSide::Motion => VirtualKey::MRing,
        }
    }
}

pub struct CameraStick {
    current_speed: f64,
}

impl CameraStick {
    pub fn new() -> Self {
        CameraStick { current_speed: 0. }
    }
}

impl Stick for CameraStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        side: StickSide,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        _now: Instant,
        dt: Duration,
    ) {
        // TODO: check settings semantic
        let s = &settings.stick;
        let amp = stick.magnitude();
        let amp_zones = (amp - s.deadzone) / (s.fullzone - s.deadzone);
        if amp_zones >= 1. {
            self.current_speed = (self.current_speed + s.aim.acceleration_rate * dt.as_secs_f64())
                .min(s.aim.acceleration_cap);
        } else {
            self.current_speed = 0.;
        }
        let amp_clamped = amp_zones.max(0.).min(1.);
        let amp_exp = amp_clamped.powf(s.aim.power);
        if stick.magnitude2() > 0. {
            let mut offset = stick.normalize_to(amp_exp)
                * s.aim.sens_dps
                * ((1. + self.current_speed) * dt.as_secs_f64());
            offset.mul_assign_element_wise(
                match side {
                    StickSide::Left => s.aim.left_axis,
                    StickSide::Right => s.aim.right_axis,
                    StickSide::Motion => s.motion.axis,
                }
                .cast::<f64>()
                .unwrap(),
            );
            mouse.mouse_move_relative(&settings.mouse, MouseMovement::from_vec_deg(offset));
        }
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
        _side: StickSide,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        now: Instant,
        _dt: Duration,
    ) {
        let s = &settings.stick;
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
                let max = s.flick.flick_time.as_secs_f64() * target.0.abs() / 180.;
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
            mouse.mouse_move_relative(&settings.mouse, MouseMovement::new(offset, Deg(0.)));
        }
    }
}

pub struct ButtonStick {
    angle: Deg<f64>,
    ring_mode: RingMode,
}

impl ButtonStick {
    pub fn new(ring_mode: RingMode) -> Self {
        Self {
            angle: Deg(30.),
            ring_mode,
        }
    }
}

impl Stick for ButtonStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        side: StickSide,
        settings: &Settings,
        bindings: &mut Buttons,
        _mouse: &mut Mouse,
        now: Instant,
        _dt: Duration,
    ) {
        let settings = &settings.stick;
        let amp = stick.magnitude();
        let amp_zones = (amp - settings.deadzone) / (settings.fullzone - settings.deadzone);
        let amp_clamped = amp_zones.max(0.).min(1.);

        if amp_clamped > 0. {
            let stick = stick.normalize_to(amp_clamped);

            let epsilon = Rad::from(Deg(90.) - self.angle).0;

            let angle_r = stick.angle(Vector2::unit_x());
            let angle_l = stick.angle(-Vector2::unit_x());
            let angle_u = stick.angle(Vector2::unit_y());
            let angle_d = stick.angle(-Vector2::unit_y());

            bindings.key(
                side.ring(),
                match self.ring_mode {
                    RingMode::Inner => amp_clamped < 1.,
                    RingMode::Outer => amp_clamped >= 1.,
                },
                now,
            );
            bindings.key(side.right(), angle_r.abs_diff_eq(&Rad(0.), epsilon), now);
            bindings.key(side.left(), angle_l.abs_diff_eq(&Rad(0.), epsilon), now);
            bindings.key(side.up(), angle_u.abs_diff_eq(&Rad(0.), epsilon), now);
            bindings.key(side.down(), angle_d.abs_diff_eq(&Rad(0.), epsilon), now);
        } else {
            bindings.key_up(side.left(), now);
            bindings.key_up(side.right(), now);
            bindings.key_up(side.up(), now);
            bindings.key_up(side.down(), now);
        }
    }
}

pub struct AreaStick {
    snap: bool,
    last_location: Vector2<i32>,
    last_offset: Vector2<f64>,
}

impl AreaStick {
    pub fn area() -> Self {
        Self {
            snap: false,
            last_location: Vector2::zero(),
            last_offset: Vector2::zero(),
        }
    }

    pub fn ring() -> Self {
        Self {
            snap: true,
            last_location: Vector2::zero(),
            last_offset: Vector2::zero(),
        }
    }
}

impl Stick for AreaStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        _side: StickSide,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        _now: Instant,
        _dt: Duration,
    ) {
        let radius = settings.stick.area.screen_radius as f64;
        let offset = if self.snap {
            if stick.magnitude() > settings.stick.deadzone {
                stick.normalize_to(radius)
            } else {
                Vector2::zero()
            }
        } else {
            stick * radius
        }
        .mul_element_wise(vec2(1., -1.));
        let center = settings.stick.area.screen_resolution / 2;
        let location = center.cast::<i32>().unwrap() + offset.cast::<i32>().unwrap();
        if self.snap {
            if location != self.last_location || location != Vector2::zero() {
                mouse.mouse_move_absolute_pixel(location);
            }
        } else {
            mouse.mouse_move_relative_pixel(offset.sub_element_wise(self.last_offset));
        }
        self.last_location = location;
        self.last_offset = offset;
    }
}

pub enum ScrollStick {
    Center,
    Scrolling { last: Deg<f64>, acc: f64 },
}

impl ScrollStick {
    pub fn new() -> Self {
        Self::Center
    }
}

impl Stick for ScrollStick {
    fn handle(
        &mut self,
        stick: Vector2<f64>,
        _side: StickSide,
        settings: &Settings,
        _bindings: &mut Buttons,
        mouse: &mut Mouse,
        _now: Instant,
        _dt: Duration,
    ) {
        let angle = vec2(0., 1.).angle(stick).into();
        match self {
            _ if stick.magnitude() < settings.stick.deadzone => *self = Self::Center,
            ScrollStick::Center => {
                *self = ScrollStick::Scrolling {
                    last: angle,
                    acc: 0.,
                }
            }
            ScrollStick::Scrolling { last, acc } => {
                let delta = (angle - *last).normalize_signed() / settings.stick.scroll.sens + *acc;
                let delta_rounded = delta.round();
                *acc = delta - delta_rounded;
                mouse
                    .enigo()
                    .scroll(delta_rounded as i32, Axis::Vertical)
                    .unwrap();
                *last = angle;
            }
        }
    }
}
