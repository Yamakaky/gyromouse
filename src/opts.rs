use std::{path::PathBuf, str::FromStr};

use clap::{Parser, ValueEnum};

/// Input mapper from gamepad keypress and movement to mouse and keyboard.
///
/// See <https://github.com/Yamakaky/gyromouse/blob/master/README.md> for more
/// information about features and configuration file format.
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Opts {
    /// Force the use of a specific backend for gamepad access.
    #[arg(short, long)]
    pub backend: Option<Backend>,
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum Backend {
    #[cfg(feature = "sdl2")]
    Sdl,
    #[cfg(feature = "hidapi")]
    Hid,
}

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Validate the syntax of a configuration file.
    Validate(Run),
    /// Compute the value of REAL_WORLD_CALIBRATION.
    #[command(hide = true)]
    FlickCalibrate,
    /// Run the program using the specified configuration file.
    Run(Run),
    /// List connected gamepads.
    List,
}

#[derive(Debug, Parser)]
pub struct Run {
    /// Configuration file to use.
    pub mapping_file: PathBuf,
}

impl FromStr for Backend {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "sdl2")]
            "sdl" => Ok(Backend::Sdl),
            #[cfg(feature = "hidapi")]
            "hid" => Ok(Backend::Hid),
            _ => Err(format!("unknown backend: {}", s)),
        }
    }
}
