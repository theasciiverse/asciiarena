mod server_manager;
mod session;
pub mod game;

pub use server_manager::{ServerManager, Config, ServerHandle};
pub use game::{GameConfig};
pub use game::arena::map::{ArenaChoice};

use crate::logger::{self};
use crate::message::{BotLevel};

use clap::{Arg, ArgMatches, Command, value_parser};

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread::{self, JoinHandle};
use std::sync::mpsc;
use std::time::{Duration};

pub const DEFAULT_PORT: u16 = 3549;
const DEFAULT_PORT_TEXT: &str = "3549";

pub fn configure_cli() -> Command {
    Command::new("server")
        .about("Runs a game server for the local network, without a screen")
        .arg(
            Arg::new("tcp-port")
                .long("tcp-port")
                .value_name("PORT")
                .default_value(DEFAULT_PORT_TEXT)
                .value_parser(value_parser!(u16))
                .help("Set the tcp port for client connections"),
        )
        .arg(
            Arg::new("udp-port")
                .long("udp-port")
                .value_name("PORT")
                .default_value(DEFAULT_PORT_TEXT)
                .value_parser(value_parser!(u16))
                .help("Set the udp port for client connections"),
        )
        .arg(
            Arg::new("map-size")
                .long("map-size")
                .short('s')
                .value_name("SIZE")
                .default_value("20")
                .value_parser(value_parser!(u16).range(8..=60))
                .help("Set the side of The Pit, the open square arena"),
        )
        .arg(
            Arg::new("players")
                .long("players")
                .short('p')
                .value_name("NUMBER")
                .required(true)
                .value_parser(value_parser!(u8).range(1..=26))
                .help(
                    "Number of players. \
                The game will not start until the number of players has been reached.",
                ),
        )
        .args(match_args())
}

/// Options for a match, shared by `server` and `solo`.
pub fn match_args() -> Vec<Arg> {
    vec![
        Arg::new("bots")
            .long("bots")
            .short('b')
            .value_name("NUMBER")
            .value_parser(value_parser!(u8).range(0..=25))
            .help("Computer players to add when the game starts"),
        Arg::new("level")
            .long("level")
            .value_name("LEVEL")
            .value_parser(["easy", "normal", "hard"])
            .ignore_case(true)
            .help("How well the computer players fight"),
        Arg::new("rounds")
            .long("rounds")
            .short('r')
            .value_name("NUMBER")
            .value_parser(value_parser!(u16).range(1..=99))
            .help("Arenas in a match"),
        Arg::new("points")
            .long("points")
            .value_name("NUMBER")
            .value_parser(value_parser!(u16))
            .help("End the match as soon as someone has this many points (0: play all rounds)"),
        Arg::new("arena")
            .long("arena")
            .short('a')
            .value_name("NAME")
            .value_parser(["rotate", "pit", "pillars", "crossroads", "ruins", "rings"])
            .ignore_case(true)
            .help("The arena to fight in, or a different one every round"),
        Arg::new("storm")
            .long("storm")
            .value_name("SECONDS")
            .value_parser(value_parser!(u16).range(5..=3600))
            .help("Seconds before the storm starts closing in on each arena"),
    ]
}

/// Match settings with defaults for anything not given.
pub struct MatchSettings {
    pub bots: usize,
    pub level: BotLevel,
    pub rounds: usize,
    pub points: usize,
    pub arena: ArenaChoice,
    pub storm_after: Duration,
}

impl MatchSettings {
    pub fn from_matches(matches: &ArgMatches, defaults: MatchSettings) -> MatchSettings {
        MatchSettings {
            bots: matches.get_one::<u8>("bots").map(|n| *n as usize).unwrap_or(defaults.bots),
            level: matches
                .get_one::<String>("level")
                .and_then(|name| BotLevel::from_name(name))
                .unwrap_or(defaults.level),
            rounds: matches.get_one::<u16>("rounds").map(|n| *n as usize).unwrap_or(defaults.rounds),
            points: matches.get_one::<u16>("points").map(|n| *n as usize).unwrap_or(defaults.points),
            arena: matches
                .get_one::<String>("arena")
                .and_then(|name| ArenaChoice::from_name(name))
                .unwrap_or(defaults.arena),
            storm_after: matches
                .get_one::<u16>("storm")
                .map(|secs| Duration::from_secs(*secs as u64))
                .unwrap_or(defaults.storm_after),
        }
    }
}

impl Default for MatchSettings {
    fn default() -> Self {
        MatchSettings {
            bots: 0,
            level: BotLevel::Normal,
            rounds: 5,
            points: 0,
            arena: ArenaChoice::Rotate,
            storm_after: GameConfig::default().storm_after,
        }
    }
}

pub fn run(matches: &ArgMatches) {
    logger::init(matches, logger::Mode::Server);

    let settings = MatchSettings::from_matches(matches, MatchSettings::default());
    let config = Config {
        interface: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        tcp_port: *matches.get_one::<u16>("tcp-port").unwrap(),
        udp_port: *matches.get_one::<u16>("udp-port").unwrap(),
        players_number: *matches.get_one::<u8>("players").unwrap(),
        bots: settings.bots,
        bot_level: settings.level,
        autopilot: None,
        game: GameConfig {
            map_size: *matches.get_one::<u16>("map-size").unwrap() as usize,
            rounds: settings.rounds,
            winner_points: settings.points,
            arena: settings.arena,
            storm_after: settings.storm_after,
            seed: rand::random(),
        },
        arena_waiting: Duration::from_secs(3),
        first_arena_waiting: Duration::from_secs(3),
    };

    if let Some(server_manager) = ServerManager::new(config) {
        let handle = server_manager.handle();
        ctrlc::set_handler(move || handle.stop()).unwrap();
        server_manager.run();
    }
}

/// A server inside this program, for a game on this computer or one this
/// computer hosts. It stops when dropped.
pub struct LocalServer {
    handle: ServerHandle,
    addr: SocketAddr,
    thread: Option<JoinHandle<()>>,
}

impl LocalServer {
    pub fn start(config: Config) -> Option<LocalServer> {
        let (sender, receiver) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("asciiarena: server".into())
            .spawn(move || match ServerManager::new(config) {
                Some(server_manager) => {
                    let _ = sender.send(Some((server_manager.handle(), server_manager.tcp_addr())));
                    server_manager.run();
                }
                None => {
                    let _ = sender.send(None);
                }
            })
            .ok()?;

        match receiver.recv() {
            Ok(Some((handle, addr))) => Some(LocalServer { handle, addr, thread: Some(thread) }),
            _ => {
                let _ = thread.join();
                None
            }
        }
    }

    /// Where a client on this computer reaches the server.
    pub fn addr(&self) -> SocketAddr {
        match self.addr.ip().is_unspecified() {
            true => SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), self.addr.port()),
            false => self.addr,
        }
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.handle.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        log::info!("Local server stopped");
    }
}
