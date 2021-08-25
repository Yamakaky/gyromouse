use crate::{
    mapping::{Action, Buttons, Layer},
    ClickType,
};

use self::{parse::Error, settings::Settings, types::*};

mod parse;
pub mod settings;
pub mod types;

pub fn parse_file<'a>(
    source: &'a str,
    settings: &mut Settings,
    mapping: &mut Buttons,
) -> Vec<nom::Err<Error<'a>>> {
    let (cmds, errors) = parse::jsm_parse(source);
    for cmd in cmds {
        match cmd {
            Cmd::Map(Key::Simple(key), ref actions) => map_key(mapping.get(key, 0), actions),

            Cmd::Map(Key::Chorded(k1, k2), ref actions) if k1 == k2 => {
                assert_eq!(
                    actions.len(),
                    1,
                    "only one action is supported on double click"
                );
                let action = actions[0];
                assert_eq!(
                    action.event_mod, None,
                    "event modificators not supported on double click"
                );
                mapping.get(k1, 0).on_double_click = convert_action_mod(&action, ClickType::Click);
            }
            Cmd::Map(Key::Chorded(k1, k2), ref actions) => {
                mapping.get(k1, 0).on_down = Some(Action::Layer(k1.to_layer(), true));
                mapping.get(k1, 0).on_up = Some(Action::Layer(k1.to_layer(), false));
                map_key(mapping.get(k2, k1.to_layer()), actions);
            }
            Cmd::Map(Key::Simul(_k1, _k2), ref _actions) => {
                // TODO: Support simultaneous key presses
                eprintln!("Warning: simultaneous keys are unsupported for now");
            }
            Cmd::Setting(setting) => settings.apply(setting),
            Cmd::Reset => {
                settings.reset();
                mapping.reset()
            }
            Cmd::Special(s) => {
                // TODO: Support special key presses
                eprintln!("Warning: special key {:?} is unsupported for now", s);
            }
        }
    }
    errors
}

fn convert_action_mod(action: &JSMAction, default: ClickType) -> Option<Action> {
    if let ActionType::Special(s) = action.action {
        if s == SpecialKey::None {
            return None;
        }
    }
    let action_type = match action.action_mod {
        None => default,
        Some(ActionModifier::Toggle) => ClickType::Toggle,
        Some(ActionModifier::Instant) => ClickType::Click,
    };
    Some(Action::Ext((action.action, action_type).into()))
}

fn map_key(layer: &mut Layer, actions: &[JSMAction]) {
    use EventModifier::*;

    let mut first = true;
    for action in actions {
        match action.event_mod.unwrap_or_else(|| {
            if first {
                if actions.len() == 1 {
                    Start
                } else {
                    Tap
                }
            } else {
                Hold
            }
        }) {
            Tap => {
                layer.on_click = convert_action_mod(action, ClickType::Click);
            }
            Hold => {
                layer.on_hold_down = convert_action_mod(action, ClickType::Press);
                if action.action_mod.is_none() {
                    layer.on_hold_up = convert_action_mod(action, ClickType::Release);
                }
            }
            Start => {
                layer.on_down = convert_action_mod(action, ClickType::Press);
                if action.action_mod.is_none() {
                    layer.on_up = convert_action_mod(action, ClickType::Release);
                }
            }
            Release => {
                assert_eq!(
                    action.action_mod, None,
                    "action modifier non supported on release event type"
                );
                layer.on_up = convert_action_mod(action, ClickType::Release);
            }
            Turbo => {
                // TODO: Implement turbo keys
                eprintln!("Warning: Turbo event modifier is unsupported for now.");
            }
        }
        first = false;
    }
}
#[cfg(test)]
mod test {
    use crate::config::parse::jsm_parse;

    #[test]
    fn parse_all_settings() {
        let settings_str = include_str!("all-settings-example");
        let (_, errors) = jsm_parse(settings_str);
        dbg!(&errors);
        assert!(errors.is_empty());
    }
}
