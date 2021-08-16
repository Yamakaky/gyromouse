#![cfg_attr(test, allow(dead_code, unreachable_code, unused_variables))]

mod backend;
mod calibration;
mod config;
mod engine;
mod gyromouse;
mod joystick;
mod mapping;
mod mouse;
mod opts;
mod space_mapper;

use std::{fs::File, io::Read};

use anyhow::Context;
use backend::Backend;
use clap::Clap;
use nom_supreme::error::{BaseErrorKind, ErrorTree};
use opts::Opts;

use crate::{config::settings::Settings, mapping::Buttons, opts::Run};

#[derive(Debug, Copy, Clone)]
pub enum ClickType {
    Press,
    Release,
    Click,
    Toggle,
}

impl ClickType {
    pub fn apply(self, val: bool) -> bool {
        match self {
            ClickType::Press => false,
            ClickType::Release => true,
            ClickType::Click => unimplemented!(),
            ClickType::Toggle => !val,
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|p| {
        eprintln!("\n/!\\ A crash occured /!\\\n    {}", p);
        eprintln!("\nPlease report it at <https://github.com/Yamakaky/gyromouse/issues>.")
    }));

    if let Err(e) = do_main() {
        eprintln!("Error: {:?}", e);
    }

    // Keep cmd.exe opened
    #[cfg(windows)]
    let _ = stdin.read(&mut [0u8]).unwrap();
}

fn do_main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    #[allow(unreachable_patterns)]
    let mut backend: Box<dyn Backend> = match opts.backend {
        #[cfg(feature = "sdl2")]
        Some(opts::Backend::Sdl) | None => Box::new(backend::sdl::SDLBackend::new()?),
        #[cfg(feature = "hidapi")]
        Some(opts::Backend::Hid) | None => Box::new(backend::hidapi::HidapiBackend::new()?),
        Some(_) | None => {
            println!("A backend must be enabled");
            return Ok(());
        }
    };

    let mut settings = Settings::default();
    let mut bindings = Buttons::new();

    match opts.cmd {
        Some(opts::Cmd::Validate(v)) => {
            let mut content_file = File::open(&v.mapping_file)
                .with_context(|| format!("opening config file \"{}\"", v.mapping_file))?;
            let content = {
                let mut buf = String::new();
                content_file.read_to_string(&mut buf)?;
                buf
            };
            match config::parse::parse_file(&content, &mut settings, &mut bindings) {
                Ok(_) => {}
                Err(e) => {
                    print_parse_error(
                        &content,
                        &e.map_locations(|l| {
                            let line = content
                                .lines()
                                .skip(l.line - 1)
                                .next()
                                .expect("should not fail");
                            format!("line {}, \"{}\"", l.line, line)
                        }),
                    );
                    //dbg!(e);
                }
            };
            Ok(())
        }
        Some(opts::Cmd::FlickCalibrate) => todo!(),
        Some(opts::Cmd::Run(r)) => run(r, backend, settings, bindings),
        Some(opts::Cmd::List) => backend.list_devices(),
        None => {
            println!("Using default config file \"Default.txt\".");
            run(
                Run {
                    mapping_file: "Default.txt".to_string(),
                },
                backend,
                settings,
                bindings,
            )
        }
    }
}
fn run(
    r: Run,
    mut backend: Box<dyn Backend>,
    mut settings: Settings,
    mut bindings: Buttons,
) -> anyhow::Result<()> {
    let mut content_file = File::open(&r.mapping_file)
        .with_context(|| format!("opening config file {}", r.mapping_file))?;
    let content = {
        let mut buf = String::new();
        content_file.read_to_string(&mut buf)?;
        buf
    };
    config::parse::parse_file(&content, &mut settings, &mut bindings)?;
    backend.run(r, settings, bindings)
}

fn print_parse_error(input: &str, e: &ErrorTree<String>) {
    match e {
        ErrorTree::Base { location, kind } => {
            eprintln!("Error parsing {}: {}", location, kind,);
        }
        ErrorTree::Stack { base, contexts } => {
            eprintln!("{:?}", contexts);
            print_parse_error(input, &base);
        }
        ErrorTree::Alt(alts) => {
            let mut last_loc = None;
            for alt in alts {
                if let ErrorTree::Base {
                    location,
                    kind: BaseErrorKind::Expected(exp),
                } = alt
                {
                    match last_loc.map(|l: &String| l == location) {
                        None => print!("  at {}: expected {}", location, exp),
                        Some(false) => print!("\n  at {}: expected {}", location, exp),
                        Some(true) => print!(" or {}", exp),
                    }
                    last_loc = Some(location);
                } else {
                    println!();
                    print_parse_error(input, alt);
                    last_loc = None;
                }
            }
            println!();
        }
    }
}
