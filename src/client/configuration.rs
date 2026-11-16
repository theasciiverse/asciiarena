use crate::message::{BotLevel};
use crate::server::{ArenaChoice, GameConfig};
use crate::settings::{Settings};

use std::net::{SocketAddr};
use std::time::{Duration};

/// How the client starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Start {
    /// The main menu.
    Home,
    /// The network lobby, as `asciiarena client`.
    Join,
    /// Straight into a match against bots, as `asciiarena solo`.
    Solo,
    /// Bots play every letter, you watch, as `asciiarena --demo`.
    Demo,
}

pub struct Config {
    pub server_addr: Option<SocketAddr>,
    pub character: Option<char>,
    pub start: Start,
    pub options: MatchOptions,
}

/// The match the menu sets up, remembered between games.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchOptions {
    pub letter: char,
    pub bots: usize,
    pub level: BotLevel,
    pub arena: ArenaChoice,
    pub rounds: usize,
    /// People a hosted game waits for, the host included.
    pub lan_players: usize,
    /// From the command line only: points that end a match early, 0 for none.
    pub points: usize,
    /// From the command line only: time before the storm.
    pub storm_after: Duration,
}

impl Default for MatchOptions {
    fn default() -> Self {
        MatchOptions {
            letter: 'A',
            bots: 3,
            level: BotLevel::Normal,
            arena: ArenaChoice::Rotate,
            rounds: 5,
            lan_players: 2,
            points: 0,
            storm_after: GameConfig::default().storm_after,
        }
    }
}

impl MatchOptions {
    pub const MAX_BOTS: usize = 7;
    pub const ROUNDS: [usize; 5] = [1, 3, 5, 7, 10];

    pub fn load(settings: &Settings) -> MatchOptions {
        let default = MatchOptions::default();
        MatchOptions {
            letter: settings
                .get("letter")
                .and_then(|value| value.chars().next())
                .filter(|c| c.is_ascii_uppercase())
                .unwrap_or(default.letter),
            bots: settings.get_number("bots").filter(|n| *n <= Self::MAX_BOTS).unwrap_or(default.bots),
            level: settings.get("level").and_then(BotLevel::from_name).unwrap_or(default.level),
            arena: settings.get("arena").and_then(ArenaChoice::from_name).unwrap_or(default.arena),
            rounds: settings.get_number("rounds").filter(|n| (1..=99).contains(n)).unwrap_or(default.rounds),
            lan_players: settings
                .get_number("lan_players")
                .filter(|n| (2..=8).contains(n))
                .unwrap_or(default.lan_players),
            ..default
        }
    }

    pub fn save(&self, settings: &mut Settings) {
        settings.set("letter", self.letter);
        settings.set("bots", self.bots);
        settings.set("level", self.level.name());
        settings.set("arena", self.arena.name());
        settings.set("rounds", self.rounds);
        settings.set("lan_players", self.lan_players);
    }
}
