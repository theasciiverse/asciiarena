//! Computer players. A bot sees the arena the way a player sees the screen and
//! answers with the same actions a player's keys give: it walks at the same
//! speed, pays the same energy and waits out the same cooldowns.

use super::arena::{ArenaView};
use super::arena::entity::{Entity, EntityAction};

use crate::direction::{Direction};
use crate::ids::{SpellId};
use crate::message::{BotLevel};
use crate::specification::{skills, spells};
use crate::vec2::{Vec2};

use rand::{Rng, SeedableRng};
use rand::rngs::{StdRng};

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

struct Traits {
    /// Time between two looks at the arena.
    think: Duration,
    /// Chance to see a fire ball coming in time to step aside.
    dodge: f32,
    /// Chance to fire when lined up with someone.
    aim: f32,
    /// Farthest shot it takes.
    range: i32,
    /// Chance to set off a nova with someone close.
    nova: f32,
    dash: bool,
    wall: bool,
    mine: f32,
    /// How much its idea of the best spot wobbles.
    noise: f32,
}

fn traits(level: BotLevel) -> Traits {
    match level {
        BotLevel::Easy => Traits {
            think: Duration::from_millis(260),
            dodge: 0.25,
            aim: 0.4,
            range: 9,
            nova: 0.2,
            dash: false,
            wall: false,
            mine: 0.0,
            noise: 3.0,
        },
        BotLevel::Normal => Traits {
            think: Duration::from_millis(150),
            dodge: 0.6,
            aim: 0.75,
            range: 12,
            nova: 0.6,
            dash: true,
            wall: false,
            mine: 0.015,
            noise: 1.5,
        },
        BotLevel::Hard => Traits {
            think: Duration::from_millis(80),
            dodge: 0.9,
            aim: 1.0,
            range: 16,
            nova: 1.0,
            dash: true,
            wall: true,
            mine: 0.025,
            noise: 0.6,
        },
    }
}

/// Cells of the arena something hurtful is about to pass through.
struct Danger {
    /// Cell, and how many steps the nearest projectile needs to reach it.
    paths: HashMap<Vec2, (u32, SpellId, Vec2)>,
    mines: HashSet<Vec2>,
}

impl Danger {
    fn read(me: &Entity, arena: &ArenaView, now: Instant) -> Danger {
        let mut paths: HashMap<Vec2, (u32, SpellId, Vec2)> = HashMap::new();
        let mut mines = HashSet::new();
        let mut ids = arena.spells.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            let spell = &arena.spells[&id];
            if spell.entity_origin_id() == me.id() {
                continue
            }
            match spell.spec_id() {
                spells::FIRE_BALL | spells::SPARK => {
                    let reach = match spell.spec().range {
                        Some(range) => range.saturating_sub(spell.travelled()),
                        None => 6,
                    };
                    let mut cell = spell.position();
                    for steps in 0..reach {
                        if !arena.map.is_floor(cell) || arena.has_obstacle(cell) {
                            break
                        }
                        let entry = paths.entry(cell).or_insert((steps, id, spell.step()));
                        if steps < entry.0 {
                            *entry = (steps, id, spell.step());
                        }
                        cell += spell.step();
                    }
                }
                spells::MINE if now >= spell.created() + skills::MINE_ARMING => {
                    mines.insert(spell.position());
                }
                _ => (),
            }
        }
        Danger { paths, mines }
    }

    fn at(&self, cell: Vec2) -> bool {
        self.mines.contains(&cell) || self.paths.get(&cell).map(|(steps, ..)| *steps <= 4).unwrap_or(false)
    }
}

pub struct BotBrain {
    level: BotLevel,
    traits: Traits,
    rng: StdRng,
    next_think: Instant,
    dodges: HashMap<SpellId, bool>,
    wander_to: Option<Vec2>,
}

impl BotBrain {
    pub fn new(level: BotLevel, seed: u64, now: Instant) -> BotBrain {
        let mut rng = StdRng::seed_from_u64(seed);
        let traits = traits(level);
        // Not all bots look up at the same moment.
        let first = Duration::from_millis(rng.random_range(0..=traits.think.as_millis() as u64));
        BotBrain { level, traits, rng, next_think: now + first, dodges: HashMap::new(), wander_to: None }
    }

    pub fn think(&mut self, now: Instant, me: &Entity, arena: &ArenaView) -> Vec<EntityAction> {
        if now < self.next_think {
            return vec![]
        }
        let jitter = self.rng.random_range(0..=self.traits.think.as_millis() as u64 / 4);
        self.next_think = now + self.traits.think + Duration::from_millis(jitter);

        let here = me.position();
        let danger = Danger::read(me, arena, now);
        self.dodges.retain(|id, _| arena.spells.contains_key(id));

        if let Some(action) = self.dodge(now, me, arena, &danger) {
            log::trace!("Bot {} dodges with {:?}", me.character().symbol(), action);
            return vec![action]
        }

        let mut enemies = arena.entities.values().filter(|entity| entity.id() != me.id()).collect::<Vec<_>>();
        enemies.sort_by_key(|entity| entity.id().0);

        let target = match self.level {
            BotLevel::Hard => enemies.iter().min_by_key(|enemy| {
                (enemy.position() - here).manhattan_length() * 4 + enemy.health() as i32 / 10
            }),
            _ => enemies.iter().min_by_key(|enemy| (enemy.position() - here).manhattan_length()),
        }
        .copied();

        let mut actions = Vec::new();
        if let Some(target) = target {
            if let Some(action) = self.attack(now, me, target, &enemies, arena) {
                actions.push(action);
            }
        }

        if me.can_walk(now) {
            if let Some(direction) = self.choose_step(me, target, arena, &danger) {
                actions.push(EntityAction::Walk(direction));
            }
        }
        actions
    }

    /// Steps out of the way of whatever is flying at us, if it is seen in time.
    fn dodge(&mut self, now: Instant, me: &Entity, arena: &ArenaView, danger: &Danger) -> Option<EntityAction> {
        let here = me.position();
        let (steps, spell_id, step) = *danger.paths.get(&here)?;
        let chance = self.traits.dodge;
        let roll = self.rng.random::<f32>();
        let seen = *self.dodges.entry(spell_id).or_insert(roll < chance);
        if !seen {
            return None
        }

        let incoming = Direction::from_vec2(step);
        let mut sides = match incoming {
            Some(direction) => direction.perpendiculars().to_vec(),
            None => Direction::ALL.to_vec(),
        };
        if self.rng.random::<bool>() {
            sides.reverse();
        }

        if me.can_walk(now) {
            for side in &sides {
                let cell = here + side.to_vec2();
                if arena.is_walkable(cell) && !danger.at(cell) && !arena.in_storm(cell) {
                    return Some(EntityAction::Walk(*side))
                }
            }
        }

        if self.traits.wall && steps >= 3 && me.can_cast(skills::WALL, now) {
            if let Some(towards) = incoming.map(|direction| direction.opposite()) {
                return Some(EntityAction::Cast(towards, skills::WALL))
            }
        }

        if self.traits.dash && me.can_cast(skills::DASH, now) {
            for side in &sides {
                if arena.is_walkable(here + side.to_vec2()) {
                    return Some(EntityAction::Cast(*side, skills::DASH))
                }
            }
        }
        None
    }

    fn attack(
        &mut self,
        now: Instant,
        me: &Entity,
        target: &Entity,
        enemies: &[&Entity],
        arena: &ArenaView,
    ) -> Option<EntityAction> {
        let here = me.position();

        let nova_hits = enemies
            .iter()
            .filter(|enemy| {
                let delta = enemy.position() - here;
                let on_a_line = delta.x == 0 || delta.y == 0 || delta.x.abs() == delta.y.abs();
                on_a_line && delta.chebyshev_length() <= 3
            })
            .count();
        if nova_hits > 0 && me.can_cast(skills::NOVA, now) && self.rng.random::<f32>() < self.traits.nova {
            return Some(EntityAction::Cast(me.direction(), skills::NOVA))
        }

        let delta = target.position() - here;
        if let Some(direction) = Direction::from_vec2(delta) {
            let lined_up = delta.manhattan_length() <= self.traits.range && clear_line(here, target.position(), arena);
            if lined_up && me.can_cast(skills::FIRE_BALL, now) && self.rng.random::<f32>() < self.traits.aim {
                return Some(EntityAction::Cast(direction, skills::FIRE_BALL))
            }
        }

        let distance = delta.manhattan_length();
        if (3..=7).contains(&distance)
            && me.energy() >= 45
            && me.can_cast(skills::MINE, now)
            && self.rng.random::<f32>() < self.traits.mine
        {
            return Some(EntityAction::Cast(me.direction(), skills::MINE))
        }
        None
    }

    /// Looks over the cells it can reach and heads for the best one: lined up
    /// with the target at a fair distance, out of the storm and out of harm's way.
    fn choose_step(
        &mut self,
        me: &Entity,
        target: Option<&Entity>,
        arena: &ArenaView,
        danger: &Danger,
    ) -> Option<Direction> {
        const MAX_DEPTH: u32 = 12;
        let here = me.position();
        let in_storm_now = arena.in_storm(here);

        if target.is_none() {
            let reached = self.wander_to.map(|goal| goal == here).unwrap_or(true);
            if reached || self.rng.random::<f32>() < 0.02 {
                let floors = arena.map.floor_positions().collect::<Vec<_>>();
                self.wander_to = Some(floors[self.rng.random_range(0..floors.len())]);
            }
        }

        let mut parents: HashMap<Vec2, (Vec2, u32)> = HashMap::new();
        let mut queue = VecDeque::from(vec![here]);
        parents.insert(here, (here, 0));
        while let Some(cell) = queue.pop_front() {
            let depth = parents[&cell].1;
            if depth >= MAX_DEPTH {
                continue
            }
            for direction in Direction::ALL {
                let next = cell + direction.to_vec2();
                if parents.contains_key(&next) || !arena.is_walkable(next) {
                    continue
                }
                if danger.mines.contains(&next) {
                    continue
                }
                if arena.in_storm(next) && !in_storm_now {
                    continue
                }
                parents.insert(next, (cell, depth + 1));
                queue.push_back(next);
            }
        }

        let next_ring = arena.storm + 1;
        let mut best: Option<(f32, Vec2)> = None;
        let mut cells = parents.keys().copied().collect::<Vec<_>>();
        cells.sort_by_key(|cell| (cell.y, cell.x));
        for cell in cells {
            let depth = parents[&cell].1;
            let mut score = -(depth as f32) * 0.4;
            if arena.in_storm(cell) {
                score -= 20.0;
            }
            else if arena.map.ring(cell) <= next_ring {
                score -= 2.0 * (next_ring as f32 - arena.map.ring(cell) as f32 + 1.0).min(3.0);
            }
            if danger.at(cell) {
                score -= 15.0;
            }
            match target {
                Some(target) => {
                    let delta = target.position() - cell;
                    let distance = delta.manhattan_length();
                    let lined_up = (delta.x == 0 || delta.y == 0)
                        && distance <= self.traits.range
                        && clear_line(cell, target.position(), arena);
                    if lined_up {
                        score += 8.0;
                    }
                    if distance < 3 {
                        score -= 4.0;
                    }
                    if distance > self.traits.range {
                        score -= 0.3 * (distance - self.traits.range) as f32;
                    }
                    if self.level != BotLevel::Easy && me.health() < 35 && distance < 5 {
                        score -= 3.0;
                    }
                }
                None => {
                    if let Some(goal) = self.wander_to {
                        score -= (goal - cell).manhattan_length() as f32 * 0.5;
                    }
                }
            }
            score += self.rng.random::<f32>() * self.traits.noise;
            if best.map(|(top, _)| score > top).unwrap_or(true) {
                best = Some((score, cell));
            }
        }

        let (_, mut goal) = best?;
        if goal == here {
            return None
        }
        while parents[&goal].0 != here {
            goal = parents[&goal].0;
        }
        Direction::from_vec2(goal - here)
    }
}

/// Nothing solid between two cells in a row or column.
pub fn clear_line(from: Vec2, to: Vec2, arena: &ArenaView) -> bool {
    let step = (to - from).signum();
    if step.x != 0 && step.y != 0 {
        return false
    }
    let mut cell = from + step;
    while cell != to {
        if !arena.map.is_floor(cell) || arena.has_obstacle(cell) {
            return false
        }
        cell += step;
    }
    true
}
