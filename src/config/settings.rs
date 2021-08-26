use std::time::Duration;

use cgmath::{vec2, Deg, Vector2, Zero};

use crate::joystick::*;

use super::types::*;

#[derive(Debug, Clone)]
pub struct Settings {
    pub gyro: GyroSettings,
    pub stick: StickSettings,
    pub left_stick_mode: StickMode,
    pub right_stick_mode: StickMode,
    pub left_ring_mode: RingMode,
    pub right_ring_mode: RingMode,
    pub trigger_threshold: f64,
    pub zl_mode: TriggerMode,
    pub zr_mode: TriggerMode,
    pub mouse: MouseSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gyro: GyroSettings::default(),
            stick: StickSettings::default(),
            left_stick_mode: StickMode::NoMouse,
            right_stick_mode: StickMode::Aim,
            left_ring_mode: RingMode::Outer,
            right_ring_mode: RingMode::Outer,
            trigger_threshold: 0.5,
            zl_mode: TriggerMode::NoFull,
            zr_mode: TriggerMode::NoFull,
            mouse: MouseSettings::default(),
        }
    }
}

impl Settings {
    pub fn apply(&mut self, setting: Setting) {
        match setting {
            Setting::Gyro(s) => self.gyro.apply(s),
            Setting::Stick(s) => self.stick.apply(s),
            Setting::LeftStickMode(m) => self.left_stick_mode = m,
            Setting::RightStickMode(m) => self.right_stick_mode = m,
            Setting::LeftRingMode(m) => self.left_ring_mode = m,
            Setting::RightRingMode(m) => self.right_ring_mode = m,
            Setting::TriggerThreshold(t) => self.trigger_threshold = t,
            Setting::ZLMode(m) => self.zl_mode = m,
            Setting::ZRMode(m) => self.zr_mode = m,
            Setting::Mouse(m) => self.mouse.apply(m),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn new_left_stick(&self) -> Box<dyn Stick> {
        self.new_stick(self.left_stick_mode, true)
    }

    pub fn new_right_stick(&self) -> Box<dyn Stick> {
        self.new_stick(self.right_stick_mode, false)
    }

    fn new_stick(&self, mode: StickMode, left: bool) -> Box<dyn Stick> {
        match mode {
            StickMode::Aim => Box::new(CameraStick::default()),
            StickMode::Flick | StickMode::FlickOnly | StickMode::RotateOnly => {
                let flick = mode != StickMode::RotateOnly;
                let rotate = mode != StickMode::FlickOnly;
                Box::new(FlickStick::new(flick, rotate))
            }
            StickMode::MouseRing => Box::new(AreaStick::ring()),
            StickMode::MouseArea => Box::new(AreaStick::area()),
            StickMode::NoMouse => Box::new(if left {
                ButtonStick::left(self.left_ring_mode)
            } else {
                ButtonStick::right(self.right_ring_mode)
            }),
            StickMode::ScrollWheel => todo!("Scoll wheel stick is unimplemented for now"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StickSettings {
    pub deadzone: f64,
    pub fullzone: f64,
    pub aim: AimStickSettings,
    pub flick: FlickStickSettings,
    pub scroll: ScrollStickSettings,
    pub area: AreaStickSettings,
}

impl Default for StickSettings {
    fn default() -> Self {
        Self {
            deadzone: 0.15,
            fullzone: 0.9,
            aim: Default::default(),
            flick: Default::default(),
            scroll: Default::default(),
            area: Default::default(),
        }
    }
}

impl StickSettings {
    fn apply(&mut self, setting: StickSetting) {
        match setting {
            StickSetting::Deadzone(d) => self.deadzone = d,
            StickSetting::FullZone(d) => self.fullzone = d,
            StickSetting::Aim(s) => self.aim.apply(s),
            StickSetting::Flick(s) => self.flick.apply(s),
            StickSetting::Scroll(s) => self.scroll.apply(s),
            StickSetting::Area(s) => self.area.apply(s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AimStickSettings {
    pub sens_dps: f64,
    pub power: f64,
    pub invert_x: bool,
    pub invert_y: bool,
    pub acceleration_rate: f64,
    pub acceleration_cap: f64,
}

impl Default for AimStickSettings {
    fn default() -> Self {
        Self {
            sens_dps: 360.,
            power: 1.,
            invert_x: false,
            invert_y: false,
            acceleration_rate: 0.,
            acceleration_cap: 1000000.,
        }
    }
}

impl AimStickSettings {
    fn apply(&mut self, setting: AimStickSetting) {
        match setting {
            AimStickSetting::Sens(s) => self.sens_dps = s,
            AimStickSetting::Power(s) => self.power = s,
            AimStickSetting::InvertX(v) => self.invert_x = v == InvertMode::Inverted,
            AimStickSetting::InvertY(v) => self.invert_y = v == InvertMode::Inverted,
            AimStickSetting::AccelerationRate(s) => self.acceleration_rate = s,
            AimStickSetting::AccelerationCap(s) => self.acceleration_cap = s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FlickStickSettings {
    pub flick_time: Duration,
    pub exponent: f64,
    pub forward_deadzone_arc: Deg<f64>,
}

impl Default for FlickStickSettings {
    fn default() -> Self {
        Self {
            flick_time: Duration::from_millis(100),
            exponent: 0.,
            forward_deadzone_arc: Deg(0.),
        }
    }
}

impl FlickStickSettings {
    fn apply(&mut self, setting: FlickStickSetting) {
        match setting {
            FlickStickSetting::FlickTime(s) => self.flick_time = s,
            FlickStickSetting::Exponent(s) => self.exponent = s,
            FlickStickSetting::ForwardDeadzoneArc(s) => self.forward_deadzone_arc = s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScrollStickSettings {
    pub sens: Deg<f64>,
}

impl Default for ScrollStickSettings {
    fn default() -> Self {
        Self { sens: Deg(10.) }
    }
}

impl ScrollStickSettings {
    fn apply(&mut self, setting: ScrollStickSetting) {
        match setting {
            ScrollStickSetting::Sens(s) => self.sens = s,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AreaStickSettings {
    pub screen_resolution: Vector2<u32>,
    pub screen_radius: u32,
}

impl Default for AreaStickSettings {
    fn default() -> Self {
        Self {
            screen_resolution: vec2(1920, 1080),
            screen_radius: 50,
        }
    }
}

impl AreaStickSettings {
    fn apply(&mut self, setting: AreaStickSetting) {
        match setting {
            AreaStickSetting::ScreenResolutionX(r) => self.screen_resolution.x = r,
            AreaStickSetting::ScreenResolutionY(r) => self.screen_resolution.y = r,
            AreaStickSetting::Radius(r) => self.screen_radius = r,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GyroSettings {
    /// Sensitivity to use without acceleration.
    ///
    /// <http://gyrowiki.jibbsmart.com/blog:good-gyro-controls-part-1:the-gyro-is-a-mouse#toc5>
    pub sens: Vector2<f64>,
    pub invert: (bool, bool),
    pub space: GyroSpace,
    /// Stabilize slow movements
    ///
    /// <http://gyrowiki.jibbsmart.com/blog:good-gyro-controls-part-1:the-gyro-is-a-mouse#toc9>
    pub cutoff_speed: f64,
    pub cutoff_recovery: f64,
    /// Smoothing threshold.
    ///
    /// Rotations smaller than this will be smoothed over a small period of time.
    pub smooth_threshold: f64,
    pub smooth_time: Duration,
    /// Enables acceleration.
    ///
    /// <http://gyrowiki.jibbsmart.com/blog:good-gyro-controls-part-1:the-gyro-is-a-mouse#toc7>
    pub slow_threshold: f64,
    pub slow_sens: Vector2<f64>,
    pub fast_threshold: f64,
    pub fast_sens: Vector2<f64>,
}

impl Default for GyroSettings {
    fn default() -> Self {
        Self {
            sens: vec2(1., 1.),
            invert: (false, false),
            space: GyroSpace::PlayerTurn,
            cutoff_speed: 0.,
            cutoff_recovery: 0.,
            smooth_threshold: 0.,
            smooth_time: Duration::from_millis(125),
            slow_sens: Vector2::zero(),
            slow_threshold: 0.,
            fast_sens: Vector2::zero(),
            fast_threshold: 0.,
        }
    }
}

impl GyroSettings {
    fn apply(&mut self, setting: GyroSetting) {
        match setting {
            GyroSetting::Sensitivity(x, y) => {
                self.sens = vec2(x, y.unwrap_or(x));
            }
            GyroSetting::MinSens(x, y) => {
                self.slow_sens = vec2(x, y.unwrap_or(x));
            }
            GyroSetting::MinThreshold(s) => self.slow_threshold = s,
            GyroSetting::MaxSens(x, y) => {
                self.fast_sens = vec2(x, y.unwrap_or(x));
            }
            GyroSetting::MaxThreshold(s) => self.fast_threshold = s,
            GyroSetting::Space(s) => self.space = s,
            GyroSetting::InvertX(b) => self.invert.0 = b == InvertMode::Inverted,
            GyroSetting::InvertY(b) => self.invert.1 = b == InvertMode::Inverted,
            GyroSetting::CutoffSpeed(s) => self.cutoff_speed = s,
            GyroSetting::CutoffRecovery(s) => self.cutoff_recovery = s,
            GyroSetting::SmoothThreshold(s) => self.smooth_threshold = s,
            GyroSetting::SmoothTime(s) => self.smooth_time = s,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MouseSettings {
    pub counter_os_speed: bool,
    pub real_world_calibration: f64,
    pub in_game_sens: f64,
}

impl Default for MouseSettings {
    fn default() -> Self {
        Self {
            counter_os_speed: false,
            real_world_calibration: 1.,
            in_game_sens: 1.,
        }
    }
}

impl MouseSettings {
    fn apply(&mut self, setting: MouseSetting) {
        match setting {
            MouseSetting::CounterOSSpeed(c) => {
                println!("Warning: counter os speed not supported");
                self.counter_os_speed = c;
            }
            MouseSetting::RealWorldCalibration(c) => self.real_world_calibration = c,
            MouseSetting::InGameSens(s) => self.in_game_sens = s,
        }
    }
}
