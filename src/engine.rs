use std::{
    ops::DerefMut,
    time::{Duration, Instant},
};

use cgmath::{Vector2, Zero};
use enigo::{KeyboardControllable, MouseControllable};
use hid_gamepad_types::{Acceleration, Motion, RotationSpeed};
use virtual_gamepad::{Backend, VirtualGamepad};

use crate::{
    calibration::Calibration,
    config::{settings::Settings, types::GyroSpace},
    gyromouse::GyroMouse,
    joystick::Stick,
    mapping::{Buttons, ExtAction},
    mouse::Mouse,
    space_mapper::{
        self, LocalSpace, PlayerSpace, SensorFusion, SimpleFusion, SpaceMapper, WorldSpace,
    },
    ClickType,
};

pub struct Engine {
    settings: Settings,
    left_stick: Box<dyn Stick>,
    right_stick: Box<dyn Stick>,
    buttons: Buttons,
    mouse: Mouse,
    gyro: Gyro,
    gamepad: VirtualGamepad,
}

impl Engine {
    pub fn new(
        settings: Settings,
        buttons: Buttons,
        calibration: Calibration,
        mouse: Mouse,
    ) -> anyhow::Result<Self> {
        Ok(Engine {
            left_stick: settings.new_left_stick(),
            right_stick: settings.new_right_stick(),
            buttons,
            mouse,
            gyro: Gyro::new(&settings, calibration),
            settings,
            gamepad: VirtualGamepad::new("gyromouse")?,
        })
    }

    pub fn buttons(&mut self) -> &mut Buttons {
        &mut self.buttons
    }

    pub fn handle_left_stick(&mut self, stick: Vector2<f64>, now: Instant) {
        self.left_stick.handle(
            stick,
            &self.settings,
            &mut self.buttons,
            &mut self.mouse,
            now,
        );
    }

    pub fn handle_right_stick(&mut self, stick: Vector2<f64>, now: Instant) {
        self.right_stick.handle(
            stick,
            &self.settings,
            &mut self.buttons,
            &mut self.mouse,
            now,
        );
    }

    pub fn apply_actions(&mut self, now: Instant) -> anyhow::Result<()> {
        let mut gamepad_pressed = false;
        for action in self.buttons.tick(now).drain(..) {
            match action {
                ExtAction::GyroOn(ClickType::Press) | ExtAction::GyroOff(ClickType::Release) => {
                    self.gyro.enabled = true
                }
                ExtAction::GyroOn(ClickType::Release) | ExtAction::GyroOff(ClickType::Press) => {
                    self.gyro.enabled = false
                }
                ExtAction::GyroOn(_) | ExtAction::GyroOff(_) => unimplemented!(),
                ExtAction::KeyPress(c, ClickType::Click) => self.mouse.enigo().key_click(c),
                ExtAction::KeyPress(c, ClickType::Press) => self.mouse.enigo().key_down(c),
                ExtAction::KeyPress(c, ClickType::Release) => self.mouse.enigo().key_up(c),
                ExtAction::KeyPress(_, ClickType::Toggle) => unimplemented!(),
                ExtAction::MousePress(c, ClickType::Click) => self.mouse.enigo().mouse_click(c),
                ExtAction::MousePress(c, ClickType::Press) => self.mouse.enigo().mouse_down(c),
                ExtAction::MousePress(c, ClickType::Release) => self.mouse.enigo().mouse_up(c),
                ExtAction::MousePress(_, ClickType::Toggle) => unimplemented!(),
                ExtAction::GamepadKeyPress(key, ClickType::Press) => {
                    self.gamepad.key(key, true)?;
                    gamepad_pressed = true;
                }
                ExtAction::GamepadKeyPress(key, ClickType::Release) => {
                    self.gamepad.key(key, false)?;
                    gamepad_pressed = true;
                }
                ExtAction::GamepadKeyPress(_, _) => todo!(),
            }
        }
        if gamepad_pressed {
            self.gamepad.push()?;
        }
        Ok(())
    }

    pub fn apply_motion(
        &mut self,
        rotation_speed: RotationSpeed,
        acceleration: Acceleration,
        dt: Duration,
    ) {
        self.handle_motion_frame(
            &[Motion {
                rotation_speed,
                acceleration,
            }],
            dt,
        )
    }

    pub fn handle_motion_frame(&mut self, motions: &[Motion], dt: Duration) {
        self.gyro
            .handle_frame(&self.settings, motions, &mut self.mouse, dt)
    }

    pub fn set_calibration(&mut self, calibration: Calibration) {
        self.gyro.calibration = calibration;
    }
}

pub struct Gyro {
    enabled: bool,
    calibration: Calibration,
    sensor_fusion: Box<dyn SensorFusion>,
    space_mapper: Box<dyn SpaceMapper>,
    gyromouse: GyroMouse,
}

impl Gyro {
    pub fn new(settings: &Settings, calibration: Calibration) -> Gyro {
        Gyro {
            enabled: true,
            calibration,
            sensor_fusion: Box::new(SimpleFusion::new()),
            space_mapper: match settings.gyro.space {
                GyroSpace::Local => Box::new(LocalSpace::default()),
                GyroSpace::WorldTurn => Box::new(WorldSpace::default()),
                GyroSpace::WorldLean => todo!(),
                GyroSpace::PlayerTurn => Box::new(PlayerSpace::default()),
                GyroSpace::PlayerLean => todo!(),
            },
            gyromouse: GyroMouse::default(),
        }
    }

    pub fn handle_frame(
        &mut self,
        settings: &Settings,
        motions: &[Motion],
        mouse: &mut Mouse,
        dt: Duration,
    ) {
        const SMOOTH_RATE: bool = true;
        let mut delta_position = Vector2::zero();
        let dt = dt / motions.len() as u32;
        for (i, frame) in motions.iter().cloned().enumerate() {
            let frame = self.calibration.calibrate(frame);
            let delta = space_mapper::map_input(
                &frame,
                dt,
                self.sensor_fusion.deref_mut(),
                self.space_mapper.deref_mut(),
            );
            let offset = self.gyromouse.process(&settings.gyro, delta, dt);
            delta_position += offset;
            if self.enabled && !SMOOTH_RATE {
                if i > 0 {
                    std::thread::sleep(dt);
                }
                mouse.mouse_move_relative(&settings.mouse, offset);
            }
        }
        if self.enabled && SMOOTH_RATE {
            mouse.mouse_move_relative(&settings.mouse, delta_position);
        }
    }
}
