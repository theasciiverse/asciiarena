mod application;
pub mod configuration;
mod state;
mod store;
mod server_proxy;
mod terminal;

use application::{Application};
use configuration::{Config, Start, MatchOptions};

use crate::logger::{self};
use crate::server::{self, MatchSettings};
use crate::settings::{Settings};

use clap::{Arg, ArgMatches, Command};

use std::net::{SocketAddr};

fn character_arg() -> Arg {
    Arg::new("character")
        .long("character")
        .short('c')
        .value_name("CAPITAL_LETTER")
        .value_parser(|name: &str| match super::util::is_valid_character_name(name) {
            true => Ok(name.chars().next().unwrap()),
            false => Err(String::from("The character must be an unique capital letter")),
        })
        .help("Set the player's character. Must be unique in the server")
}

pub fn configure_cli() -> Command {
    Command::new("client")
        .about("Joins a game on the local network")
        .arg(character_arg())
        .arg(
            Arg::new("host")
                .long("host")
                .value_name("HOST")
                .value_parser(|host: &str| {
                    host.parse::<SocketAddr>()
                        .map_err(|_| String::from("Host must be a valid network address"))
                })
                .help("Set the server address (ip and port). Format example: 192.168.0.56:3549"),
        )
}

pub fn solo_cli() -> Command {
    Command::new("solo")
        .about("Goes straight into a match against computer players")
        .arg(character_arg())
        .args(server::match_args())
}

pub fn run(matches: &ArgMatches, start: Start) {
    logger::init(matches, logger::Mode::Client);

    let settings = Settings::load();
    let mut options = MatchOptions::load(&settings);
    let character = matches.try_get_one::<char>("character").ok().flatten().copied();
    if let Some(letter) = character {
        options.letter = letter;
    }

    if start == Start::Solo {
        let defaults = MatchSettings {
            bots: options.bots,
            level: options.level,
            rounds: options.rounds,
            arena: options.arena,
            ..MatchSettings::default()
        };
        let chosen = MatchSettings::from_matches(matches, defaults);
        options.bots = chosen.bots.max(1);
        options.level = chosen.level;
        options.rounds = chosen.rounds;
        options.arena = chosen.arena;
        options.points = chosen.points;
        options.storm_after = chosen.storm_after;
    }

    let config = Config {
        character,
        server_addr: matches.try_get_one::<SocketAddr>("host").ok().flatten().copied(),
        start,
        options,
    };

    log::info!("asciiarena {} starts: {:?}", env!("CARGO_PKG_VERSION"), start);
    Application::new(config, settings).run();
}
