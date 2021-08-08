use std::time::{Duration, Instant};

use crate::{
    calibration::BetterCalibration, config::settings::Settings, engine::Engine, mapping::Buttons,
    mouse::Mouse, opts::Run,
};

use anyhow::{bail, Result};
use enum_map::EnumMap;
use hid_gamepad::sys::GamepadDevice;
use hid_gamepad_types::{JoyKey, KeyStatus};
use joycon::{
    hidapi::HidApi,
    joycon_sys::{
        input::BatteryLevel,
        light::{self, PlayerLight},
    },
    JoyCon,
};

use super::Backend;

pub struct HidapiBackend {
    api: HidApi,
}

impl HidapiBackend {
    pub fn new() -> Result<Self> {
        Ok(Self {
            api: HidApi::new()?,
        })
    }
}

impl Backend for HidapiBackend {
    fn list_devices(&mut self) -> Result<()> {
        println!("Listing gamepads:");
        for device_info in self.api.device_list() {
            if hid_gamepad::open_gamepad(&self.api, device_info)?.is_some() {
                println!("Found one");
                return Ok(());
            }
        }
        bail!("No gamepad found");
    }

    fn run(&mut self, _opts: Run, settings: Settings, bindings: Buttons) -> Result<()> {
        loop {
            for device_info in self.api.device_list() {
                if let Some(mut gamepad) = hid_gamepad::open_gamepad(&self.api, device_info)? {
                    return hid_main(gamepad.as_mut(), settings, bindings);
                }
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            self.api.refresh_devices()?;
        }
    }
}

fn hid_main(gamepad: &mut dyn GamepadDevice, settings: Settings, bindings: Buttons) -> Result<()> {
    if let Some(joycon) = gamepad.as_any().downcast_mut::<JoyCon>() {
        dbg!(joycon.set_home_light(light::HomeLight::new(
            0x8,
            0x2,
            0x0,
            &[(0xf, 0xf, 0), (0x2, 0xf, 0)],
        ))?);

        let battery_level = joycon.tick()?.info.battery_level();

        joycon.set_player_light(light::PlayerLights::new(
            (battery_level >= BatteryLevel::Full).into(),
            (battery_level >= BatteryLevel::Medium).into(),
            (battery_level >= BatteryLevel::Low).into(),
            if battery_level >= BatteryLevel::Low {
                PlayerLight::On
            } else {
                PlayerLight::Blinking
            },
        ))?;
    }

    let mut calibrator = BetterCalibration::default();

    println!("calibrating");
    loop {
        let report = gamepad.recv()?;
        if calibrator.push(report.motion[0], Instant::now(), Duration::from_secs(1)) {
            break;
        }
    }
    println!("calibrating done");
    let mut engine = Engine::new(settings, bindings, calibrator.finish(), Mouse::new());

    let mut last_keys = EnumMap::default();
    loop {
        let report = gamepad.recv()?;
        let now = Instant::now();

        diff(engine.buttons(), now, &last_keys, &report.keys);
        last_keys = report.keys;

        engine.handle_left_stick(report.left_joystick, now);
        engine.handle_right_stick(report.right_joystick, now);

        engine.apply_actions(now);

        let dt = Duration::from_secs_f64(1. / report.frequency as f64 * report.motion.len() as f64);
        engine.handle_motion_frame(&report.motion, dt);
    }
}

macro_rules! diff {
    ($mapping:ident, $now:ident, $old:expr, $new:expr, $key:ident) => {
        match ($old[$key], $new[$key]) {
            (KeyStatus::Released, KeyStatus::Pressed) => $mapping.key_down($key, $now),
            (KeyStatus::Pressed, KeyStatus::Released) => $mapping.key_up($key, $now),
            _ => (),
        }
    };
}

fn diff(
    mapping: &mut Buttons,
    now: Instant,
    old: &EnumMap<JoyKey, KeyStatus>,
    new: &EnumMap<JoyKey, KeyStatus>,
) {
    use hid_gamepad_types::JoyKey::*;

    diff!(mapping, now, old, new, Up);
    diff!(mapping, now, old, new, Down);
    diff!(mapping, now, old, new, Left);
    diff!(mapping, now, old, new, Right);
    diff!(mapping, now, old, new, L);
    diff!(mapping, now, old, new, ZL);
    diff!(mapping, now, old, new, SL);
    diff!(mapping, now, old, new, SR);
    diff!(mapping, now, old, new, L3);
    diff!(mapping, now, old, new, R3);
    diff!(mapping, now, old, new, Minus);
    diff!(mapping, now, old, new, Plus);
    diff!(mapping, now, old, new, Capture);
    diff!(mapping, now, old, new, Home);
    diff!(mapping, now, old, new, W);
    diff!(mapping, now, old, new, N);
    diff!(mapping, now, old, new, S);
    diff!(mapping, now, old, new, E);
    diff!(mapping, now, old, new, R);
    diff!(mapping, now, old, new, ZR);
    diff!(mapping, now, old, new, SL);
    diff!(mapping, now, old, new, SR);
}
