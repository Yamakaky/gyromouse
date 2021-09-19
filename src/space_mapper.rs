use std::{f64::consts::PI, time::Duration};

use cgmath::{vec2, vec3, InnerSpace, Quaternion, Rotation, Vector2, Vector3, VectorSpace, Zero};
use hid_gamepad_types::{Motion, RotationSpeed};

pub fn map_input(
    motion: &Motion,
    dt: Duration,
    sensor_fusion: &mut dyn SensorFusion,
    mapper: &mut dyn SpaceMapper,
) -> Vector2<f64> {
    let up_vector = sensor_fusion.compute_up_vector(motion, dt);
    mapper.map(motion.rotation_speed, up_vector)
}
pub trait SensorFusion {
    fn up_vector(&self) -> Vector3<f64>;
    fn compute_up_vector(&mut self, motion: &Motion, dt: Duration) -> Vector3<f64>;
}

/// Convert local space motion to 2D mouse-like motion.
pub trait SpaceMapper {
    fn map(&self, rot_speed: RotationSpeed, grav: Vector3<f64>) -> Vector2<f64>;
}

#[derive(Debug, Copy, Clone)]
pub struct SimpleFusion {
    up_vector: Vector3<f64>,
    correction_factor: f64,
}

impl SimpleFusion {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            up_vector: vec3(0., 1., 0.),
            correction_factor: 0.02,
        }
    }
}

impl SensorFusion for SimpleFusion {
    fn up_vector(&self) -> Vector3<f64> {
        self.up_vector
    }
    fn compute_up_vector(&mut self, motion: &Motion, dt: Duration) -> Vector3<f64> {
        let rotation = Quaternion::from(motion.rotation_speed * dt).invert();
        self.up_vector = rotation.rotate_vector(self.up_vector);
        // TODO: Make the correction rate depend on dt instead of fixed per tick.
        self.up_vector +=
            (motion.acceleration.as_vec().normalize() - self.up_vector) * self.correction_factor;
        self.up_vector = if self.up_vector.magnitude2() > 0. {
            self.up_vector.normalize()
        } else {
            Vector3::zero()
        };
        self.up_vector
    }
}

#[derive(Debug, Copy, Clone)]
pub struct AdaptativeFusion {
    shakiness: f64,
    smooth_accel: Vector3<f64>,
    up_vector: Vector3<f64>,
}

impl AdaptativeFusion {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            shakiness: 0.,
            // Way off when starting but should converge rapidly
            smooth_accel: Vector3::zero(),
            up_vector: Vector3::zero(),
        }
    }
}

impl SensorFusion for AdaptativeFusion {
    fn up_vector(&self) -> Vector3<f64> {
        self.up_vector
    }

    // TODO: check http://gyrowiki.jibbsmart.com/blog:finding-gravity-with-sensor-fusion
    // TODO: check normalize() with magnitude 0.
    fn compute_up_vector(&mut self, motion: &Motion, dt: Duration) -> Vector3<f64> {
        // settings
        let smoothing_half_time = 0.25;
        let shakiness_min_threshold = 0.4;
        let shakiness_max_threshold = 0.01;
        let still_rate = 1.;
        let shaky_rate = 0.1;
        let correction_gyro_factor = 0.1;
        let correction_gyro_min_threshold = 0.05;
        let correction_gyro_max_threshold = 0.25;
        let correction_min_speed = 0.01;

        let rot = motion.rotation_speed * dt;
        let rot_vec = vec3(rot.x.0, rot.y.0, rot.z.0);
        let acc = motion.acceleration;

        let invert_rotation = Quaternion::from(motion.rotation_speed * dt).invert();

        self.up_vector = invert_rotation.rotate_vector(self.up_vector);
        self.smooth_accel = invert_rotation.rotate_vector(self.smooth_accel);
        let smooth_interpolator = if smoothing_half_time <= 0. {
            0.
        } else {
            -dt.as_secs_f64() / smoothing_half_time
        };
        self.shakiness = (self.shakiness * smooth_interpolator)
            .max((acc.as_vec() - self.smooth_accel).magnitude());
        self.smooth_accel = acc.as_vec().lerp(self.smooth_accel, smooth_interpolator);

        let up_delta = acc.as_vec() - self.up_vector;
        let up_direction = up_delta.normalize();
        let shake_factor = normalize(
            self.shakiness,
            shakiness_min_threshold,
            shakiness_max_threshold,
        );
        let mut correction_rate = still_rate + (shaky_rate - still_rate) * shake_factor;

        let angle_rate = rot_vec.magnitude() * PI / 180.;
        let correction_limit = angle_rate * self.up_vector.magnitude() * correction_gyro_factor;
        if correction_rate > correction_limit {
            let close_enough_factor = normalize(
                up_delta.magnitude(),
                correction_gyro_min_threshold,
                correction_gyro_max_threshold,
            );
            correction_rate += (correction_limit - correction_rate) * close_enough_factor;
        }

        correction_rate = correction_rate.max(correction_min_speed);

        let correction = up_direction.normalize_to(correction_rate * dt.as_secs_f64());
        self.up_vector += if correction.magnitude2() < up_delta.magnitude2() {
            correction
        } else {
            up_delta
        };
        self.up_vector
    }
}

// Normalize value as (0..1) between min and max.
// Handles edge cases where min >= max
fn normalize(val: f64, min: f64, max: f64) -> f64 {
    if min >= max {
        (val > max) as u8 as f64
    } else {
        (val - min) / (max - min)
    }
    .clamp(0., 1.)
}

#[derive(Default)]
pub struct LocalSpace;

impl SpaceMapper for LocalSpace {
    fn map(&self, rot_speed: RotationSpeed, _up_vector: Vector3<f64>) -> Vector2<f64> {
        vec2(-rot_speed.y, rot_speed.x)
    }
}

#[derive(Default)]
pub struct WorldSpace;

impl SpaceMapper for WorldSpace {
    fn map(&self, rot_speed: RotationSpeed, up_vector: Vector3<f64>) -> Vector2<f64> {
        let flatness = up_vector.y.abs();
        let upness = up_vector.z.abs();
        let side_reduction = (flatness.max(upness) - 0.125).clamp(0., 1.);

        let yaw_diff = -rot_speed.as_vec().dot(up_vector);

        let pitch = vec3(1., 0., 0.) - up_vector * up_vector.x;
        let pitch_diff = if pitch.magnitude2() > 0. {
            side_reduction * rot_speed.as_vec().dot(pitch.normalize())
        } else {
            0.
        };
        vec2(yaw_diff, pitch_diff)
    }
}

pub struct PlayerSpace {
    yaw_relax_factor: f64,
}

impl Default for PlayerSpace {
    fn default() -> Self {
        Self {
            yaw_relax_factor: 1.41,
        }
    }
}

impl SpaceMapper for PlayerSpace {
    fn map(&self, rot_speed: RotationSpeed, up_vector: Vector3<f64>) -> Vector2<f64> {
        let world_yaw = rot_speed.y * up_vector.y + rot_speed.z * up_vector.z;
        vec2(
            -world_yaw.signum()
                * (world_yaw.abs() * self.yaw_relax_factor)
                    .min(vec2(rot_speed.y, rot_speed.z).magnitude()),
            rot_speed.x,
        )
    }
}
