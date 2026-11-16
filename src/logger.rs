use clap::{Arg, ArgMatches};
use log::{LevelFilter};

use std::io::{Write};

pub const LOG_LEVELS: [&str; 7] = ["off", "error", "warning", "info", "debug", "trace", "dev"];

#[derive(PartialEq, Clone, Copy)]
pub enum Mode {
    /// Logs to stdout at info unless told otherwise.
    Server,
    /// Silent unless told otherwise: the screen belongs to the game.
    Client,
}

pub fn global_args() -> Vec<Arg> {
    vec![
        Arg::new("log")
            .long("log")
            .short('l')
            .value_name("LEVEL")
            .value_parser(LOG_LEVELS)
            .global(true)
            .help("Set the log level of verbosity (RUST_LOG works too)"),
        Arg::new("log-file")
            .long("log-file")
            .value_name("FILE")
            .global(true)
            .help("Write the log to this file"),
    ]
}

fn filter_for(level: &str) -> String {
    let crate_name = env!("CARGO_PKG_NAME");
    match level {
        "warning" => format!("{}=warn", crate_name),
        "dev" => "trace".into(),
        level => format!("{}={}", crate_name, level),
    }
}

/// `--log` wins over `RUST_LOG`. A client logs to `--log-file`, to stderr when
/// only `RUST_LOG` is set (`RUST_LOG=debug asciiarena 2>log`), or to
/// asciiarena.log in the state folder when only `--log` is given.
pub fn init(matches: &ArgMatches, mode: Mode) {
    let level = matches.get_one::<String>("log").cloned();
    let env_filter = std::env::var("RUST_LOG").ok().filter(|filter| !filter.is_empty());

    let filter = match (&level, &env_filter, mode) {
        (Some(level), _, _) => filter_for(level),
        (None, Some(filter), _) => filter.clone(),
        (None, None, Mode::Server) => filter_for("info"),
        (None, None, Mode::Client) => return,
    };
    if filter == filter_for("off") {
        return
    }

    let mut builder = env_logger::Builder::new();
    builder.parse_filters(&filter).format_timestamp_millis();

    let file = match matches.get_one::<String>("log-file") {
        Some(path) => Some(std::path::PathBuf::from(path)),
        None if mode == Mode::Client && level.is_some() => {
            crate::settings::state_dir().map(|dir| dir.join("asciiarena.log"))
        }
        None => None,
    };

    match file {
        Some(path) => match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => {
                builder.target(env_logger::Target::Pipe(Box::new(file)));
                builder.write_style(env_logger::WriteStyle::Never);
            }
            Err(error) => {
                let _ = writeln!(std::io::stderr(), "Can not open log file {}: {}", path.display(), error);
                return
            }
        },
        None => {
            builder.target(match mode {
                Mode::Server => env_logger::Target::Stdout,
                Mode::Client => env_logger::Target::Stderr,
            });
        }
    }

    let _ = builder.try_init();
    log::set_max_level(log::max_level().max(LevelFilter::Off));
}
