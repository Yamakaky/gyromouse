#[cfg(feature = "gui")]
mod gui;

use std::{
    collections::HashMap,
    thread::sleep,
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use cgmath::{vec2, Vector3};
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

use self::gui::Gui;

use super::Backend;

pub struct SDLBackend {
    sdl: Sdl,
    game_controller_system: GameControllerSubsystem,
    mouse: Mouse,
    #[cfg(feature = "gui")]
    gui: Gui,
}

impl SDLBackend {
    pub fn new() -> Result<Self> {
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_PS4_RUMBLE", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_PS5_RUMBLE", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_JOY_CONS", "1");
        sdl2::hint::set("SDL_JOYSTICK_HIDAPI_SWITCH_HOME_LED", "0");
        sdl2::hint::set("SDL_GAMECONTROLLER_USE_BUTTON_LABELS", "0");

        // Better Windows support
        sdl2::hint::set("SDL_HINT_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
        sdl2::hint::set("SDL_HINT_JOYSTICK_THREAD", "1");

        let sdl = sdl2::init().expect("can't initialize SDL");
        let game_controller_system = sdl
            .game_controller()
            .expect("can't initialize SDL game controller subsystem");

        #[cfg(feature = "gui")]
        let gui = Gui::new(&sdl);

        Ok(Self {
            sdl,
            game_controller_system,
            mouse: Mouse::new(),
            #[cfg(feature = "gui")]
            gui,
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
        if self
            .game_controller_system
            .num_joysticks()
            .expect("can't enumerate the joysticks")
            == 0
        {
            println!("Waiting for a game controller to connect...");
        }
        let mut event_pump = self
            .sdl
            .event_pump()
            .expect("can't create the SDL event pump");

        let mut controllers: HashMap<u32, ControllerState> = HashMap::new();

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

                        if controllers
                            .values()
                            .any(|c| c.controller.name() == controller.name())
                        {
                            continue;
                        }

                        if controller.name() == "Steam Virtual Gamepad" {
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
                            controller.instance_id(),
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
                    _ => {
                        #[cfg(feature = "gui")]
                        self.gui.event(event);
                    }
                }
            }

            for controller in controllers.values_mut() {
                let c = &mut controller.controller;
                let engine = &mut controller.engine;
                let mut left = vec2(c.axis(Axis::LeftX), c.axis(Axis::LeftY))
                    .cast::<f64>()
                    .expect("can't cast i16 to f64")
                    / (i16::MAX as f64);
                let mut right = vec2(c.axis(Axis::RightX), c.axis(Axis::RightY))
                    .cast::<f64>()
                    .expect("can't cast i16 to f64")
                    / (i16::MAX as f64);

                // In SDL, -..+ y is top..bottom
                left.y = -left.y;
                right.y = -right.y;

                engine.handle_left_stick(left, now, dt);
                engine.handle_right_stick(right, now, dt);

                if c.sensor_enabled(SensorType::Accelerometer)
                    && c.sensor_enabled(SensorType::Gyroscope)
                {
                    let mut accel = [0.; 3];
                    c.sensor_get_data(SensorType::Accelerometer, &mut accel)?;
                    let acceleration = Acceleration::from(
                        Vector3::from(accel)
                            .cast::<f64>()
                            .expect("can't cast f32 to f64")
                            / 9.82,
                    );
                    let mut gyro = [0.; 3];
                    c.sensor_get_data(SensorType::Gyroscope, &mut gyro)?;
                    let rotation_speed = RotationSpeed::from(
                        Vector3::from(gyro)
                            .cast::<f64>()
                            .expect("can't cast f32 to f64")
                            / std::f64::consts::PI
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
                        engine.apply_motion(rotation_speed, acceleration, now, dt);
                    }
                }
                engine.apply_actions(now)?;
            }

            #[cfg(feature = "gui")]
            if self.gui.tick(dt) {
                break 'running;
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
