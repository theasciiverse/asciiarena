pub mod player;
pub mod arena;
pub mod bot;

use player::{Player};
use arena::{Arena, ArenaEvent};
use arena::map::{Map, ArenaKind, ArenaChoice};

use crate::character::{Character, CharacterId, CharacterBuilder};
use crate::ids::{EntityId};
use crate::message::{BotLevel, GameEvent, Standing};
use crate::vec2::{Vec2};

use rand::{Rng, SeedableRng};
use rand::rngs::{StdRng};
use rand::seq::{IteratorRandom};

use std::collections::{HashMap, BTreeMap, BTreeSet};
use std::rc::{Rc};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct GameConfig {
    /// Side of The Pit, the original open arena.
    pub map_size: usize,
    /// Arenas in a match.
    pub rounds: usize,
    /// Points that end the match at once; 0 plays all the rounds.
    pub winner_points: usize,
    pub arena: ArenaChoice,
    /// Time from the start of an arena until the storm starts closing in.
    pub storm_after: Duration,
    pub seed: u64,
}

impl Default for GameConfig {
    fn default() -> Self {
        GameConfig {
            map_size: 20,
            rounds: 5,
            winner_points: 0,
            arena: ArenaChoice::Rotate,
            storm_after: Duration::from_secs(40),
            seed: 0,
        }
    }
}

/// Someone who takes part: a letter, played by a person, by the computer, or both
/// (a person's letter the computer plays for them, as in `--demo`).
#[derive(Debug, Clone, Copy)]
pub struct Contender {
    pub symbol: char,
    pub bot: Option<BotLevel>,
    pub human: bool,
}

pub struct Game {
    config: GameConfig,

    arena_number: usize,
    arena: Option<Arena>,
    arena_over: bool,
    first_kind: usize,

    characters: HashMap<CharacterId, Rc<Character>>,

    players: BTreeMap<char, Player>,
    rng: StdRng,
}

impl Game {
    pub fn new(config: GameConfig, contenders: impl IntoIterator<Item = Contender>) -> Game {
        let mut players = BTreeMap::new();
        let mut characters = HashMap::new();
        for contender in contenders {
            let character = CharacterBuilder::default()
                .id(CharacterId::Player(contender.symbol))
                .symbol(contender.symbol)
                .max_health(Player::MAX_LIFE)
                .max_energy(Player::MAX_ENERGY)
                .speed_base(Player::SPEED_BASE)
                .build()
                .unwrap();
            let character = Rc::new(character);
            characters.insert(character.id(), character.clone());
            players.insert(contender.symbol, Player::new(character, contender.bot, contender.human));
        }

        let mut rng = StdRng::seed_from_u64(config.seed);
        let first_kind = rng.random_range(0..ArenaKind::ALL.len());

        Game { config, arena_number: 0, arena: None, arena_over: false, first_kind, players, characters, rng }
    }

    pub fn config(&self) -> &GameConfig {
        &self.config
    }

    pub fn arena(&self) -> Option<&Arena> {
        self.arena.as_ref()
    }

    pub fn characters(&self) -> &HashMap<CharacterId, Rc<Character>> {
        &self.characters
    }

    pub fn player_mut(&mut self, character_symbol: char) -> Option<&mut Player> {
        self.players.get_mut(&character_symbol)
    }

    pub fn players(&self) -> &BTreeMap<char, Player> {
        &self.players
    }

    pub fn arena_number(&self) -> usize {
        self.arena_number
    }

    pub fn is_arena_over(&self) -> bool {
        self.arena_over
    }

    pub fn pole(&self) -> Vec<&Player> {
        let mut sorted_players = self.players.values().collect::<Vec<_>>();
        sorted_players.sort_by(|a, b| {
            b.points()
                .cmp(&a.points())
                .then(b.kills().cmp(&a.kills()))
                .then(b.rounds_won().cmp(&a.rounds_won()))
                .then(a.character().symbol().cmp(&b.character().symbol()))
        });
        sorted_players
    }

    pub fn standings(&self) -> Vec<Standing> {
        self.pole()
            .into_iter()
            .map(|player| Standing {
                symbol: player.character().symbol(),
                points: player.points(),
                kills: player.kills(),
                rounds_won: player.rounds_won(),
                bot: !player.is_human(),
            })
            .collect()
    }

    fn next_kind(&self) -> ArenaKind {
        match self.config.arena {
            ArenaChoice::Only(kind) => kind,
            ArenaChoice::Rotate => {
                ArenaKind::ALL[(self.first_kind + self.arena_number) % ArenaKind::ALL.len()]
            }
        }
    }

    /// Starting cells as far from each other as the arena allows, away from the outer ring.
    fn spawn_positions(&mut self, map: &Map, count: usize) -> Vec<Vec2> {
        let candidates = map.floor_positions().filter(|cell| map.ring(*cell) >= 2).collect::<Vec<_>>();
        let candidates = match candidates.len() >= count {
            true => candidates,
            false => map.floor_positions().collect(),
        };
        let mut chosen = candidates.iter().copied().choose_multiple(&mut self.rng, 1);
        while chosen.len() < count {
            let farthest = candidates
                .iter()
                .filter(|cell| !chosen.contains(cell))
                .map(|cell| {
                    let nearest = chosen.iter().map(|other| (*other - *cell).manhattan_length()).min().unwrap();
                    (nearest * 16 + self.rng.random_range(0..16), *cell)
                })
                .max_by_key(|(score, _)| *score);
            match farthest {
                Some((_, cell)) => chosen.push(cell),
                None => break,
            }
        }
        // Who gets which corner is luck.
        let mut order = chosen;
        for i in (1..order.len()).rev() {
            let j = self.rng.random_range(0..=i);
            order.swap(i, j);
        }
        order
    }

    pub fn create_new_arena(&mut self, now: Instant) -> &Arena {
        let map = Map::of_kind(self.next_kind(), self.config.map_size);
        let positions = self.spawn_positions(&map, self.players.len());
        let mut arena = Arena::new(map, now, self.config.storm_after);

        let seeds = (0..self.players.len()).map(|_| self.rng.random::<u64>()).collect::<Vec<_>>();
        for (index, player) in self.players.values_mut().enumerate() {
            let position = positions[index % positions.len()];
            let character = player.character().clone();
            let entity = arena.create_entity(character, position, now);
            let entity_id = entity.id();
            entity.set_behaviour(player.create_entity_behaviour(entity_id, seeds[index], now));
        }

        self.arena = Some(arena);
        self.arena_number += 1;
        self.arena_over = false;
        self.arena.as_ref().unwrap()
    }

    /// Moves the arena on to `now` and says what happened.
    pub fn step(&mut self, now: Instant) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let living_players_before = self.living_players();
        let symbols = self
            .players
            .values()
            .filter(|player| player.is_alive())
            .map(|player| (player.entity_id(), player.character().symbol()))
            .collect::<HashMap<EntityId, char>>();

        if let Some(arena) = &mut self.arena {
            arena.update(now);
            for event in arena.take_events() {
                let ArenaEvent::Killed { victim, killer, cause } = event;
                let victim = match symbols.get(&victim) {
                    Some(symbol) => *symbol,
                    None => continue,
                };
                let killer = killer.and_then(|id| symbols.get(&id).copied()).filter(|k| *k != victim);
                if let Some(killer) = killer {
                    self.players.get_mut(&killer).unwrap().add_kill();
                }
                events.push(GameEvent::PlayerKilled(victim, killer, cause));
            }
        }

        let living_players_after = self.living_players();
        let deleted_players = living_players_before.difference(&living_players_after).count();
        if deleted_players > 0 {
            for symbol in &living_players_after {
                let player = self.players.get_mut(symbol).unwrap();
                player.add_points(deleted_players);
            }
            let points = self.players.values().map(|player| player.points()).collect();
            events.push(GameEvent::PlayerPointsUpdated(points));
        }

        if self.arena.is_some() && !self.arena_over && living_players_after.len() <= 1 {
            self.arena_over = true;
            let winner = living_players_after.iter().next().copied();
            if let Some(winner) = winner {
                self.players.get_mut(&winner).unwrap().add_round_won();
            }
            events.push(GameEvent::ArenaWinner(winner));
        }

        events
    }

    pub fn living_players(&self) -> BTreeSet<char> {
        self.players
            .values()
            .filter(|player| player.is_alive())
            .map(|player| player.character().symbol())
            .collect()
    }

    pub fn has_finished(&self) -> bool {
        let by_points = self.config.winner_points > 0
            && self.players.values().any(|player| player.points() >= self.config.winner_points);
        let by_rounds = self.arena_over && self.arena_number >= self.config.rounds;
        by_points || by_rounds
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bots(level: BotLevel, symbols: &str) -> Vec<Contender> {
        symbols.chars().map(|symbol| Contender { symbol, bot: Some(level), human: false }).collect()
    }

    /// Plays a whole match between bots, the way the server does it, without a
    /// network or a clock: time moves on 33 ms a step.
    fn play_match(config: GameConfig, contenders: Vec<Contender>) -> (Game, Vec<GameEvent>, Duration) {
        let start = Instant::now();
        let mut now = start;
        let mut game = Game::new(config, contenders);
        let mut events = Vec::new();
        game.create_new_arena(now);
        loop {
            now += Duration::from_millis(33);
            events.extend(game.step(now));
            if game.has_finished() {
                break
            }
            if game.is_arena_over() {
                now += Duration::from_secs(3);
                game.create_new_arena(now);
            }
            assert!(now - start < Duration::from_secs(60 * 30), "the match never ends");
        }
        (game, events, now - start)
    }

    #[test]
    fn bots_fight_a_whole_match() {
        let config = GameConfig { rounds: 5, seed: 7, ..GameConfig::default() };
        let (game, events, played) = play_match(config, bots(BotLevel::Normal, "ABCDEF"));

        assert_eq!(game.arena_number(), 5);
        let rounds = events.iter().filter(|e| matches!(e, GameEvent::ArenaWinner(_))).count();
        assert_eq!(rounds, 5);
        let kills = events.iter().filter(|e| matches!(e, GameEvent::PlayerKilled(_, Some(_), _))).count();
        assert!(kills >= 5, "bots should take each other out, not only the storm: {} kills", kills);
        let standings = game.standings();
        assert_eq!(standings.len(), 6);
        assert!(standings.windows(2).all(|pair| pair[0].points >= pair[1].points));
        assert!(standings[0].points > 0);
        // Five rounds of six: the storm makes sure no round lasts for ever.
        assert!(played < Duration::from_secs(5 * 120), "{:?}", played);
    }

    #[test]
    fn same_seed_same_match() {
        let config = GameConfig { rounds: 2, seed: 11, ..GameConfig::default() };
        let (first, ..) = play_match(config.clone(), bots(BotLevel::Hard, "XYZ"));
        let (second, ..) = play_match(config, bots(BotLevel::Hard, "XYZ"));
        let table = |game: &Game| {
            game.standings().iter().map(|s| (s.symbol, s.points, s.kills)).collect::<Vec<_>>()
        };
        assert_eq!(table(&first), table(&second));
    }

    #[test]
    fn harder_bots_win_more() {
        let mut hard_points = 0;
        let mut easy_points = 0;
        for seed in 0..4 {
            let config = GameConfig { rounds: 3, seed, arena: ArenaChoice::Only(ArenaKind::Pillars), ..GameConfig::default() };
            let mut contenders = bots(BotLevel::Hard, "HI");
            contenders.extend(bots(BotLevel::Easy, "EF"));
            let (game, ..) = play_match(config, contenders);
            for standing in game.standings() {
                match standing.symbol {
                    'H' | 'I' => hard_points += standing.points,
                    _ => easy_points += standing.points,
                }
            }
        }
        assert!(hard_points > easy_points, "hard {} easy {}", hard_points, easy_points);
    }

    #[test]
    fn points_follow_survival() {
        let config = GameConfig { rounds: 1, seed: 3, ..GameConfig::default() };
        let (game, events, _) = play_match(config, bots(BotLevel::Normal, "ABC"));
        // Three players: the first out gets nothing, the last one standing gets two.
        let mut points = game.players().values().map(|p| p.points()).collect::<Vec<_>>();
        points.sort();
        let winner = events.iter().find_map(|e| match e {
            GameEvent::ArenaWinner(winner) => Some(*winner),
            _ => None,
        });
        match winner.unwrap() {
            Some(symbol) => {
                assert_eq!(points, vec![0, 1, 2]);
                assert_eq!(game.players()[&symbol].rounds_won(), 1);
                assert_eq!(game.standings()[0].symbol, symbol);
            }
            None => assert!(points.iter().sum::<usize>() <= 1),
        }
    }

    #[test]
    fn match_ends_on_points_when_asked() {
        let config = GameConfig { rounds: 50, winner_points: 4, seed: 5, ..GameConfig::default() };
        let (game, ..) = play_match(config, bots(BotLevel::Normal, "ABC"));
        assert!(game.standings()[0].points >= 4);
        assert!(game.arena_number() < 50);
    }

    #[test]
    fn spawns_are_spread_out() {
        let mut game = Game::new(GameConfig { seed: 1, ..GameConfig::default() }, bots(BotLevel::Easy, "ABCDEFGH"));
        let arena = game.create_new_arena(Instant::now());
        let cells = arena.entities().values().map(|e| e.position()).collect::<Vec<_>>();
        for (i, a) in cells.iter().enumerate() {
            for b in &cells[i + 1..] {
                assert!((*a - *b).manhattan_length() >= 4, "{} and {} too close", a, b);
            }
        }
    }
}
