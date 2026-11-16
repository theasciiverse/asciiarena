use super::state::{
    State, StaticGameInfo, VersionInfo, GameStatus, Arena, Player, UserPlayer, Session, Server,
    FeedItem,
};
use super::server_proxy::{ServerApi, ApiCall, ConnectionStatus, ServerEvent};
use super::configuration::{MatchOptions};

use crate::message::{GameEvent, BotLevel};
use crate::character::{CharacterId};
use crate::direction::{Direction};
use crate::ids::{EntityId, SkillId};
use crate::server::{self, LocalServer, GameConfig, ArenaChoice};
use crate::settings::{Settings};
use crate::version::{self};

use std::net::{SocketAddr, IpAddr, Ipv4Addr, UdpSocket};
use std::time::{Instant, Duration};
use std::collections::{HashMap};

/// Lines of the kill feed kept on screen.
const FEED_LENGTH: usize = 2;

/// Action API
#[derive(Debug)]
pub enum Action {
    StartApp,
    Connect(SocketAddr),
    Disconnect,
    Login(char),
    Logout,
    CloseGame,
    CloseApp,
    MovePlayer(Direction),
    CastSkill(SkillId),
    ServerEvent(ServerEvent),
    /// A match against bots on this computer; the bool saves the options for next time.
    StartSolo(MatchOptions, bool),
    /// Bots play every letter.
    StartDemo,
    /// Serves a game to the local network and plays in it.
    HostGame(MatchOptions),
    /// Opens the lobby to join a game on the local network.
    JoinGame,
    /// Same letter, same server, next match.
    Rematch,
    /// Back to the main menu, ending a local server if there is one.
    LeaveGame,
    /// The menu changed a match option.
    SetOptions(MatchOptions),
}

pub struct Store {
    state: State,
    server: ServerApi,
    close: bool,
    local: Option<LocalServer>,
    settings: Settings,
}

impl Store {
    pub fn new(state: State, server: ServerApi, settings: Settings) -> Store {
        Store { state, server, close: false, local: None, settings }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn should_close(&self) -> bool {
        self.close
    }

    fn save_options(&mut self, options: &MatchOptions) {
        self.state.options = options.clone();
        options.save(&mut self.settings);
        self.settings.save();
    }

    fn start_local(&mut self, config: server::Config) -> Option<SocketAddr> {
        self.local = None; // An old one stops first.
        match LocalServer::start(config) {
            Some(local) => {
                let addr = local.addr();
                self.local = Some(local);
                Some(addr)
            }
            None => None,
        }
    }

    fn local_config(options: &MatchOptions, interface: IpAddr, port: u16, players: usize) -> server::Config {
        server::Config {
            interface,
            tcp_port: port,
            udp_port: port,
            players_number: players as u8,
            bots: options.bots,
            bot_level: options.level,
            autopilot: None,
            game: GameConfig {
                rounds: options.rounds,
                winner_points: options.points,
                storm_after: options.storm_after,
                arena: options.arena,
                seed: rand::random(),
                ..GameConfig::default()
            },
            arena_waiting: Duration::from_secs(3),
            first_arena_waiting: Duration::from_secs(3),
        }
    }

    fn enter_local_game(&mut self, session: Session, addr: SocketAddr, letter: char) {
        log::info!("Entering {:?} at {}", session, addr);
        self.state.problem = None;
        self.state.session = Some(session);
        self.state.server = Server::new(Some(addr));
        self.state.user.character_symbol = Some(letter);
        self.state.user.login_status = None;
        self.server.call(ApiCall::Connect(addr));
    }

    pub fn dispatch(&mut self, action: Action) {
        match &action {
            Action::ServerEvent(ServerEvent::GameStep(_)) => log::trace!("Dispatch: GameStep"),
            action => log::debug!("Dispatch: {:?}", action),
        }
        match action {
            Action::StartApp => {
                if let Some(addr) = self.state.server.addr {
                    self.state.session = Some(Session::Join);
                    self.server.call(ApiCall::Connect(addr));
                }
            }

            Action::Connect(addr) => {
                self.state.server.addr = Some(addr);
                self.settings.set("server", addr);
                self.settings.save();
                self.server.call(ApiCall::Connect(addr));
            }

            Action::Disconnect => {
                self.state.server.addr = None;
                self.server.call(ApiCall::Disconnect);
            }

            Action::Login(character) => {
                self.state.user.character_symbol = Some(character);
                self.state.user.last_character = Some(character);
                self.server.call(ApiCall::Login(character));
            }

            Action::Logout => {
                self.state.user.character_symbol = None;
                self.state.user.login_status = None;
                self.server.call(ApiCall::Logout);
            }

            Action::CloseGame => {
                self.state.server.game.status = GameStatus::NotStarted;
                self.state.server.game.arena = None;
            }

            Action::CloseApp => {
                self.close = true;
            }

            Action::MovePlayer(direction) => {
                if let Some(arena) = self.state.server.game.arena.as_mut() {
                    arena.user_player.direction = direction;
                    self.server.call(ApiCall::MovePlayer(direction));
                }
            }

            Action::CastSkill(id) => {
                if let Some(arena) = self.state.server.game.arena.as_ref() {
                    let direction = arena.user_player.direction;
                    self.server.call(ApiCall::CastSkill(direction, id));
                }
            }

            Action::SetOptions(options) => {
                self.save_options(&options);
            }

            Action::StartSolo(options, save) => {
                match save {
                    true => self.save_options(&options),
                    false => self.state.options = options.clone(),
                }
                let config = Self::local_config(&options, IpAddr::V4(Ipv4Addr::LOCALHOST), 0, 1);
                match self.start_local(config) {
                    Some(addr) => self.enter_local_game(Session::Solo(options.clone()), addr, options.letter),
                    None => self.state.problem = Some("Could not start a game on this computer".into()),
                }
            }

            Action::StartDemo => {
                let options = MatchOptions {
                    bots: 5,
                    level: BotLevel::Normal,
                    rounds: 3,
                    arena: ArenaChoice::Rotate,
                    ..self.state.options.clone()
                };
                let mut config = Self::local_config(&options, IpAddr::V4(Ipv4Addr::LOCALHOST), 0, 1);
                config.autopilot = Some(BotLevel::Hard);
                config.game.storm_after = Duration::from_secs(18);
                config.arena_waiting = Duration::from_secs(3);
                config.first_arena_waiting = Duration::from_millis(300);
                match self.start_local(config) {
                    Some(addr) => self.enter_local_game(Session::Demo, addr, options.letter),
                    None => self.state.problem = Some("Could not start the demo".into()),
                }
            }

            Action::HostGame(options) => {
                self.save_options(&options);
                let interface = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
                let started = (0..10).find_map(|offset| {
                    let port = server::DEFAULT_PORT + offset;
                    let config = Self::local_config(&options, interface, port, options.lan_players);
                    self.start_local(config)
                });
                match (started, self.local.as_ref().map(|local| local.port())) {
                    (Some(addr), Some(port)) => {
                        let session = Session::Host { lan_ip: lan_address(), port };
                        self.enter_local_game(session, addr, options.letter);
                    }
                    _ => self.state.problem = Some("Could not open a port for the local network".into()),
                }
            }

            Action::JoinGame => {
                self.local = None;
                self.state.problem = None;
                self.state.session = Some(Session::Join);
                self.state.server = Server::new(None);
                self.state.user.character_symbol = None;
                self.state.user.login_status = None;
            }

            Action::Rematch => {
                if let Some(letter) = self.state.user.last_character {
                    self.state.server.game.status = GameStatus::NotStarted;
                    self.state.server.game.arena = None;
                    self.dispatch(Action::Login(letter));
                }
            }

            Action::LeaveGame => {
                self.server.call(ApiCall::Disconnect);
                self.local = None;
                self.state.session = None;
                self.state.server = Server::new(None);
                self.state.user.character_symbol = None;
                self.state.user.login_status = None;
            }

            Action::ServerEvent(server_event) => self.process_server_event(server_event),
        }
    }

    fn process_server_event(&mut self, server_event: ServerEvent) {
        match server_event {
            ServerEvent::ConnectionResult(status) => {
                log::info!("Connection: {:?}", status);
                self.state.server.connection_status = status;
                if let ConnectionStatus::Connected = status {
                    self.server.call(ApiCall::CheckVersion(version::current().into()));
                }
                else {
                    //No connected (no matter the reason)
                    self.state.server.game.status = GameStatus::Finished;
                    self.state.server.udp_confirmed = None;
                    self.state.user.character_symbol = None;
                    self.state.user.login_status = None;
                    self.state.server.logged_players = Vec::new();
                    self.state.server.game.arena = None;
                    self.state.server.game_info = None;
                    let lost = matches!(status, ConnectionStatus::Lost | ConnectionStatus::NotFound);
                    if lost && self.state.session.as_ref().map(|s| s.is_local()).unwrap_or(false) {
                        self.state.session = None;
                        self.state.problem = Some("The game on this computer stopped".into());
                    }
                }
            }

            ServerEvent::CheckedVersion(server_version, compatibility) => {
                let version_info = VersionInfo { version: server_version, compatibility };
                self.state.server.version_info = Some(version_info);

                if compatibility.is_compatible() {
                    self.server.call(ApiCall::SubscribeInfo);
                }
                else {
                    // Protect the client against an unknown or not compatible server version
                    self.server.call(ApiCall::Disconnect);
                }
            }

            ServerEvent::StaticServerInfo(info) => {
                let game_info = StaticGameInfo {
                    players_number: info.players_number as usize,
                    winner_points: info.winner_points as usize,
                    rounds: info.rounds as usize,
                    bots: info.bots as usize,
                    bot_level: info.bot_level,
                    arena: info.arena,
                };
                self.state.server.udp_port = Some(info.udp_port);
                self.state.server.game_info = Some(game_info);
                self.state.server.logged_players = info.logged_players;

                if let Some(character) = self.state.user.character_symbol {
                    self.dispatch(Action::Login(character));
                }
            }

            ServerEvent::DynamicServerInfo(logged_players) => {
                self.state.server.logged_players = logged_players;
            }

            ServerEvent::LoginStatus(status) => {
                log::info!("Login: {:?}", status);
                self.state.user.login_status = Some(status);
            }

            ServerEvent::UdpReachable(value) => {
                self.state.server.udp_confirmed = Some(value);
            }

            ServerEvent::StartGame(game_info) => {
                log::info!("Game starts: {} players, {} rounds", game_info.players.len(), game_info.rounds);
                let game = &mut self.state.server.game;
                game.status = GameStatus::Started;
                game.rounds = game_info.rounds;
                game.winner_points = game_info.winner_points;
                game.feed.clear();
                game.standings.clear();
                game.arena_winner = None;
                game.characters = game_info
                    .characters
                    .into_iter()
                    .map(|character| (character.id(), character))
                    .collect();

                game.players = game_info
                    .players
                    .into_iter()
                    .enumerate()
                    .map(|(index, info)| Player {
                        id: index,
                        character_id: info.character_id,
                        entity_id: EntityId::NONE,
                        points: info.points,
                        kills: info.kills,
                        rounds_won: info.rounds_won,
                        bot: info.bot,
                    })
                    .collect();
            }

            ServerEvent::FinishGame(standings) => {
                log::info!("Game over: {:?}", standings);
                self.state.server.game.status = GameStatus::Finished;
                self.state.server.game.standings = standings;
                self.state.server.udp_confirmed = None;
                self.state.user.character_symbol = None;
                self.state.user.login_status = None;
                if let Some(arena) = self.state.server.game.arena.as_mut() {
                    arena.entities = HashMap::new();
                    arena.spells = HashMap::new();
                }
            }

            ServerEvent::WaitArena(duration) => {
                self.state.server.game.next_arena_timestamp = Some(Instant::now() + duration);
            }

            ServerEvent::StartArena(arena_info) => {
                log::info!("Arena {} starts: {}", arena_info.number, arena_info.name);
                let user_symbol = self.state.user.character_symbol.or(self.state.user.last_character);
                let game = &mut self.state.server.game;
                game.next_arena_timestamp = None;
                game.arena_number = arena_info.number;
                game.arena_winner = None;

                for (i, player) in arena_info.players.into_iter().enumerate() {
                    game.players[i].entity_id = player;
                }

                let player_id = game
                    .players
                    .iter()
                    .position(|player| match player.character_id {
                        CharacterId::Player(symbol) => Some(symbol) == user_symbol,
                        _ => false,
                    })
                    .unwrap_or(0);

                game.arena = Some(Arena {
                    entities: HashMap::new(),
                    spells: HashMap::new(),
                    user_player: UserPlayer { player_id, direction: Direction::Down },
                    name: arena_info.name,
                    width: arena_info.width as usize,
                    height: arena_info.height as usize,
                    ground: arena_info.ground,
                    storm: 0,
                    started: Instant::now(),
                    storm_after: arena_info.storm_after,
                });
            }

            ServerEvent::GameEvent(game_event) => {
                let game = &mut self.state.server.game;
                match game_event {
                    GameEvent::PlayerPointsUpdated(player_points) => {
                        for (i, points) in player_points.into_iter().enumerate() {
                            if let Some(player) = game.players.get_mut(i) {
                                player.points = points;
                            }
                        }
                    }
                    GameEvent::PlayerKilled(victim, killer, cause) => {
                        log::info!("{} is out by {:?} ({:?})", victim, killer, cause);
                        if let Some(killer) = killer {
                            let symbol = |p: &&mut Player| matches!(p.character_id, CharacterId::Player(s) if s == killer);
                            if let Some(player) = game.players.iter_mut().find(|p| symbol(p)) {
                                player.kills += 1;
                            }
                        }
                        game.feed.push(FeedItem { victim, killer, cause, at: Instant::now() });
                        if game.feed.len() > FEED_LENGTH {
                            game.feed.remove(0);
                        }
                    }
                    GameEvent::ArenaWinner(winner) => {
                        log::info!("Arena {} goes to {:?}", game.arena_number, winner);
                        if let Some(winner) = winner {
                            let symbol = |p: &&mut Player| matches!(p.character_id, CharacterId::Player(s) if s == winner);
                            if let Some(player) = game.players.iter_mut().find(|p| symbol(p)) {
                                player.rounds_won += 1;
                            }
                        }
                        game.arena_winner = Some(winner);
                    }
                }
            }

            ServerEvent::GameStep(frame) => {
                if let Some(arena) = self.state.server.game.arena.as_mut() {
                    if arena.storm != frame.storm {
                        log::debug!("Storm: {} rings", frame.storm);
                    }
                    arena.storm = frame.storm;
                    arena.entities =
                        frame.entities.into_iter().map(|entity| (entity.id, entity)).collect();
                    arena.spells = frame.spells.into_iter().map(|spell| (spell.id, spell)).collect();
                }
            }
        }
    }
}

/// This computer's address on the local network, to tell the others. Asks the
/// system which interface would carry a packet out, without sending one.
fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(10, 254, 254, 254), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    match ip.is_unspecified() || ip.is_loopback() {
        true => None,
        false => Some(ip),
    }
}
