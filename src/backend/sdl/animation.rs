use std::{collections::HashMap, ops::Deref};

use cgmath::{Decomposed, Matrix4, One, Quaternion, Vector3, VectorSpace, Zero};
use gltf::animation::Interpolation;

use super::model::NodeId;

pub type AnimationId = usize;

pub struct AnimationStore {
    nodes_state: HashMap<NodeId, NodeState>,
    animations: HashMap<AnimationId, Animation>,
    names: HashMap<String, AnimationId>,
}

impl AnimationStore {
    pub fn get(&self, name: &str) -> Option<AnimationId> {
        self.names.get(name).cloned()
    }

    pub fn start_frame(&mut self) {
        self.nodes_state.clear();
    }

    pub fn play(&mut self, id: AnimationId, value: f32) {
        let animation = &self.animations[&id];
        for channel in &animation.channels {
            match &channel.component {
                Component::Translation(frames) => {
                    *self
                        .nodes_state
                        .entry(channel.target)
                        .or_default()
                        .translation
                        .get_or_insert(Vector3::zero()) += dbg!(Self::interpolate_translation(
                        frames,
                        value,
                        channel.interpolation
                    ));
                }
                Component::Rotation(frames) => {
                    let rotation = self
                        .nodes_state
                        .entry(channel.target)
                        .or_default()
                        .rotation
                        .get_or_insert(Quaternion::one());
                    *rotation = *rotation
                        * Self::interpolate_rotation(frames, value, channel.interpolation);
                }
            }
        }
    }

    pub fn state(&self, id: NodeId) -> Option<&NodeState> {
        self.nodes_state.get(&id)
    }

    fn interpolate_translation(
        frames: &[(f32, Vector3<f32>)],
        value: f32,
        interpolation: Interpolation,
    ) -> Vector3<f32> {
        match interpolation {
            Interpolation::Step => frames
                .iter()
                .find(|(t, _)| *t > value)
                .map(|x| x.1)
                .unwrap_or(frames[0].1),
            Interpolation::Linear => {
                if value <= frames[0].0 {
                    return frames[0].1;
                }
                dbg!(frames, value);
                let mut last = &frames[0];
                for frame @ (t, val) in frames.iter().skip(1) {
                    if value <= *t {
                        return last.1.lerp(*val, (value - last.0) / (t - last.0));
                    }
                    last = frame;
                }
                last.1
            }
            Interpolation::CubicSpline => todo!(),
        }
    }

    fn interpolate_rotation(
        frames: &[(f32, Quaternion<f32>)],
        value: f32,
        interpolation: Interpolation,
    ) -> Quaternion<f32> {
        match interpolation {
            Interpolation::Step => frames
                .iter()
                .find(|(t, _)| *t > value)
                .map(|x| x.1)
                .unwrap_or(frames[0].1),
            Interpolation::Linear => {
                if value <= frames[0].0 {
                    return frames[0].1;
                }
                let mut last = &frames[0];
                for frame @ (t, val) in frames.iter().skip(1) {
                    if value <= *t {
                        return last.1.slerp(*val, (value - last.0) / (t - last.0));
                    }
                    last = frame;
                }
                last.1
            }
            Interpolation::CubicSpline => todo!(),
        }
    }

    pub fn load(
        animations: gltf::iter::Animations,
        buffers: &[gltf::buffer::Data],
    ) -> AnimationStore {
        let mut names = HashMap::new();
        let animations = animations
            .map(|animation| {
                names.insert(
                    animation.name().expect("animation has a name").to_string(),
                    animation.index(),
                );
                let channels = animation
                    .channels()
                    .map(|channel| {
                        let target = channel.target().node().index();
                        let interpolation = channel.sampler().interpolation();
                        let reader =
                            channel.reader(|buffer| buffers.get(buffer.index()).map(Deref::deref));
                        let inputs = reader.read_inputs().unwrap();
                        let component = match reader.read_outputs().unwrap() {
                            gltf::animation::util::ReadOutputs::Translations(outputs) => {
                                Component::Translation(
                                    inputs.zip(outputs.map(From::from)).collect(),
                                )
                            }
                            gltf::animation::util::ReadOutputs::Rotations(outputs) => {
                                Component::Rotation(
                                    inputs
                                        .zip(
                                            outputs
                                                .into_f32()
                                                .map(|[x, y, z, w]| Quaternion::new(w, x, y, z)),
                                        )
                                        .collect(),
                                )
                            }
                            gltf::animation::util::ReadOutputs::Scales(_) => todo!(),
                            gltf::animation::util::ReadOutputs::MorphTargetWeights(_) => todo!(),
                        };
                        Channel {
                            target,
                            interpolation,
                            component,
                        }
                    })
                    .collect();
                (animation.index(), Animation { channels })
            })
            .collect();

        Self {
            nodes_state: HashMap::new(),
            animations,
            names,
        }
    }
}

#[derive(Debug)]
struct Animation {
    channels: Vec<Channel>,
}

#[derive(Debug)]
struct Channel {
    target: NodeId,
    interpolation: Interpolation,
    component: Component,
}

#[derive(Debug)]
enum Component {
    Translation(Vec<(f32, Vector3<f32>)>),
    Rotation(Vec<(f32, Quaternion<f32>)>),
}

#[derive(Default)]
pub struct NodeState {
    translation: Option<Vector3<f32>>,
    rotation: Option<Quaternion<f32>>,
}

impl NodeState {
    pub fn update(&self, rest: &Decomposed<Vector3<f32>, Quaternion<f32>>) -> Matrix4<f32> {
        Matrix4::from(Decomposed {
            scale: rest.scale,
            rot: self.rotation.unwrap_or(rest.rot),
            disp: self.translation.unwrap_or(rest.disp),
        })
    }
}
