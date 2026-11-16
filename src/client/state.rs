use super::server_proxy::{ConnectionStatus};
use super::configuration::{Config, MatchOptions};

use crate::version::{Compatibility};
use crate::message::{LoginStatus, EntityData, SpellData, Terrain, BotLevel, Cause, Standing};
use crate::character::{CharacterId, Character};
use crate::direction::{Direction};
use crate::vec2::{Vec2};
use crate::ids::{EntityId, SpellId};

use std::net::{SocketAddr, IpAddr};
use std::time::{Instant, Duration};
use std::collections::{HashMap};

/// What kind of game the client is in.
#[derive(Debug, Clone, PartialEq)]
pub enum Session {
    /// Against bots on this computer.
    Solo(MatchOptions),
    /// Bots play for you too.
    Demo,
    /// This computer runs the server for the local network.
    Host { lan_ip: Option<IpAddr>, port: u16 },
    /// A server elsewhere on the local network.
    Join,
}

impl Session {
    pub fn is_local(&self) -> bool {
        matches!(self, Session::Solo(_) | Session::Demo)
    }
}

pub struct User {
    pub character_symbol: Option<char>,
    pub login_status: Option<LoginStatus>,
    /// The letter last played, for another match.
    pub last_character: Option<char>,
}

impl User {
    pub fn is_logged(&self) -> bool {
        if let Some(LoginStatus::Logged(..)) = self.login_status {
            return true
        }
        false
    }
}

pub struct VersionInfo {
    pub version: String,
    pub compatibility: Compatibility,
}

pub struct StaticGameInfo {
    pub players_number: usize,
    pub winner_points: usize,
    pub rounds: usize,
    pub bots: usize,
    pub bot_level: BotLevel,
    pub arena: String,
}

pub struct UserPlayer {
    pub player_id: usize, // The position of server.arena.players Vec.
    pub direction: Direction,
}

pub struct Arena {
    pub user_player: UserPlayer,
    pub entities: HashMap<EntityId, EntityData>,
    pub spells: HashMap<SpellId, SpellData>,
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub ground: Vec<Terrain>,
    pub storm: u16,
    pub started: Instant,
    pub storm_after: Duration,
}

impl Arena {
    pub fn terrain(&self, position: Vec2) -> Terrain {
        if position.x < 0 || position.y < 0 {
            return Terrain::Wall
        }
        let (x, y) = (position.x as usize, position.y as usize);
        if x >= self.width || y >= self.height {
            return Terrain::Wall
        }
        self.ground[y * self.width + x]
    }

    pub fn ring(&self, position: Vec2) -> u16 {
        crate::server::game::arena::map::ring_of(position, self.width, self.height)
    }

    pub fn in_storm(&self, position: Vec2) -> bool {
        let ring = self.ring(position);
        self.storm > 0 && ring >= 1 && ring <= self.storm
    }
}

/// A line of the kill feed.
#[derive(Debug, Clone)]
pub struct FeedItem {
    pub victim: char,
    pub killer: Option<char>,
    pub cause: Cause,
    pub at: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GameStatus {
    NotStarted,
    Started,
    Finished,
}

pub struct Player {
    pub id: usize, // The position of server.arena.players Vec.
    pub character_id: CharacterId,
    pub entity_id: EntityId,
    pub points: usize,
    pub kills: usize,
    pub rounds_won: usize,
    pub bot: Option<BotLevel>,
}

pub struct Game {
    pub status: GameStatus,
    pub next_arena_timestamp: Option<Instant>,
    pub arena_number: usize,
    pub arena: Option<Arena>,
    pub characters: HashMap<CharacterId, Character>,
    pub players: Vec<Player>,
    pub rounds: usize,
    pub winner_points: usize,
    pub feed: Vec<FeedItem>,
    /// The last one standing in the arena just played, once it is over.
    pub arena_winner: Option<Option<char>>,
    pub standings: Vec<Standing>,
}

impl Game {
    pub fn new() -> Game {
        Game {
            status: GameStatus::NotStarted,
            arena_number: 0,
            next_arena_timestamp: None,
            arena: None,
            characters: HashMap::new(),
            players: Vec::new(),
            rounds: 0,
            winner_points: 0,
            feed: Vec::new(),
            arena_winner: None,
            standings: Vec::new(),
        }
    }

    pub fn symbol_of(&self, player: &Player) -> char {
        self.characters.get(&player.character_id).map(|c| c.symbol()).unwrap_or('?')
    }

    pub fn user_symbol(&self) -> Option<char> {
        let arena = self.arena.as_ref()?;
        self.players.get(arena.user_player.player_id).map(|player| self.symbol_of(player))
    }

    pub fn user_entity(&self) -> Option<&EntityData> {
        let arena = self.arena.as_ref()?;
        let player = self.players.get(arena.user_player.player_id)?;
        arena.entities.get(&player.entity_id)
    }

    pub fn arena(&self) -> &Arena {
        self.arena.as_ref().unwrap()
    }

}

pub struct Server {
    pub addr: Option<SocketAddr>,
    pub connection_status: ConnectionStatus,
    pub udp_port: Option<u16>,
    pub udp_confirmed: Option<bool>,
    pub version_info: Option<VersionInfo>,
    pub game_info: Option<StaticGameInfo>,
    pub logged_players: Vec<char>,
    pub game: Game,
}

impl Server {
    pub fn is_full(&self) -> bool {
        if let Some(StaticGameInfo { players_number, .. }) = self.game_info {
            if players_number == self.logged_players.len() {
                return true
            }
        }
        false
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.connection_status, ConnectionStatus::Connected)
    }

    pub fn has_compatible_version(&self) -> bool {
        if let Some(version_info) = &self.version_info {
            return version_info.compatibility.is_compatible()
        }
        false
    }
}

pub struct State {
    pub user: User,
    pub server: Server,
    pub session: Option<Session>,
    pub options: MatchOptions,
    /// Something went wrong starting a game, for the menu to say.
    pub problem: Option<String>,
}

impl Server {
    pub fn new(addr: Option<SocketAddr>) -> Server {
        Server {
            addr,
            connection_status: ConnectionStatus::NotConnected,
            udp_port: None,
            udp_confirmed: None,
            version_info: None,
            game_info: None,
            logged_players: Vec::new(),
            game: Game::new(),
        }
    }
}

impl State {
    pub fn new(config: &Config) -> State {
        State {
            user: User { character_symbol: config.character, login_status: None, last_character: None },
            server: Server::new(config.server_addr),
            session: None,
            options: config.options.clone(),
            problem: None,
        }
    }
}
