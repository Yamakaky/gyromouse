use std::{
    collections::HashMap,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use cgmath::{vec2, vec3, Vector3};
use hid_gamepad_types::{Acceleration, JoyKey, Motion, RotationSpeed};
use sdl2::{
    self,
    controller::{Axis, Button, GameController},
    event::Event,
    keyboard::Keycode,
    sensor::SensorType,
    GameControllerSubsystem, Sdl,
};

use crate::{
    calibration::{BetterCalibration, Calibration},
    config::settings::Settings,
    engine::Engine,
    mapping::Buttons,
    mouse::Mouse,
};

use super::Backend;

pub struct SDLBackend {
    sdl: Sdl,
    game_controller_system: GameControllerSubsystem,
    mouse: Mouse,
}

impl SDLBackend {
    pub fn new() -> Result<Self> {
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_PS4_RUMBLE", "1");
        let sdl = sdl2::init().unwrap();
        let game_controller_system = sdl.game_controller().unwrap();
        Ok(Self {
            sdl,
            game_controller_system,
            mouse: Mouse::new(),
        })
    }
}

impl Backend for SDLBackend {
    fn list_devices(&mut self) -> anyhow::Result<()> {
        let num_joysticks = match self.game_controller_system.num_joysticks() {
            Ok(x) => x,
            Err(e) => bail!("{}", e),
        };
        if num_joysticks == 0 {
            println!("No controller detected");
        } else {
            println!("Detected controllers:");
            for i in 0..num_joysticks {
                let controller = self.game_controller_system.open(i)?;
                println!(" - {}", controller.name());
            }
        }
        Ok(())
    }

    fn run(
        &mut self,
        _opts: crate::opts::Run,
        settings: Settings,
        bindings: Buttons,
    ) -> anyhow::Result<()> {
        let mut event_pump = self.sdl.event_pump().unwrap();

        let mut controllers = HashMap::new();

        let mut last_tick = Instant::now();

        'running: loop {
            let now = Instant::now();
            let dt = now.duration_since(last_tick);

            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => break 'running,
                    Event::ControllerDeviceAdded { which, .. } => {
                        let mut controller = self.game_controller_system.open(which)?;

                        if controller.name() == "gyromouse"
                            || controller.name() == "Steam Virtual Gamepad"
                        {
                            continue;
                        }

                        println!("New controller: {}", controller.name());

                        // Ignore errors, handled later
                        let calibrator = if controller
                            .sensor_set_enabled(SensorType::Accelerometer, true)
                            .and(controller.sensor_set_enabled(SensorType::Gyroscope, true))
                            .is_ok()
                        {
                            println!(
                                "Starting calibration for {}, don't move the controller...",
                                controller.name()
                            );
                            Some(BetterCalibration::default())
                        } else {
                            let _ = controller.set_rumble(220, 440, 100);
                            None
                        };

                        let engine = Engine::new(
                            settings.clone(),
                            bindings.clone(),
                            Calibration::empty(),
                            self.mouse.clone(),
                        )?;
                        controllers.insert(
                            which,
                            ControllerState {
                                controller,
                                engine,
                                calibrator,
                            },
                        );
                    }
                    Event::ControllerDeviceRemoved { which, .. } => {
                        if let Some(controller) = controllers.remove(&which) {
                            println!("Controller disconnected: {}", controller.controller.name());
                        }
                    }
                    Event::ControllerButtonDown {
                        timestamp: _,
                        which,
                        button,
                    } => {
                        if let Some(controller) = controllers.get_mut(&which) {
                            controller
                                .engine
                                .buttons()
                                .key_down(sdl_to_sys(button), now);
                        }
                    }
                    Event::ControllerButtonUp {
                        timestamp: _,
                        which,
                        button,
                    } => {
                        if let Some(controller) = controllers.get_mut(&which) {
                            controller.engine.buttons().key_up(sdl_to_sys(button), now);
                        }
                    }
                    _ => {}
                }
            }

            for controller in controllers.values_mut() {
                let c = &mut controller.controller;
                let engine = &mut controller.engine;
                let left = vec2(c.axis(Axis::LeftX), c.axis(Axis::LeftY))
                    .cast::<f64>()
                    .unwrap()
                    / (i16::MAX as f64);
                let right = vec2(c.axis(Axis::RightX), c.axis(Axis::RightY))
                    .cast::<f64>()
                    .unwrap()
                    / (i16::MAX as f64);
                engine.handle_left_stick(left, now);
                engine.handle_right_stick(right, now);
                if c.sensor_enabled(SensorType::Accelerometer)
                    && c.sensor_enabled(SensorType::Gyroscope)
                {
                    let mut accel = [0.; 3];
                    c.sensor_get_data(SensorType::Accelerometer, &mut accel)?;
                    let acceleration =
                        Acceleration::from(Vector3::from(accel).cast::<f64>().unwrap() / 9.82);
                    let mut gyro = [0.; 3];
                    c.sensor_get_data(SensorType::Gyroscope, &mut gyro)?;
                    let rotation_speed = RotationSpeed::from(
                        vec3(gyro[0] as f64, gyro[1] as f64, gyro[2] as f64) / std::f64::consts::PI
                            * 180.,
                    );

                    if let Some(ref mut calibrator) = controller.calibrator {
                        let finished = calibrator.push(
                            Motion {
                                rotation_speed,
                                acceleration,
                            },
                            now,
                            Duration::from_secs(2),
                        );
                        if finished {
                            println!("Calibration finished for {}", c.name());
                            let _ = c.set_rumble(220, 440, 100);
                            engine.set_calibration(calibrator.finish());
                            controller.calibrator = None;
                        }
                    } else {
                        engine.apply_motion(rotation_speed, acceleration, dt);
                    }
                }
                engine.apply_actions(now)?;
            }

            last_tick = now;
            sleep(Duration::from_millis(1));
        }

        Ok(())
    }
}

struct ControllerState {
    controller: GameController,
    engine: Engine,
    calibrator: Option<BetterCalibration>,
}

fn sdl_to_sys(button: Button) -> JoyKey {
    match button {
        Button::A => JoyKey::S,
        Button::B => JoyKey::E,
        Button::X => JoyKey::W,
        Button::Y => JoyKey::N,
        Button::Back => JoyKey::Minus,
        Button::Guide => JoyKey::Home,
        Button::Start => JoyKey::Plus,
        Button::LeftStick => JoyKey::L3,
        Button::RightStick => JoyKey::R3,
        Button::LeftShoulder => JoyKey::L,
        Button::RightShoulder => JoyKey::R,
        Button::DPadUp => JoyKey::Up,
        Button::DPadDown => JoyKey::Down,
        Button::DPadLeft => JoyKey::Left,
        Button::DPadRight => JoyKey::Right,
    }
}
