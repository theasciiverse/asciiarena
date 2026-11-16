use super::session::{RoomSession, SessionStatus};
use super::game::{Game, GameConfig, Contender};
use super::game::arena::{Arena};

use crate::message::{
    ClientMessage, ServerMessage, ServerInfo, GameInfo, ArenaInfo, LoginStatus, LoggedKind,
    EntityData, Frame, GameEvent, SpellData, PlayerInfo, BotLevel, Cause,
};
use crate::specification::{skills, spells};
use crate::encoding::{self, Encoder};
use crate::version::{self, Compatibility};
use crate::direction::{Direction};
use crate::ids::{SessionToken, SkillId};
use crate::util::{self};

use message_io::node::{self, NodeHandler, NodeListener, NodeEvent};
use message_io::network::{Endpoint, Transport, NetEvent};

use itertools::{Itertools};

use rand::{Rng, SeedableRng};
use rand::rngs::{StdRng};
use rand::seq::{SliceRandom};

use std::time::{Duration, Instant};
use std::collections::{HashSet};
use std::net::{IpAddr, SocketAddr};

const GAME_STEP_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 30);

/// Steps between two snapshots in the debug log, about half a second.
const SNAPSHOT_STEPS: usize = 15;

#[derive(Debug)]
enum Signal {
    AsyncCreateGame, // Could take time in processing
    AsyncStartArena, // Generated Eventually
    GameStep,        // Generated Eventually
    Close,           // Main loop control
}

pub struct Config {
    /// Where to listen: 0.0.0.0 for the local network, 127.0.0.1 for a game on this computer.
    pub interface: IpAddr,
    /// 0 takes any free port.
    pub tcp_port: u16,
    pub udp_port: u16,
    /// People the game waits for.
    pub players_number: u8,
    /// Computer players added when the game starts.
    pub bots: usize,
    pub bot_level: BotLevel,
    /// The computer plays for the people too (`--demo`).
    pub autopilot: Option<BotLevel>,
    pub game: GameConfig,
    /// Pause before each arena, to see who won the last one.
    pub arena_waiting: Duration,
    /// Pause before the first arena.
    pub first_arena_waiting: Duration,
}

/// Stops a running server from another thread.
#[derive(Clone)]
pub struct ServerHandle {
    node: NodeHandler<Signal>,
}

impl ServerHandle {
    pub fn stop(&self) {
        self.node.signals().send_with_priority(Signal::Close);
    }
}

pub struct ServerManager {
    config: Config,
    encoder: Encoder,
    node: NodeHandler<Signal>,
    listener: Option<NodeListener<Signal>>,
    subscriptions: HashSet<Endpoint>,
    room: RoomSession<char>,
    game: Option<Game>,
    waiting_arena_from: Option<Instant>,
    waiting_arena_for: Duration,
    rng: StdRng,
    steps: usize,
}

impl ServerManager {
    pub fn new(mut config: Config) -> Option<ServerManager> {
        let (node, listener) = node::split();

        let interface = config.interface;
        match node.network().listen(Transport::FramedTcp, (interface, config.tcp_port)) {
            Ok((_, addr)) => config.tcp_port = addr.port(),
            Err(_) => {
                log::error!("Can not run server on TCP port {}", config.tcp_port);
                return None
            }
        }

        match node.network().listen(Transport::Udp, (interface, config.udp_port)) {
            Ok((_, addr)) => config.udp_port = addr.port(),
            Err(_) => {
                log::error!("Can not run server on UDP port {}", config.udp_port);
                return None
            }
        }

        log::info!(
            "Server running on {} ports {} (tcp) and {} (udp) for {} players and {} bots",
            interface,
            config.tcp_port,
            config.udp_port,
            config.players_number,
            config.bots,
        );

        Some(ServerManager {
            encoder: Encoder::new(),
            node,
            listener: Some(listener),
            subscriptions: HashSet::new(),
            room: RoomSession::new(config.players_number as usize),
            game: None,
            waiting_arena_from: None,
            waiting_arena_for: Duration::ZERO,
            rng: StdRng::seed_from_u64(config.game.seed),
            steps: 0,
            config,
        })
    }

    pub fn handle(&self) -> ServerHandle {
        ServerHandle { node: self.node.clone() }
    }

    /// The address clients connect to.
    pub fn tcp_addr(&self) -> SocketAddr {
        SocketAddr::new(self.config.interface, self.config.tcp_port)
    }

    fn send_to_client(&mut self, endpoint: Endpoint, message: ServerMessage) {
        self.node.network().send(endpoint, self.encoder.encode(message));
    }

    fn send_to_all_clients(&mut self, endpoints: Vec<Endpoint>, message: ServerMessage) {
        let output_data = self.encoder.encode(message);
        for endpoint in endpoints {
            self.node.network().send(endpoint, output_data);
        }
    }

    pub fn run(mut self) {
        let listener = self.listener.take().unwrap();
        listener.for_each(move |event| match event {
            NodeEvent::Signal(signal) => match signal {
                Signal::AsyncCreateGame => self.process_create_game(),
                Signal::AsyncStartArena => self.process_start_arena(),
                Signal::GameStep => self.process_game_step(),
                Signal::Close => {
                    log::info!("Closing server");
                    self.node.stop();
                }
            },
            NodeEvent::Network(network) => match network {
                NetEvent::Connected(endpoint, _) => log::trace!("{} has connected", endpoint),
                NetEvent::Accepted(endpoint, _) => log::debug!("{} has connected", endpoint),
                NetEvent::Disconnected(endpoint) => {
                    log::trace!("{} has disconnected", endpoint);
                    self.process_disconnection(endpoint);
                }
                NetEvent::Message(endpoint, data) => {
                    log::trace!("Message from {}", endpoint.addr());
                    match encoding::decode::<ClientMessage>(data) {
                        Some(message) => match message {
                            ClientMessage::Version(client_version) => {
                                self.process_version(endpoint, &client_version);
                            }
                            ClientMessage::SubscribeServerInfo => {
                                self.process_subscribe_server_info(endpoint);
                            }
                            ClientMessage::Login(user) => {
                                self.process_login(endpoint, user);
                            }
                            ClientMessage::Logout => {
                                self.process_logout(endpoint);
                            }
                            ClientMessage::ConnectUdp(session_token) => {
                                self.process_connect_udp(endpoint, session_token);
                            }
                            ClientMessage::TrustUdp => {
                                self.process_trust_udp(endpoint);
                            }
                            ClientMessage::MovePlayer(direction) => {
                                self.process_move_player(endpoint, direction);
                            }
                            ClientMessage::CastSkill(direction, id) => {
                                self.process_cast_skill(endpoint, direction, id);
                            }
                        },
                        None => {
                            log::error!(
                                "{} sends an unknown message. Connection rejected",
                                endpoint
                            );
                            self.node.network().remove(endpoint.resource_id());
                        }
                    };
                }
            },
        });
    }

    fn process_version(&mut self, endpoint: Endpoint, client_version: &str) {
        let compatibility = version::check(client_version, version::current());
        match compatibility {
            Compatibility::Fully => log::trace!("Fully compatible versions: {}", client_version),
            Compatibility::NotExact => log::warn!(
                "Compatible client version, but not exact. Client: {}. Server: {}",
                client_version,
                version::current()
            ),
            Compatibility::None => log::warn!(
                "Incompatible client version. Client: {}. Server: {}. Connection rejected",
                client_version,
                version::current()
            ),
        };

        let message = ServerMessage::Version(version::current().into(), compatibility);
        self.send_to_client(endpoint, message);

        if let Compatibility::None = compatibility {
            self.node.network().remove(endpoint.resource_id());
        }
    }

    fn process_subscribe_server_info(&mut self, endpoint: Endpoint) {
        let info = ServerInfo {
            udp_port: self.config.udp_port,
            players_number: self.config.players_number,
            map_size: self.config.game.map_size as u16,
            winner_points: self.config.game.winner_points as u16,
            logged_players: self.room.sessions().map(|session| *session.user()).collect(),
            rounds: self.config.game.rounds as u16,
            bots: self.config.bots as u8,
            bot_level: self.config.bot_level,
            arena: self.config.game.arena.name().into(),
        };

        log::trace!("Client {} has subscribed to server info", endpoint.addr());
        self.subscriptions.insert(endpoint);
        let message = ServerMessage::StaticServerInfo(info);
        self.send_to_client(endpoint, message);
    }

    fn process_login(&mut self, endpoint: Endpoint, player_symbol: char) {
        let status = if !util::is_valid_character(player_symbol) {
            log::warn!("Invalid character symbol '{}' has tried to login", player_symbol);
            LoginStatus::InvalidPlayerName
        }
        else {
            match self.room.create_session(player_symbol, endpoint) {
                SessionStatus::Created(token) => {
                    let player_symbols =
                        self.room.sessions().map(|session| session.user()).sorted();

                    log::info!(
                        "New player logged: {}, current players: {}",
                        player_symbol,
                        util::format::items_to_string(player_symbols)
                    );
                    LoginStatus::Logged(token, LoggedKind::FirstTime)
                }
                SessionStatus::Recycled(token) => {
                    log::info!("Player '{}' reconnected", player_symbol);
                    LoginStatus::Logged(token, LoggedKind::Reconnection)
                }
                SessionStatus::AlreadyLogged => {
                    log::warn!(
                        "Player '{}' has tried to login but the character symbol is already logged",
                        player_symbol
                    );
                    LoginStatus::AlreadyLogged
                }
                SessionStatus::Full => {
                    log::warn!(
                        "Player '{}' has tried to login but the player limit has been reached",
                        player_symbol
                    );
                    LoginStatus::PlayerLimit
                }
            }
        };

        log::trace!(
            "{} with player '{}' attempts to login. Status: {:?}",
            endpoint.addr(),
            player_symbol,
            status
        );

        let message = ServerMessage::LoginStatus(player_symbol, status);
        self.send_to_client(endpoint, message);

        if let LoginStatus::Logged(_, kind) = status {
            match kind {
                LoggedKind::FirstTime => {
                    let player_symbols =
                        self.room.sessions().map(|session| *session.user()).collect();

                    let message = ServerMessage::DynamicServerInfo(player_symbols);
                    let subscriptions = self.subscriptions.iter().cloned().collect();
                    self.send_to_all_clients(subscriptions, message);

                    if self.game.is_none() && self.room.is_full() {
                        self.node.signals().send(Signal::AsyncCreateGame);
                    }
                }
                LoggedKind::Reconnection => {
                    if let Some(game) = &self.game {
                        let message = Self::create_start_game_message(game);
                        self.send_to_client(endpoint, message);

                        if let Some(waiting_from) = self.waiting_arena_from {
                            let duration = Instant::now().duration_since(waiting_from);
                            let waiting = self
                                .waiting_arena_for
                                .checked_sub(duration)
                                .unwrap_or_else(|| Duration::new(0, 0));
                            let message = ServerMessage::WaitArena(waiting);
                            self.send_to_client(endpoint, message);
                        }

                        let game = self.game.as_ref().unwrap();
                        if game.arena().is_some() {
                            let message = Self::create_start_arena_message(game);
                            self.send_to_client(endpoint, message);
                        }
                    }
                }
            }
        }
    }

    fn process_logout(&mut self, endpoint: Endpoint) {
        if self.game.is_some() {
            if let Some(session) = self.room.session_by_endpoint_mut(endpoint) {
                session.disconnect();
                log::info!("Player '{}' disconnected", session.user());
            }
        }
        else if let Some(session) = self.room.remove_session_by_endpoint(endpoint) {
            let player_symbols =
                self.room.sessions().map(|session| *session.user()).collect::<Vec<_>>();

            log::info!(
                "Player '{}' logout, current players: {} ",
                session.user(),
                util::format::items_to_string(player_symbols.iter().sorted())
            );

            let message = ServerMessage::DynamicServerInfo(player_symbols);
            let subscriptions = self.subscriptions.iter().cloned().collect();
            self.send_to_all_clients(subscriptions, message);
        }
    }

    fn process_connect_udp(&mut self, udp_endpoint: Endpoint, session_token: SessionToken) {
        match self.room.session_mut(session_token) {
            Some(session) => {
                log::trace!("Attached udp endpoint to session '{}'", session_token);
                session.set_untrusted_fast_endpoint(udp_endpoint);
                let message = ServerMessage::UdpConnected;
                self.send_to_client(udp_endpoint, message);
            }
            None => log::warn!(
                "Attempt to attach udp endpoint to non-existent session '{}'",
                session_token
            ),
        }
    }

    fn process_trust_udp(&mut self, related_tcp_endpoint: Endpoint) {
        match self.room.session_by_endpoint_mut(related_tcp_endpoint) {
            Some(session) => match session.trust_in_fast_endpoint() {
                Some(_) => log::trace!("Trusted udp endpoint for session '{}'", session.token()),
                None => log::error!(
                    "Attempt to trust into a non-existent udp endpoint. Session '{}'",
                    session.token()
                ),
            },
            None => log::error!("Attempt to trust an udp endpoint in an non-existent session"),
        }
    }

    fn process_create_game(&mut self) {
        let humans = self.room.sessions().map(|session| *session.user()).collect::<Vec<_>>();
        let mut free = ('A'..='Z').filter(|symbol| !humans.contains(symbol)).collect::<Vec<_>>();
        free.shuffle(&mut self.rng);

        let mut contenders = humans
            .iter()
            .map(|&symbol| Contender { symbol, bot: self.config.autopilot, human: true })
            .collect::<Vec<_>>();
        contenders.extend(free.into_iter().take(self.config.bots).map(|symbol| Contender {
            symbol,
            bot: Some(self.config.bot_level),
            human: false,
        }));

        log::info!(
            "Starting new game: {}",
            util::format::items_to_string(contenders.iter().map(|c| match c.bot {
                Some(level) if !c.human => format!("{} (bot, {})", c.symbol, level.name()),
                _ => c.symbol.to_string(),
            }))
        );

        let game_config = GameConfig { seed: self.rng.random(), ..self.config.game.clone() };
        let game = Game::new(game_config, contenders);

        let message = Self::create_start_game_message(&game);
        self.send_to_all_clients(self.room.safe_endpoints(), message);

        self.game = Some(game);
        self.process_wait_arena(self.config.first_arena_waiting);

        self.node.signals().send(Signal::GameStep);
    }

    fn process_wait_arena(&mut self, waiting: Duration) {
        log::trace!("Initializing next arena in {} seconds...", waiting.as_secs_f32());

        let message = ServerMessage::WaitArena(waiting);
        self.send_to_all_clients(self.room.safe_endpoints(), message);

        self.node.signals().send_with_timer(Signal::AsyncStartArena, waiting);

        self.waiting_arena_from = Some(Instant::now());
        self.waiting_arena_for = waiting;
    }

    fn process_start_arena(&mut self) {
        self.waiting_arena_from = None;
        self.game.as_mut().unwrap().create_new_arena(Instant::now());
        let game = self.game.as_ref().unwrap();
        let arena = game.arena().unwrap();
        log::info!("Start arena {}: {}", game.arena_number(), arena.map().name());

        let entities = arena.entities();
        let player_positions = game
            .players()
            .iter()
            .map(|(symbol, player)| {
                let entity = entities.get(&player.entity_id());

                let position = match entity {
                    Some(entity) => entity.position().to_string(),
                    None => "-".into(),
                };
                (symbol, position)
            })
            .collect::<Vec<_>>();

        log::trace!("Player positions: {}", util::format::pair_items_to_string(player_positions));

        let message = Self::create_start_arena_message(game);
        self.send_to_all_clients(self.room.safe_endpoints(), message);
    }

    fn process_game_step(&mut self) {
        log::trace!("Processing step");

        let game = self.game.as_mut().unwrap();
        let events = game.step(Instant::now());

        if let Some(arena) = game.arena() {
            let message = Self::create_game_step_message(arena);
            self.send_to_all_clients(self.room.faster_endpoints(), message);
        }

        self.steps += 1;
        if self.steps % SNAPSHOT_STEPS == 0 {
            self.log_snapshot();
        }

        for event in events {
            match &event {
                GameEvent::PlayerKilled(victim, killer, cause) => {
                    let by = match cause {
                        Cause::Spell(spell) => spells::spec(*spell).name,
                        Cause::Storm => "the storm",
                    };
                    match killer {
                        Some(killer) => log::info!("{} is out: {} of {}", victim, by, killer),
                        None => log::info!("{} is out: {}", victim, by),
                    }
                }
                GameEvent::ArenaWinner(winner) => log::info!("Arena winner: {:?}", winner),
                GameEvent::PlayerPointsUpdated(_) => {
                    let game = self.game.as_ref().unwrap();
                    let player_total_points_pairs = game
                        .pole()
                        .iter()
                        .map(|player| (player.character().symbol(), player.points()))
                        .collect::<Vec<_>>();
                    log::info!("Points: {}", util::format::pair_items_to_string(player_total_points_pairs));
                }
            }
            let message = ServerMessage::GameEvent(event);
            self.send_to_all_clients(self.room.safe_endpoints(), message);
        }

        let game = self.game.as_ref().unwrap();
        if game.has_finished() {
            log::info!("End game");
            let message = ServerMessage::FinishGame(game.standings());
            self.send_to_all_clients(self.room.safe_endpoints(), message);
            self.process_reset();
        }
        else {
            if game.is_arena_over() && self.waiting_arena_from.is_none() {
                log::info!("End arena");
                self.process_wait_arena(self.config.arena_waiting);
            }
            self.node.signals().send_with_timer(Signal::GameStep, GAME_STEP_DURATION);
        }
    }

    fn log_snapshot(&self) {
        let game = match &self.game {
            Some(game) => game,
            None => return,
        };
        if let Some(arena) = game.arena() {
            let mut entities = arena.entities().values().collect::<Vec<_>>();
            entities.sort_by_key(|entity| entity.character().symbol());
            log::debug!(
                "snapshot: arena {} {} storm {} spells {} | {}",
                game.arena_number(),
                arena.map().name(),
                arena.storm(),
                arena.spells().len(),
                entities
                    .iter()
                    .map(|entity| format!(
                        "{} {} hp {} en {}",
                        entity.character().symbol(),
                        entity.position(),
                        entity.health(),
                        entity.energy()
                    ))
                    .join(", ")
            );
        }
    }

    fn process_move_player(&mut self, endpoint: Endpoint, direction: Direction) {
        match self.room.session_by_endpoint(endpoint) {
            Some(session) => match self.game.as_mut() {
                Some(game) => {
                    let player = game.player_mut(*session.user()).unwrap();
                    if player.is_alive() {
                        player.walk(direction);
                    }
                }
                None => log::warn!("Client attempted to move a player without a created game"),
            },
            None => log::warn!("Unlogged client attempted to move a player. Maybe an attack?"),
        };
    }

    fn process_cast_skill(&mut self, endpoint: Endpoint, direction: Direction, id: SkillId) {
        match self.room.session_by_endpoint(endpoint) {
            Some(session) => match self.game.as_mut() {
                Some(game) => {
                    let player = game.player_mut(*session.user()).unwrap();
                    if player.is_alive() {
                        player.cast(direction, id);
                    }
                }
                None => log::warn!("Client attempted to cast a skill without a created game"),
            },
            None => log::warn!("Unlogged client attempted to cast a skill. Maybe an attack?"),
        };
    }

    fn process_reset(&mut self) {
        log::info!("Reset server");
        self.game = None;
        self.room.clear();

        let player_symbols = self.room.sessions().map(|session| *session.user()).collect();

        let message = ServerMessage::DynamicServerInfo(player_symbols);
        let subscriptions = self.subscriptions.iter().cloned().collect();
        self.send_to_all_clients(subscriptions, message);
    }

    fn process_disconnection(&mut self, endpoint: Endpoint) {
        if self.subscriptions.remove(&endpoint) {
            log::trace!("Client {} has unsubscribed to server info", endpoint.addr());
        }
        self.process_logout(endpoint);
    }

    fn create_start_game_message(game: &Game) -> ServerMessage {
        let game_info = GameInfo {
            characters: game
                .characters()
                .iter()
                .map(|(_, character)| (**character).clone())
                .collect(),
            players: game
                .players()
                .iter()
                .map(|(_, player)| PlayerInfo {
                    character_id: player.character().id(),
                    points: player.points(),
                    kills: player.kills(),
                    rounds_won: player.rounds_won(),
                    bot: if player.is_human() { None } else { player.bot() },
                })
                .collect(),
            rounds: game.config().rounds,
            winner_points: game.config().winner_points,
        };

        ServerMessage::StartGame(game_info)
    }

    fn create_start_arena_message(game: &Game) -> ServerMessage {
        let arena = game.arena().unwrap();
        let arena_info = ArenaInfo {
            number: game.arena_number(),
            players: game.players().iter().map(|(_, player)| player.entity_id()).collect(),
            ground: arena.map().ground().clone(),
            name: arena.map().name().into(),
            width: arena.map().width() as u16,
            height: arena.map().height() as u16,
            storm_after: arena.storm_after(),
        };

        ServerMessage::StartArena(arena_info)
    }

    fn create_game_step_message(arena: &Arena) -> ServerMessage {
        let now = Instant::now();
        let entities = arena
            .entities()
            .values()
            .map(|entity| EntityData {
                id: entity.id(),
                character_id: entity.character().id(),
                position: entity.position(),
                health: entity.health(),
                energy: entity.energy(),
                direction: entity.direction(),
                cooldowns: skills::all()
                    .map(|(id, _)| entity.cooldown(id, now).as_millis().min(u16::MAX as u128) as u16)
                    .collect(),
            })
            .collect();

        let spells = arena
            .spells()
            .values()
            .map(|spell| SpellData {
                id: spell.id(),
                spec_id: spell.spec_id(),
                position: spell.position(),
                owner: spell.entity_origin_id(),
            })
            .collect();

        ServerMessage::GameStep(Frame { entities, spells, storm: arena.storm() })
    }
}
