use crate::version::{Compatibility};
use crate::character::{CharacterId, Character};
use crate::vec2::{Vec2};
use crate::direction::{Direction};
use crate::ids::{SessionToken, EntityId, SpellId, SpellSpecId, SkillId};

use serde::{Serialize, Deserialize};

use std::time::{Duration};

// ===================================================
//     High level messages
// ===================================================

/// Messages that Client sends to Server
#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    // Version
    Version(String),

    // Server info
    SubscribeServerInfo,

    // Login messages
    Login(char),
    Logout,

    // Udp handshake
    ConnectUdp(SessionToken),
    TrustUdp,

    // Arena real time messages
    MovePlayer(Direction),
    CastSkill(Direction, SkillId),
}

/// Messages that Server sends to Client
#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    // Version
    Version(String, Compatibility),

    // Server info
    StaticServerInfo(ServerInfo),
    DynamicServerInfo(Vec<char>), //player list

    // Login messages
    LoginStatus(char, LoginStatus), //player, status

    // Udp handshake
    UdpConnected,

    // Game messages
    StartGame(GameInfo),
    FinishGame(Vec<Standing>),
    GameEvent(GameEvent),
    GameStep(Frame),

    // Arena messages
    WaitArena(Duration),
    StartArena(ArenaInfo),
}

// ===================================================
//     Composable message pieces
// ===================================================
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum LoggedKind {
    FirstTime,
    Reconnection,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum LoginStatus {
    Logged(SessionToken, LoggedKind),
    InvalidPlayerName,
    AlreadyLogged,
    PlayerLimit,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ServerInfo {
    pub udp_port: u16,
    pub players_number: u8,
    pub map_size: u16,
    pub winner_points: u16,
    pub logged_players: Vec<char>,
    pub rounds: u16,
    pub bots: u8,
    pub bot_level: BotLevel,
    pub arena: String,
}

/// How well computer players fight.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotLevel {
    Easy,
    Normal,
    Hard,
}

impl BotLevel {
    pub const ALL: [BotLevel; 3] = [BotLevel::Easy, BotLevel::Normal, BotLevel::Hard];

    pub fn name(&self) -> &'static str {
        match self {
            BotLevel::Easy => "Easy",
            BotLevel::Normal => "Normal",
            BotLevel::Hard => "Hard",
        }
    }

    pub fn from_name(name: &str) -> Option<BotLevel> {
        Self::ALL.into_iter().find(|level| level.name().eq_ignore_ascii_case(name))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerInfo {
    pub character_id: CharacterId,
    pub points: usize,
    pub kills: usize,
    pub rounds_won: usize,
    pub bot: Option<BotLevel>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GameInfo {
    pub characters: Vec<Character>,
    pub players: Vec<PlayerInfo>,
    pub rounds: usize,
    pub winner_points: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Terrain {
    Floor,
    Wall,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ArenaInfo {
    pub number: usize,
    pub players: Vec<EntityId>, //id
    pub ground: Vec<Terrain>,
    pub name: String,
    pub width: u16,
    pub height: u16,
    /// Time from the start until the storm starts closing in.
    pub storm_after: Duration,
}

/// What took a player out.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Cause {
    Spell(SpellSpecId),
    Storm,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum GameEvent {
    PlayerPointsUpdated(Vec<usize>),
    /// Victim, killer (if a player), what did it.
    PlayerKilled(char, Option<char>, Cause),
    /// The last one standing in an arena, if anyone is.
    ArenaWinner(Option<char>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityData {
    pub id: EntityId,
    pub character_id: CharacterId,
    pub position: Vec2,
    pub health: usize,
    pub energy: usize,
    pub direction: Direction,
    /// Milliseconds until each skill can be cast again.
    pub cooldowns: Vec<u16>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SpellData {
    pub id: SpellId,
    pub spec_id: SpellSpecId,
    pub position: Vec2,
    pub owner: EntityId,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Frame {
    pub entities: Vec<EntityData>,
    pub spells: Vec<SpellData>,
    /// Rings of the arena the storm has covered, counted from the outer wall.
    pub storm: u16,
}

/// One line of the final table.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Standing {
    pub symbol: char,
    pub points: usize,
    pub kills: usize,
    pub rounds_won: usize,
    pub bot: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{self, Encoder};

    #[test]
    fn frames_survive_the_trip() {
        let frame = ServerMessage::GameStep(Frame {
            entities: vec![EntityData {
                id: EntityId(3),
                character_id: CharacterId::Player('Q'),
                position: Vec2::xy(4, 7),
                health: 90,
                energy: 12,
                direction: Direction::Left,
                cooldowns: vec![0, 1200, 0, 0, 300],
            }],
            spells: vec![SpellData {
                id: SpellId(9),
                spec_id: SpellSpecId(1),
                position: Vec2::xy(5, 7),
                owner: EntityId(3),
            }],
            storm: 2,
        });
        let mut encoder = Encoder::new();
        let data = encoder.encode(frame).to_vec();
        match encoding::decode::<ServerMessage>(&data) {
            Some(ServerMessage::GameStep(frame)) => {
                assert_eq!(frame.entities[0].position, Vec2::xy(4, 7));
                assert_eq!(frame.entities[0].cooldowns[1], 1200);
                assert_eq!(frame.spells[0].owner, EntityId(3));
                assert_eq!(frame.storm, 2);
            }
            other => panic!("unexpected {:?}", other),
        }
        assert!(encoding::decode::<ServerMessage>(&[250, 1, 2]).is_none());
    }
}

// See the protocol diagram in docs/design/communication.md
