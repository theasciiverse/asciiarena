pub mod entity;
pub mod map;
pub mod spell;

use map::{Map};
use entity::{Entity, EntityAction};
use spell::{Spell, SpellAction, SpellCast};

use crate::character::{Character};
use crate::ids::{SpellId, EntityId, SpellSpecId, SkillId};
use crate::vec2::Vec2;
use crate::direction::{Direction};
use crate::message::{Cause};
use crate::specification::{skills, spells};

use std::collections::{HashMap, HashSet, VecDeque};

use std::time::{Instant, Duration};
use std::rc::{Rc};

/// After the storm starts, it covers one more ring of the arena this often.
pub const STORM_RING_PERIOD: Duration = Duration::from_millis(2500);
/// Anyone in the storm is hurt this often...
pub const STORM_HIT_PERIOD: Duration = Duration::from_millis(500);
/// ...by this much.
pub const STORM_DAMAGE: i32 = 4;

/// What entity behaviours (players and bots) can see of the arena.
pub struct ArenaView<'a> {
    pub map: &'a Map,
    pub entities: &'a HashMap<EntityId, Entity>,
    pub spells: &'a HashMap<SpellId, Spell>,
    pub storm: u16,
}

impl ArenaView<'_> {
    pub fn in_storm(&self, position: Vec2) -> bool {
        let ring = self.map.ring(position);
        self.storm > 0 && ring >= 1 && ring <= self.storm
    }

    pub fn has_obstacle(&self, position: Vec2) -> bool {
        self.spells.values().any(|spell| spell.is_obstacle() && spell.position() == position)
    }

    pub fn is_walkable(&self, position: Vec2) -> bool {
        self.map.is_floor(position)
            && !self.has_obstacle(position)
            && !self.entities.values().any(|entity| entity.position() == position)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArenaEvent {
    Killed { victim: EntityId, killer: Option<EntityId>, cause: Cause },
}

pub struct Arena {
    map: Map,
    entities: HashMap<EntityId, Entity>,
    spells: HashMap<SpellId, Spell>,
    last_entity_id: EntityId,
    last_spell_id: SpellId,
    started: Instant,
    storm_after: Duration,
    storm: u16,
    events: Vec<ArenaEvent>,
}

impl Arena {
    pub fn new(map: Map, now: Instant, storm_after: Duration) -> Arena {
        Arena {
            map,
            entities: HashMap::new(),
            spells: HashMap::new(),
            last_entity_id: EntityId::NONE,
            last_spell_id: SpellId::NONE,
            started: now,
            storm_after,
            storm: 0,
            events: Vec::new(),
        }
    }

    pub fn map(&self) -> &Map {
        &self.map
    }

    pub fn entities(&self) -> &HashMap<EntityId, Entity> {
        &self.entities
    }

    pub fn spells(&self) -> &HashMap<SpellId, Spell> {
        &self.spells
    }

    pub fn storm(&self) -> u16 {
        self.storm
    }

    pub fn storm_after(&self) -> Duration {
        self.storm_after
    }

    pub fn view(&self) -> ArenaView<'_> {
        ArenaView { map: &self.map, entities: &self.entities, spells: &self.spells, storm: self.storm }
    }

    pub fn take_events(&mut self) -> Vec<ArenaEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn create_entity(
        &mut self,
        character: Rc<Character>,
        position: Vec2,
        now: Instant,
    ) -> &mut Entity {
        let id = EntityId::next(self.last_entity_id);
        let entity = Entity::new(id, character, position, now);
        self.last_entity_id = id;
        self.entities.insert(id, entity);
        self.entities.get_mut(&id).unwrap()
    }

    #[cfg(test)]
    pub fn entity_mut(&mut self, id: EntityId) -> Option<&mut Entity> {
        self.entities.get_mut(&id)
    }

    fn blocks_spells(&self, position: Vec2) -> bool {
        !self.map.is_floor(position) || self.view().has_obstacle(position)
    }

    /// Creates a spell where there is room for one: never inside a wall.
    pub fn create_spell(
        &mut self,
        spec_id: SpellSpecId,
        entity_id: EntityId,
        position: Vec2,
        step: Vec2,
        now: Instant,
    ) -> Option<SpellId> {
        if self.blocks_spells(position) {
            return None
        }
        let id = SpellId::next(self.last_spell_id);
        let spell = Spell::new(id, spec_id, entity_id, position, step, now);
        self.last_spell_id = id;
        self.spells.insert(id, spell);
        Some(id)
    }

    fn sorted_entity_ids(&self) -> Vec<EntityId> {
        let mut ids = self.entities.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| id.0);
        ids
    }

    fn sorted_spell_ids(&self) -> Vec<SpellId> {
        let mut ids = self.spells.keys().copied().collect::<Vec<_>>();
        ids.sort_by_key(|id| id.0);
        ids
    }

    pub fn update(&mut self, now: Instant) {
        assert!(self.spells.iter().all(|(_, spell)| !spell.is_destroyed()));
        assert!(self.entities.iter().all(|(_, entity)| entity.is_alive()));

        self.update_storm(now);

        for entity in self.entities.values_mut() {
            entity.regenerate(now);
        }

        for entity_id in self.sorted_entity_ids() {
            let entity = &self.entities[&entity_id];
            let view = self.view();
            let mut entity_actions = VecDeque::from(entity.behaviour().update(now, entity, &view));

            while let Some(action) = entity_actions.pop_front() {
                match action {
                    EntityAction::Walk(direction) => {
                        let entity = &self.entities[&entity_id];
                        let next_position = entity.position() + direction.to_vec2();
                        let walkable = self.view().is_walkable(next_position);
                        let entity = self.entities.get_mut(&entity_id).unwrap();
                        entity.set_direction(direction);
                        if walkable {
                            entity.walk(now);
                        }
                    }
                    EntityAction::Cast(direction, skill) => {
                        let entity = self.entities.get_mut(&entity_id).unwrap();
                        entity.set_direction(direction);
                        let cast = self.cast_skill(entity_id, skill, now);
                        log::trace!("{:?} casts {:?}: {}", entity_id, skill, cast);
                    }
                    EntityAction::Destroy => {
                        let entity = self.entities.get_mut(&entity_id).unwrap();
                        entity.set_health(0);
                    }
                }
            }
        }

        // A walker that steps onto a fire ball is hit as surely as one the ball flies into.
        self.collide(now);

        let mut casts = Vec::new();
        for spell_id in self.sorted_spell_ids() {
            let map = &self.map;
            let entities = &self.entities;
            let obstacles = self
                .spells
                .values()
                .filter(|spell| spell.is_obstacle())
                .map(|spell| spell.position())
                .collect::<HashSet<_>>();
            let spell = self.spells.get_mut(&spell_id).unwrap();

            let mut spell_actions = {
                let spell: &Spell = spell;
                match spell.is_expired(now) {
                    true => VecDeque::from(vec![SpellAction::Destroy]),
                    false => VecDeque::from(spell.behaviour().update(now, spell, map, entities)),
                }
            };

            while let Some(action) = spell_actions.pop_front() {
                match action {
                    SpellAction::Move => {
                        if spell.is_move_due(now) {
                            let next = spell.next_position();
                            if map.is_floor(next) && !obstacles.contains(&next) {
                                spell.advance(now);
                            }
                            else {
                                spell_actions.push_back(SpellAction::Destroy);
                            }
                        }
                    }
                    SpellAction::SetSpeed(speed) => spell.set_speed(speed),
                    SpellAction::SetDirection(direction) => spell.set_direction(direction),
                    SpellAction::Cast(new_spells) => {
                        casts.extend(new_spells.into_iter().map(|cast| (spell.entity_origin_id(), cast)))
                    }
                    SpellAction::Destroy => {
                        if !spell.is_destroyed() {
                            spell.destroy();
                            let spell: &Spell = spell;
                            let actions = spell.behaviour().destroyed(spell);
                            spell_actions.extend(actions);
                        }
                    }
                }
            }
        }
        self.create_casts(casts, now);
        self.spells.retain(|_, spell| !spell.is_destroyed());

        self.collide(now);

        let view_storm = self.storm;
        for entity in self.entities.values_mut() {
            let ring = self.map.ring(entity.position());
            if view_storm > 0 && ring >= 1 && ring <= view_storm && entity.storm_hit_due(now, STORM_HIT_PERIOD)
            {
                entity.hurt(STORM_DAMAGE, None, Cause::Storm);
            }
        }

        for entity_id in self.sorted_entity_ids() {
            let entity = &self.entities[&entity_id];
            if !entity.is_alive() {
                let (killer, cause) = entity.last_hit().unwrap_or((None, Cause::Storm));
                entity.behaviour().destroyed();
                self.events.push(ArenaEvent::Killed { victim: entity_id, killer, cause });
            }
        }

        self.spells.retain(|_, spell| !spell.is_destroyed());
        self.entities.retain(|_, entity| entity.is_alive());
    }

    fn update_storm(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed >= self.storm_after {
            let rings = 1 + ((elapsed - self.storm_after).as_millis() / STORM_RING_PERIOD.as_millis()) as u16;
            let storm = rings.min(self.map.max_ring());
            if storm != self.storm {
                log::debug!("Storm covers {} rings", storm);
                self.storm = storm;
            }
        }
    }

    /// Every spell touching an entity: the spell decides what happens.
    fn collide(&mut self, now: Instant) {
        let mut casts = Vec::new();
        for spell_id in self.sorted_spell_ids() {
            let position = self.spells[&spell_id].position();
            let mut targets = self
                .entities
                .values()
                .filter(|entity| entity.position() == position && entity.is_alive())
                .map(|entity| entity.id())
                .collect::<Vec<_>>();
            targets.sort_by_key(|id| id.0);

            for entity_id in targets {
                let spell = self.spells.get_mut(&spell_id).unwrap();
                if spell.is_destroyed() || spell.is_affected_entity(entity_id) {
                    continue
                }
                let entity = self.entities.get_mut(&entity_id).unwrap();
                let (actions, affect) = {
                    let spell: &Spell = spell;
                    let entity: &Entity = entity;
                    spell.behaviour().entity_collision(now, spell, entity)
                };
                if affect {
                    entity.hurt(spell.damage(), Some(spell.entity_origin_id()), Cause::Spell(spell.spec_id()));
                    spell.add_affected_entity(entity_id);
                }

                let mut actions = VecDeque::from(actions);
                while let Some(action) = actions.pop_front() {
                    match action {
                        SpellAction::Destroy => {
                            if !spell.is_destroyed() {
                                spell.destroy();
                                let spell: &Spell = spell;
                                actions.extend(spell.behaviour().destroyed(spell));
                            }
                        }
                        SpellAction::Cast(new_spells) => casts
                            .extend(new_spells.into_iter().map(|cast| (spell.entity_origin_id(), cast))),
                        SpellAction::SetSpeed(speed) => spell.set_speed(speed),
                        SpellAction::SetDirection(direction) => spell.set_direction(direction),
                        SpellAction::Move => (),
                    }
                }
            }
        }
        self.create_casts(casts, now);
        self.spells.retain(|_, spell| !spell.is_destroyed());
    }

    fn create_casts(&mut self, casts: Vec<(EntityId, SpellCast)>, now: Instant) {
        for (origin, cast) in casts {
            self.create_spell(cast.spec_id, origin, cast.position, cast.step, now);
        }
    }

    /// Casts a skill if the entity can pay for it and there is room for it.
    pub fn cast_skill(&mut self, entity_id: EntityId, skill: SkillId, now: Instant) -> bool {
        let entity = match self.entities.get(&entity_id) {
            Some(entity) if entity.can_cast(skill, now) => entity,
            _ => return false,
        };
        let position = entity.position();
        let direction = entity.direction();

        let done = match skill {
            skills::FIRE_BALL => self
                .create_spell(spells::FIRE_BALL, entity_id, position + direction.to_vec2(), direction.to_vec2(), now)
                .is_some(),
            skills::DASH => self.dash(entity_id, position, direction, now),
            skills::NOVA => {
                let sparks = Vec2::AROUND
                    .iter()
                    .filter_map(|step| self.create_spell(spells::SPARK, entity_id, position + *step, *step, now))
                    .count();
                sparks > 0
            }
            skills::WALL => {
                let center = position + direction.to_vec2() * 2;
                let [left, right] = direction.perpendiculars();
                let cells = [center, center + left.to_vec2(), center + right.to_vec2()];
                let free = cells.into_iter().filter(|cell| self.view().is_walkable(*cell)).collect::<Vec<_>>();
                let stones = free
                    .into_iter()
                    .filter_map(|cell| self.create_spell(spells::STONE_WALL, entity_id, cell, Vec2::zero(), now))
                    .count();
                stones > 0
            }
            skills::MINE => {
                let mine_here = self.spells.values().any(|spell| {
                    spell.spec_id() == spells::MINE && spell.position() == position
                });
                if mine_here {
                    false
                }
                else {
                    let mut own_mines = self
                        .spells
                        .values()
                        .filter(|spell| spell.spec_id() == spells::MINE && spell.entity_origin_id() == entity_id)
                        .map(|spell| (spell.created(), spell.id()))
                        .collect::<Vec<_>>();
                    own_mines.sort();
                    while own_mines.len() >= skills::MAX_MINES {
                        let (_, oldest) = own_mines.remove(0);
                        self.spells.remove(&oldest);
                    }
                    self.create_spell(spells::MINE, entity_id, position, Vec2::zero(), now).is_some()
                }
            }
            _ => false,
        };

        if done {
            self.entities.get_mut(&entity_id).unwrap().spend(skill, now);
        }
        done
    }

    fn dash(&mut self, entity_id: EntityId, from: Vec2, direction: Direction, now: Instant) -> bool {
        let step = direction.to_vec2();
        let mut to = from;
        for _ in 0..skills::DASH_LENGTH {
            if !self.view().is_walkable(to + step) {
                break
            }
            to += step;
        }
        if to == from {
            return false
        }
        let mut dust = from;
        while dust != to {
            self.create_spell(spells::TRAIL, entity_id, dust, Vec2::zero(), now);
            dust += step;
        }
        self.entities.get_mut(&entity_id).unwrap().set_position(to);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::map::{ArenaKind};
    use super::entity::{EntityBehaviour};
    use crate::character::{CharacterBuilder, CharacterId};

    use std::cell::{RefCell};

    /// Does what the test tells it, once.
    struct Puppet(Rc<RefCell<Vec<EntityAction>>>);
    impl EntityBehaviour for Puppet {
        fn destroyed(&mut self) -> Vec<EntityAction> {
            vec![]
        }
        fn update(&mut self, _: Instant, _: &Entity, _: &ArenaView) -> Vec<EntityAction> {
            self.0.borrow_mut().drain(..).collect()
        }
    }

    fn character(symbol: char) -> Rc<Character> {
        Rc::new(
            CharacterBuilder::default()
                .id(CharacterId::Player(symbol))
                .symbol(symbol)
                .max_health(100)
                .max_energy(100)
                .speed_base(8.0)
                .build()
                .unwrap(),
        )
    }

    struct Setup {
        arena: Arena,
        now: Instant,
        orders: Vec<Rc<RefCell<Vec<EntityAction>>>>,
        ids: Vec<EntityId>,
    }

    impl Setup {
        fn new(kind: ArenaKind, positions: &[(i32, i32)]) -> Setup {
            let now = Instant::now();
            let mut arena = Arena::new(Map::of_kind(kind, 20), now, Duration::from_secs(600));
            let mut orders = Vec::new();
            let mut ids = Vec::new();
            for (index, (x, y)) in positions.iter().enumerate() {
                let order = Rc::new(RefCell::new(Vec::new()));
                let entity = arena.create_entity(character((b'A' + index as u8) as char), Vec2::xy(*x, *y), now);
                entity.set_behaviour(Box::new(Puppet(order.clone())));
                ids.push(entity.id());
                orders.push(order);
            }
            Setup { arena, now, orders, ids }
        }

        fn order(&self, who: usize, action: EntityAction) {
            self.orders[who].borrow_mut().push(action);
        }

        fn run(&mut self, millis: u64) {
            let end = self.now + Duration::from_millis(millis);
            while self.now < end {
                self.now += Duration::from_millis(33);
                self.arena.update(self.now);
            }
        }

        fn entity(&self, who: usize) -> Option<&Entity> {
            self.arena.entities().get(&self.ids[who])
        }
    }

    #[test]
    fn fire_ball_flies_and_hurts() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(2, 5), (10, 5)]);
        setup.order(0, EntityAction::Cast(Direction::Right, skills::FIRE_BALL));
        setup.run(1000);
        assert_eq!(setup.entity(1).unwrap().health(), 90);
        assert_eq!(setup.entity(0).unwrap().health(), 100);
        assert!(setup.arena.spells().values().all(|spell| spell.spec_id() != spells::FIRE_BALL));
        assert!(setup.entity(0).unwrap().energy() < 100 || setup.entity(0).unwrap().energy() == 100);
    }

    #[test]
    fn walking_into_a_fire_ball_still_hurts() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(2, 5), (5, 5)]);
        setup.order(0, EntityAction::Cast(Direction::Right, skills::FIRE_BALL));
        setup.order(1, EntityAction::Walk(Direction::Left));
        setup.run(200);
        setup.order(1, EntityAction::Walk(Direction::Left));
        setup.run(600);
        assert_eq!(setup.entity(1).unwrap().health(), 90);
    }

    #[test]
    fn casting_into_the_outer_wall_does_nothing() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(1, 1)]);
        setup.order(0, EntityAction::Cast(Direction::Left, skills::FIRE_BALL));
        setup.run(300);
        assert!(setup.arena.spells().is_empty());
        assert_eq!(setup.entity(0).unwrap().energy(), 100);
    }

    #[test]
    fn cooldown_and_energy_limit_casting() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(2, 5)]);
        let id = setup.ids[0];
        let now = setup.now;
        assert!(setup.arena.cast_skill(id, skills::NOVA, now));
        assert!(!setup.arena.cast_skill(id, skills::NOVA, now + Duration::from_millis(100)));
        assert_eq!(setup.entity(0).unwrap().energy(), 70);
        let later = now + skills::spec(skills::NOVA).unwrap().cooldown;
        assert!(setup.arena.cast_skill(id, skills::NOVA, later));
        setup.arena.entity_mut(id).unwrap().set_energy(5);
        assert!(!setup.arena.cast_skill(id, skills::FIRE_BALL, later + Duration::from_secs(9)));
    }

    #[test]
    fn nova_hits_all_around() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(10, 10), (12, 10), (8, 8), (10, 13), (10, 16)]);
        setup.order(0, EntityAction::Cast(Direction::Up, skills::NOVA));
        setup.run(700);
        assert_eq!(setup.entity(1).unwrap().health(), 90);
        assert_eq!(setup.entity(2).unwrap().health(), 90);
        assert_eq!(setup.entity(3).unwrap().health(), 90);
        assert_eq!(setup.entity(4).unwrap().health(), 100, "out of range");
        assert_eq!(setup.entity(0).unwrap().health(), 100, "never the caster");
    }

    #[test]
    fn stone_wall_stops_fire_and_feet_then_crumbles() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(10, 5), (10, 12), (3, 7)]);
        setup.order(1, EntityAction::Cast(Direction::Up, skills::WALL));
        setup.run(100);
        let stones = setup.arena.spells().values().filter(|s| s.spec_id() == spells::STONE_WALL).count();
        assert_eq!(stones, 3);
        setup.order(0, EntityAction::Cast(Direction::Down, skills::FIRE_BALL));
        setup.run(1000);
        assert_eq!(setup.entity(1).unwrap().health(), 100);
        setup.run(5000);
        assert!(setup.arena.spells().values().all(|s| s.spec_id() != spells::STONE_WALL));
    }

    #[test]
    fn dash_stops_at_walls_and_leaves_dust() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(3, 5)]);
        setup.order(0, EntityAction::Cast(Direction::Left, skills::DASH));
        setup.run(50);
        assert_eq!(setup.entity(0).unwrap().position(), Vec2::xy(1, 5));
        assert!(setup.arena.spells().values().any(|s| s.spec_id() == spells::TRAIL));
        let id = setup.ids[0];
        let later = setup.now + Duration::from_secs(5);
        setup.arena.entity_mut(id).unwrap().set_direction(Direction::Right);
        assert!(setup.arena.cast_skill(id, skills::DASH, later));
        assert_eq!(setup.entity(0).unwrap().position(), Vec2::xy(5, 5));
    }

    #[test]
    fn mine_waits_for_an_enemy() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(5, 5), (7, 5)]);
        setup.order(0, EntityAction::Cast(Direction::Left, skills::MINE));
        setup.run(100);
        setup.order(0, EntityAction::Walk(Direction::Up));
        setup.run(1000);
        assert_eq!(setup.entity(0).unwrap().health(), 100, "the owner is safe");
        setup.order(1, EntityAction::Walk(Direction::Left));
        setup.run(200);
        setup.order(1, EntityAction::Walk(Direction::Left));
        setup.run(200);
        assert_eq!(setup.entity(1).unwrap().health(), 75);
        assert!(setup.arena.spells().values().all(|s| s.spec_id() != spells::MINE));
    }

    #[test]
    fn storm_closes_in_and_kills() {
        let now = Instant::now();
        let mut arena = Arena::new(Map::of_kind(ArenaKind::Pit, 20), now, Duration::from_secs(1));
        let id = arena.create_entity(character('A'), Vec2::xy(1, 1), now).id();
        let mut time = now;
        while arena.entities().contains_key(&id) && time < now + Duration::from_secs(60) {
            time += Duration::from_millis(33);
            arena.update(time);
        }
        assert!(!arena.entities().contains_key(&id));
        assert!(arena.storm() >= 1);
        assert_eq!(
            arena.take_events(),
            vec![ArenaEvent::Killed { victim: id, killer: None, cause: Cause::Storm }]
        );
    }

    #[test]
    fn kills_are_credited() {
        let mut setup = Setup::new(ArenaKind::Pit, &[(2, 5), (10, 5)]);
        let victim = setup.ids[1];
        setup.arena.entity_mut(victim).unwrap().set_health(5);
        setup.order(0, EntityAction::Cast(Direction::Right, skills::FIRE_BALL));
        setup.run(1000);
        assert!(setup.entity(1).is_none());
        assert_eq!(
            setup.arena.take_events(),
            vec![ArenaEvent::Killed {
                victim,
                killer: Some(setup.ids[0]),
                cause: Cause::Spell(spells::FIRE_BALL)
            }]
        );
    }
}
