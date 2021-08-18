# Gyromouse

A crossplatform mapper from gamepad inputs to keyboard and mouse actions, with special care for good gyro controls. Useful to play games and control a computer from the couch.

## OS support

This project supports both Windows, Linux and macOS.

## Quickstart

1. Download the latest release at https://github.com/Yamakaky/gyromouse/releases ;
2. Create a `default.txt` file in the same directory as `gyromouse`. Possible content is described below. You can start with one of the examples at https://github.com/Yamakaky/gyromouse/tree/master/mappings ;
3. Run `gyromouse`, either by double click (Windows) or in a terminal (Linux, macOS). This will run the input mapper using the configuration in `default.txt`.

### Windows

No special setup are needed. When launching, you may need to [allow the app in Defender Smartscreen](https://www.addictivetips.com/windows-tips/whitelist-apps-in-the-smartscreen-on-windows-10) or in your antivirus.

### Linux

`gyromouse` needs access rights to the controller device in `/dev` to access every features, the gyroscope in particular. If you don't see `Starting calibration, don't move the controller...` in the console when pluging in the controller, try doing one of the following:

1. Install udev rules from steam, for example `usr/lib/udev/rules.d/70-steam-input.rules` from the [Steam package in Archlinux](https://archlinux.org/packages/multilib/x86_64/steam/download), if a similar file is not installed by your distro;
2. Put your user in the `input` group and reboot (`usermod -a -G input <myuser>`);
3. Give yourself access to the raw hid devices `chmod 666 /dev/hidraw*` (temporary, lasts until next reboot));
4. Run `gyromouse` as root using `sudo` (last resort, not recommended).

### macOS

TODO

## Configuration

`gyromouse` uses the same configuration format as [JoyShockMapper](https://github.com/Electronicks/JoyShockMapper#commands). Implemented features:
- [X] Digital inputs
    - [X] Most controller inputs
    - [X] Most simple keys (letters, enter, space...)
    - [X] Tap, hold, simultaneous, double and chorded press
    - [X] Modifiers
    - [ ] Only one key supported by action
- [ ] Advanced triggers
- [X] Sticks
    - [X] AIM, FLICK, FLICK_ONLY, ROTATE_ONLY, NO_MOUSE
    - [ ] MOUSE_RING, MOUSE_AREA, SCROLL_WHEEL
    - [ ] Ring only for NO_MOUSE
- [X] Gyro
    - [X] Most settings
    - [X] Local, world and player space
    - [ ] Basic sensor fusion only
    - [ ] Calibration on connection only

