use enigo::{Key, MouseButton};
use enum_map::{Enum, EnumMap};
use hid_gamepad_types::JoyKey;
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    mem::MaybeUninit,
    time::Duration,
};
use std::{convert::TryInto, time::Instant};

use crate::ClickType;

#[derive(Debug, Copy, Clone)]
pub enum Action {
    Layer(u8, bool),
    Ext(ExtAction),
}

#[derive(Debug, Copy, Clone)]
pub enum ExtAction {
    None,
    KeyPress(Key, ClickType),
    MousePress(MouseButton, ClickType),
    #[cfg(feature = "vgamepad")]
    GamepadKeyPress(virtual_gamepad::Key, ClickType),
    GyroOn(ClickType),
    GyroOff(ClickType),
}

impl Display for ExtAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExtAction::None => f.write_str("none"),
            ExtAction::KeyPress(k, t) => write!(f, "{:?} {:?}", t, k),
            ExtAction::MousePress(m, t) => write!(f, "{:?} {:?}", t, m),
            ExtAction::GamepadKeyPress(k, t) => write!(f, "{:?} {:?}", t, k),
            ExtAction::GyroOn(t) => write!(f, "{:?} gyro on", t),
            ExtAction::GyroOff(t) => write!(f, "{:?} gyro off", t),
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum KeyStatus {
    Down,
    Up,
    Hold,
    DoubleUp,
    DoubleDown,
}

impl KeyStatus {
    pub fn is_down(self) -> bool {
        match self {
            KeyStatus::Down | KeyStatus::DoubleDown | KeyStatus::Hold => true,
            KeyStatus::Up | KeyStatus::DoubleUp => false,
        }
    }

    pub fn is_up(self) -> bool {
        !self.is_down()
    }
}

impl Default for KeyStatus {
    fn default() -> Self {
        KeyStatus::Up
    }
}

#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub on_down: Vec<Action>,
    pub on_up: Vec<Action>,

    pub on_click: Vec<Action>,
    pub on_double_click: Vec<Action>,
    pub on_hold_down: Vec<Action>,
    pub on_hold_up: Vec<Action>,
}

impl Layer {
    fn is_good(&self) -> bool {
        self.on_down.len()
            + self.on_up.len()
            + self.on_click.len()
            + self.on_hold_down.len()
            + self.on_hold_up.len()
            + self.on_double_click.len()
            > 0
    }

    fn is_simple_click(&self) -> bool {
        self.on_hold_down.is_empty()
            && self.on_hold_up.is_empty()
            && self.on_double_click.is_empty()
    }
}

#[derive(Debug, Clone)]
struct KeyState {
    status: KeyStatus,
    last_update: Instant,
}

impl Default for KeyState {
    fn default() -> Self {
        KeyState {
            status: KeyStatus::Up,
            last_update: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Enum)]
pub enum VirtualKey {
    LUp,
    LDown,
    LLeft,
    LRight,
    LRing,
    RUp,
    RDown,
    RLeft,
    RRight,
    RRing,
    MUp,
    MDown,
    MLeft,
    MRight,
    MRing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MapKey {
    Physical(JoyKey),
    Virtual(VirtualKey),
}

impl MapKey {
    pub fn to_layer(self) -> u8 {
        <Self as Enum<()>>::to_usize(self)
            .try_into()
            .expect("error converting MapKey to u8")
    }
}

const JOYKEY_SIZE: usize = <JoyKey as Enum<()>>::POSSIBLE_VALUES;
const VIRTKEY_SIZE: usize = <VirtualKey as Enum<()>>::POSSIBLE_VALUES;
const MAP_KEY_SIZE: usize = JOYKEY_SIZE + VIRTKEY_SIZE;

impl<V: Default + Sized> Enum<V> for MapKey {
    type Array = [V; MAP_KEY_SIZE];

    const POSSIBLE_VALUES: usize = MAP_KEY_SIZE;

    fn slice(array: &Self::Array) -> &[V] {
        array
    }

    fn slice_mut(array: &mut Self::Array) -> &mut [V] {
        array
    }

    fn from_usize(value: usize) -> Self {
        if value < JOYKEY_SIZE {
            <JoyKey as Enum<()>>::from_usize(value).into()
        } else if value < MAP_KEY_SIZE {
            <VirtualKey as Enum<()>>::from_usize(value - JOYKEY_SIZE).into()
        } else {
            unreachable!("MapKey value cannot be > MAP_KEY_SIZE");
        }
    }

    fn to_usize(self) -> usize {
        match self {
            MapKey::Physical(p) => <JoyKey as Enum<()>>::to_usize(p),
            MapKey::Virtual(v) => <VirtualKey as Enum<()>>::to_usize(v) + JOYKEY_SIZE,
        }
    }

    fn from_function<F: FnMut(Self) -> V>(mut f: F) -> Self::Array {
        unsafe {
            let mut out = MaybeUninit::<[MaybeUninit<V>; MAP_KEY_SIZE]>::uninit().assume_init();
            for (i, out) in out.iter_mut().enumerate() {
                *out = MaybeUninit::new(f(<Self as Enum<V>>::from_usize(i)));
            }
            out.as_ptr().cast::<Self::Array>().read()
        }
    }
}

impl From<JoyKey> for MapKey {
    fn from(k: JoyKey) -> Self {
        MapKey::Physical(k)
    }
}

impl From<VirtualKey> for MapKey {
    fn from(k: VirtualKey) -> Self {
        MapKey::Virtual(k)
    }
}

#[derive(Debug, Clone)]
pub struct Buttons {
    bindings: EnumMap<MapKey, HashMap<u8, Layer>>,
    state: EnumMap<MapKey, KeyState>,
    current_layers: Vec<u8>,

    ext_actions: Vec<ExtAction>,

    pub hold_delay: Duration,
    pub double_click_interval: Duration,
}

impl Buttons {
    pub fn new() -> Self {
        Buttons {
            bindings: EnumMap::new(),
            state: EnumMap::new(),
            current_layers: vec![0],
            ext_actions: Vec::new(),
            hold_delay: Duration::from_millis(100),
            double_click_interval: Duration::from_millis(200),
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn get(&mut self, key: impl Into<MapKey>, layer: u8) -> &mut Layer {
        self.bindings[key.into()].entry(layer).or_default()
    }

    pub fn tick(&mut self, now: Instant) -> impl Iterator<Item = ExtAction> + '_ {
        for key in (0..<MapKey as Enum<KeyStatus>>::POSSIBLE_VALUES)
            .map(<MapKey as Enum<KeyStatus>>::from_usize)
        {
            let binding = self.find_binding(key);
            match self.state[key].status {
                KeyStatus::Down => {
                    if binding.on_hold_down.len() > 0 {
                        if now.duration_since(self.state[key].last_update) >= self.hold_delay {
                            Self::actions(
                                &binding.on_hold_down,
                                &mut self.current_layers,
                                &mut self.ext_actions,
                            );
                            self.state[key].status = KeyStatus::Hold;
                        }
                    }
                }
                KeyStatus::DoubleUp => {
                    if now.duration_since(self.state[key].last_update) >= self.double_click_interval
                    {
                        Self::maybe_clicks(
                            &binding,
                            &mut self.current_layers,
                            &mut self.ext_actions,
                        );
                        self.state[key].status = KeyStatus::Up;
                    }
                }
                _ => (),
            }
        }
        self.ext_actions.drain(..)
    }

    pub fn key_down(&mut self, key: impl Into<MapKey>, now: Instant) {
        let key = key.into();
        if self.state[key].status.is_down() {
            return;
        }
        let binding = self.find_binding(key);
        Self::actions(
            &binding.on_down,
            &mut self.current_layers,
            &mut self.ext_actions,
        );
        if binding.is_simple_click() {
            Self::maybe_clicks(&binding, &mut self.current_layers, &mut self.ext_actions);
        }
        self.state[key].status = match self.state[key].status {
            KeyStatus::DoubleUp
                if now.duration_since(self.state[key].last_update) < self.double_click_interval =>
            {
                KeyStatus::DoubleDown
            }
            KeyStatus::DoubleUp => {
                Self::maybe_clicks(&binding, &mut self.current_layers, &mut self.ext_actions);
                KeyStatus::Down
            }
            KeyStatus::Up => KeyStatus::Down,
            _ => unreachable!(),
        };
        self.state[key].last_update = now;
    }

    pub fn key_up(&mut self, key: impl Into<MapKey>, now: Instant) {
        let key = key.into();
        if self.state[key].status.is_up() {
            return;
        }
        let binding = self.find_binding(key);
        Self::actions(
            &binding.on_up,
            &mut self.current_layers,
            &mut self.ext_actions,
        );
        let mut new_status = KeyStatus::Up;
        if !binding.is_simple_click() {
            if binding.on_hold_up.is_empty()
                || now.duration_since(self.state[key].last_update) < self.hold_delay
            {
                if binding.on_double_click.len() > 0 {
                    match self.state[key].status {
                        KeyStatus::DoubleDown => {
                            Self::actions(
                                &binding.on_double_click,
                                &mut self.current_layers,
                                &mut self.ext_actions,
                            );
                            new_status = KeyStatus::Up;
                        }
                        KeyStatus::Down => {
                            new_status = KeyStatus::DoubleUp;
                        }
                        _ => unreachable!(),
                    }
                } else {
                    Self::maybe_clicks(&binding, &mut self.current_layers, &mut self.ext_actions);
                }
            } else if binding.on_hold_up.len() > 0 {
                Self::actions(
                    &binding.on_hold_up,
                    &mut self.current_layers,
                    &mut self.ext_actions,
                );
            }
        }
        self.state[key].status = new_status;
        self.state[key].last_update = now;
    }

    pub fn key(&mut self, key: impl Into<MapKey>, pressed: bool, now: Instant) {
        let key = key.into();
        if pressed {
            self.key_down(key, now);
        } else {
            self.key_up(key, now);
        }
    }

    fn maybe_clicks(
        binding: &Layer,
        current_layers: &mut Vec<u8>,
        ext_actions: &mut Vec<ExtAction>,
    ) {
        Self::actions(&binding.on_click, current_layers, ext_actions);
    }

    fn find_binding(&self, key: MapKey) -> Layer {
        let layers = &self.bindings[key];
        for i in self.current_layers.iter().rev() {
            if let Some(layer) = layers.get(i) {
                if layer.is_good() {
                    // TODO: Fix ugly clone
                    return layer.clone();
                }
            }
        }
        Layer::default()
    }

    fn actions(actions: &[Action], current_layers: &mut Vec<u8>, ext_actions: &mut Vec<ExtAction>) {
        for action in actions {
            match *action {
                Action::Layer(l, true) => {
                    if current_layers.contains(&l) {
                        current_layers.retain(|x| *x != l);
                    }
                    current_layers.push(l);
                }
                Action::Layer(l, false) => {
                    current_layers.retain(|x| *x != l);
                }
                Action::Ext(action) => ext_actions.push(action),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_simple() {
        let mapping = {
            let mut mapping = Buttons::new();
            mapping
                .get(JoyKey::S, 0)
                .on_click
                .push(Action::Ext(ExtAction::KeyPress(Key::Alt, ClickType::Press)));
            mapping
                .get(JoyKey::S, 0)
                .on_double_click
                .push(Action::Ext(ExtAction::KeyPress(
                    Key::Space,
                    ClickType::Press,
                )));
            mapping
        };

        let t0 = Instant::now();
        let ms1 = Duration::from_millis(1);
        let taclick = t0 + ms1;
        let tbhold = t0 + mapping.hold_delay - ms1;
        let tahold = t0 + mapping.hold_delay + 2 * ms1;
        let tbdoub = t0 + mapping.double_click_interval - ms1;
        let tadoub = t0 + mapping.double_click_interval + 2 * ms1;

        {
            let mut mapping = mapping.clone();
            mapping.key_down(JoyKey::S, t0);
            mapping.key_up(JoyKey::S, t0);
            assert!(mapping.tick(t0).next().is_none());

            {
                let mut mapping = mapping.clone();
                let mut a = mapping.tick(tadoub);
                assert!(matches!(
                    dbg!(a.next()),
                    Some(ExtAction::KeyPress(Key::Alt, ClickType::Press))
                ));
                assert!(a.next().is_none());
            }

            {
                let mut mapping = mapping.clone();
                let t = tbdoub;
                mapping.key_down(JoyKey::S, t);
                mapping.key_up(JoyKey::S, t);
                let mut a = mapping.tick(t);
                assert!(matches!(
                    dbg!(a.next()),
                    Some(ExtAction::KeyPress(Key::Space, ClickType::Press))
                ));
                assert!(a.next().is_none());
            }

            {
                let mut mapping = mapping.clone();
                let t = tadoub;
                mapping.key_down(JoyKey::S, t);
                mapping.key_up(JoyKey::S, t);
                let mut a = mapping.tick(t);
                assert!(matches!(
                    dbg!(a.next()),
                    Some(ExtAction::KeyPress(Key::Alt, ClickType::Press))
                ));
                assert!(a.next().is_none());
            }
        }
    }
}
