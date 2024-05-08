use std::time::Duration;

use cgmath::{
    num_traits::{NumCast, ToPrimitive},
    Deg,
};

use crate::{
    mapping::{ExtAction, MapKey},
    ClickType,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ActionModifier {
    Toggle,
    Instant,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EventModifier {
    Tap,
    Hold,
    Start,
    Release,
    Turbo,
}

#[derive(Debug, Copy, Clone)]
pub struct JSMAction {
    pub action_mod: Option<ActionModifier>,
    pub event_mod: Option<EventModifier>,
    pub action: ActionType,
}

#[derive(Debug, Copy, Clone)]
pub enum ActionType {
    Key(enigo::Key),
    Mouse(enigo::Button),
    Special(SpecialKey),
    #[cfg(feature = "vgamepad")]
    Gamepad(virtual_gamepad::Key),
}

impl From<(ActionType, ClickType)> for ExtAction {
    fn from((a, b): (ActionType, ClickType)) -> Self {
        match a {
            ActionType::Key(k) => ExtAction::KeyPress(k, b),
            ActionType::Mouse(k) => ExtAction::MousePress(k, b),
            ActionType::Special(SpecialKey::GyroOn) => ExtAction::GyroOn(b),
            ActionType::Special(SpecialKey::GyroOff) => ExtAction::GyroOff(b),
            ActionType::Special(s) => {
                // TODO: Handle every special key.
                eprintln!("Warning: special key {:?} is unimplemented", s);
                ExtAction::None
            }
            #[cfg(feature = "vgamepad")]
            ActionType::Gamepad(k) => ExtAction::GamepadKeyPress(k, b),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Key {
    Simple(MapKey),
    Simul(MapKey, MapKey),
    Chorded(MapKey, MapKey),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SpecialKey {
    None,
    GyroOn,
    GyroOff,
    GyroInvertX(bool),
    GyroInvertY(bool),
    GyroTrackBall(bool),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TriggerMode {
    NoFull,
    NoSkip,
    NoSkipExclusive,
    MustSkip,
    MaySkip,
    MustSkipR,
    MaySkipR,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StickMode {
    Aim,
    Flick,
    FlickOnly,
    RotateOnly,
    MouseRing,
    MouseArea,
    NoMouse,
    ScrollWheel,
}

#[derive(Debug, Copy, Clone)]
pub enum StickSetting {
    Deadzone(f64),
    FullZone(f64),
    Aim(AimStickSetting),
    Flick(FlickStickSetting),
    Scroll(ScrollStickSetting),
    Area(AreaStickSetting),
    Motion(MotionStickSetting),
}

#[derive(Debug, Copy, Clone)]
pub enum AimStickSetting {
    Sens(f64),
    Power(f64),
    LeftAxis(InvertMode, Option<InvertMode>),
    RightAxis(InvertMode, Option<InvertMode>),
    AccelerationRate(f64),
    AccelerationCap(f64),
}

#[derive(Debug, Copy, Clone)]
pub enum FlickStickSetting {
    FlickTime(Duration),
    Exponent(f64),
    ForwardDeadzoneArc(Deg<f64>),
}

#[derive(Debug, Copy, Clone)]
pub enum ScrollStickSetting {
    Sens(Deg<f64>),
}

#[derive(Debug, Copy, Clone)]
pub enum AreaStickSetting {
    ScreenResolutionX(u32),
    ScreenResolutionY(u32),
    Radius(u32),
}

#[derive(Debug, Copy, Clone)]
pub enum MotionStickSetting {
    StickMode(StickMode),
    RingMode(RingMode),
    Deadzone(Deg<f64>),
    Fullzone(Deg<f64>),
    Axis(InvertMode, Option<InvertMode>),
}

#[derive(Debug, Copy, Clone)]
pub enum GyroSetting {
    Sensitivity(f64, Option<f64>),
    MinSens(f64, Option<f64>),
    MinThreshold(f64),
    MaxSens(f64, Option<f64>),
    MaxThreshold(f64),
    Space(GyroSpace),
    InvertX(InvertMode),
    InvertY(InvertMode),
    CutoffSpeed(f64),
    CutoffRecovery(f64),
    SmoothThreshold(f64),
    SmoothTime(Duration),
}

#[derive(Debug, Copy, Clone)]
pub enum MouseSetting {
    CounterOSSpeed(bool),
    RealWorldCalibration(f64),
    InGameSens(f64),
}

#[derive(Debug, Copy, Clone)]
pub enum GyroSpace {
    Local,
    WorldTurn,
    WorldLean,
    PlayerTurn,
    PlayerLean,
}

#[derive(Debug, Copy, Clone)]
pub enum Setting {
    Gyro(GyroSetting),
    TriggerThreshold(f64),
    ZLMode(TriggerMode),
    ZRMode(TriggerMode),
    LeftStickMode(StickMode),
    RightStickMode(StickMode),
    LeftRingMode(RingMode),
    RightRingMode(RingMode),
    Stick(StickSetting),
    Mouse(MouseSetting),
}

#[derive(Debug, Clone)]
pub enum Cmd {
    Map(Key, Vec<JSMAction>),
    Special(SpecialKey),
    Setting(Setting),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingMode {
    Inner,
    Outer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvertMode {
    Normal,
    Inverted,
}

impl ToPrimitive for InvertMode {
    fn to_i64(&self) -> std::option::Option<i64> {
        Some(match self {
            InvertMode::Normal => 1,
            InvertMode::Inverted => -1,
        })
    }

    fn to_u64(&self) -> Option<u64> {
        None
    }
}

impl NumCast for InvertMode {
    fn from<T: ToPrimitive>(_: T) -> Option<Self> {
        None
    }
}
