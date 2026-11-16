use super::{ArenaView};

use crate::character::{Character};
use crate::vec2::{Vec2};
use crate::direction::{Direction};
use crate::ids::{EntityId, SkillId};
use crate::message::{Cause};
use crate::specification::skills::{self, SKILL_COUNT, ENERGY_PER_SECOND};

use std::time::{Instant, Duration};
use std::rc::{Rc};
use std::cell::{RefCell, RefMut};

#[derive(Clone, Debug, PartialEq)]
pub enum EntityAction {
    Walk(Direction),
    Cast(Direction, SkillId),
    #[allow(dead_code)] // For behaviours of things that end themselves.
    Destroy,
}

pub trait EntityBehaviour {
    fn destroyed(&mut self) -> Vec<EntityAction>;
    fn update(&mut self, time: Instant, entity: &Entity, arena: &ArenaView) -> Vec<EntityAction>;
}

pub struct Entity {
    id: EntityId,
    character: Rc<Character>,
    behaviour: RefCell<Box<dyn EntityBehaviour>>,
    direction: Direction,
    position: Vec2,
    health: usize,
    energy: usize,
    speed: f32,
    next_walk_time: Instant,
    next_energy_time: Instant,
    skill_ready_time: [Instant; SKILL_COUNT],
    last_hit: Option<(Option<EntityId>, Cause)>,
    next_storm_hit: Instant,
}

impl Entity {
    pub fn new(id: EntityId, character: Rc<Character>, position: Vec2, now: Instant) -> Entity {
        Entity {
            id,
            position,
            behaviour: RefCell::new(get_behaviour(character.id().behaviour_name())),
            direction: Direction::Down,
            health: character.max_health(),
            energy: character.max_energy(),
            speed: character.speed_base(),
            next_walk_time: now,
            next_energy_time: now,
            skill_ready_time: [now; SKILL_COUNT],
            last_hit: None,
            next_storm_hit: now,
            character,
        }
    }

    pub fn id(&self) -> EntityId {
        self.id
    }

    pub fn character(&self) -> &Character {
        &self.character
    }

    pub fn behaviour(&self) -> RefMut<'_, Box<dyn EntityBehaviour>> {
        self.behaviour.borrow_mut()
    }

    pub fn set_behaviour(&mut self, behaviour: Box<dyn EntityBehaviour>) {
        self.behaviour = RefCell::new(behaviour);
    }

    pub fn health(&self) -> usize {
        self.health
    }

    pub fn energy(&self) -> usize {
        self.energy
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn is_alive(&self) -> bool {
        self.health > 0
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.direction = direction;
    }

    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    #[allow(dead_code)] // For spells that push, like the burst of a nova could.
    pub fn displace(&mut self, displacement: Vec2) {
        self.position += displacement;
    }

    pub fn set_health(&mut self, health: usize) {
        self.health = health.min(self.character().max_health());
    }

    #[allow(dead_code)] // For effects; used by tests.
    pub fn set_energy(&mut self, energy: usize) {
        self.energy = energy.min(self.character().max_energy());
    }

    pub fn add_health(&mut self, health: i32) {
        let new_health = (self.health as i32 + health).max(0) as usize;
        self.set_health(new_health);
    }

    #[allow(dead_code)] // For effects that drain or give energy.
    pub fn add_energy(&mut self, energy: i32) {
        let new_energy = (self.energy as i32 + energy).max(0) as usize;
        self.set_energy(new_energy);
    }

    /// Takes damage, remembering who did it for the kill feed.
    pub fn hurt(&mut self, damage: i32, attacker: Option<EntityId>, cause: Cause) {
        if damage > 0 {
            self.add_health(-damage);
            self.last_hit = Some((attacker, cause));
        }
    }

    pub fn last_hit(&self) -> Option<(Option<EntityId>, Cause)> {
        self.last_hit
    }

    pub fn walk(&mut self, current: Instant) -> bool {
        if current >= self.next_walk_time {
            self.position += self.direction.to_vec2();
            self.next_walk_time = current + Duration::from_secs_f32(1.0 / self.speed);
            return true
        }
        false
    }

    pub fn can_walk(&self, current: Instant) -> bool {
        current >= self.next_walk_time
    }

    /// Energy comes back a point at a time.
    pub fn regenerate(&mut self, current: Instant) {
        let period = Duration::from_secs_f32(1.0 / ENERGY_PER_SECOND);
        while self.next_energy_time <= current {
            if self.energy < self.character().max_energy() {
                self.energy += 1;
            }
            self.next_energy_time += period;
        }
    }

    pub fn cooldown(&self, skill: SkillId, current: Instant) -> Duration {
        match skill.0 {
            1..=SKILL_COUNT => self.skill_ready_time[skill.0 - 1].saturating_duration_since(current),
            _ => Duration::ZERO,
        }
    }

    pub fn can_cast(&self, skill: SkillId, current: Instant) -> bool {
        match skills::spec(skill) {
            Some(spec) => self.energy >= spec.energy && self.cooldown(skill, current).is_zero(),
            None => false,
        }
    }

    /// Pays for a skill: its energy, and the wait until the next one.
    pub fn spend(&mut self, skill: SkillId, current: Instant) {
        if let Some(spec) = skills::spec(skill) {
            self.energy -= spec.energy.min(self.energy);
            self.skill_ready_time[skill.0 - 1] = current + spec.cooldown;
        }
    }

    pub fn storm_hit_due(&mut self, current: Instant, period: Duration) -> bool {
        if current >= self.next_storm_hit {
            self.next_storm_hit = current + period;
            return true
        }
        false
    }
}

fn get_behaviour(name: &'static str) -> Box<dyn EntityBehaviour> {
    match name {
        "" => Box::new(behaviour::None),
        _ => panic!("Entity behaviour '{}' not found", name),
    }
}

pub mod behaviour {
    use super::{EntityBehaviour, EntityAction, Entity, ArenaView};

    use std::time::{Instant};

    pub struct None;
    impl EntityBehaviour for None {
        fn destroyed(&mut self) -> Vec<EntityAction> {
            vec![]
        }

        fn update(&mut self, _time: Instant, _entity: &Entity, _arena: &ArenaView) -> Vec<EntityAction> {
            vec![]
        }
    }
}
