use super::entity::{Entity};
use super::map::{Map};

use crate::vec2::{Vec2};
use crate::direction::{Direction};
use crate::ids::{EntityId, SpellId, SpellSpecId};
use crate::specification::spells::{self, SpellSpec};

use std::time::{Instant, Duration};
use std::collections::{HashMap, HashSet};
use std::cell::{RefCell, RefMut};

pub trait SpellBehaviour {
    /// Called while an entity shares the cell of the spell. Returns what the spell
    /// does next and whether the entity takes the spell's damage.
    fn entity_collision(
        &mut self,
        time: Instant,
        spell: &Spell,
        entity: &Entity,
    ) -> (Vec<SpellAction>, bool);
    fn destroyed(&mut self, spell: &Spell) -> Vec<SpellAction>;
    fn update(
        &mut self,
        time: Instant,
        spell: &Spell,
        map: &Map,
        entities: &HashMap<EntityId, Entity>,
    ) -> Vec<SpellAction>;
}

/// A new spell another spell gives rise to.
#[derive(Debug, Clone)]
pub struct SpellCast {
    pub spec_id: SpellSpecId,
    pub position: Vec2,
    pub step: Vec2,
}

pub enum SpellAction {
    #[allow(dead_code)] // For spells that speed up or turn, none yet.
    SetSpeed(f32),
    #[allow(dead_code)]
    SetDirection(Direction),
    Move,
    Cast(Vec<SpellCast>),
    Destroy,
}

pub struct Spell {
    id: SpellId,
    spec_id: SpellSpecId,
    entity_origin_id: EntityId,
    behaviour: RefCell<Box<dyn SpellBehaviour>>,
    damage: i32,
    //effects
    position: Vec2,
    step: Vec2,
    speed: f32,
    next_move_time: Instant,
    created: Instant,
    travelled: u32,
    affected_entities: HashSet<EntityId>,
    destroyed: bool,
}

impl Spell {
    pub fn new(
        id: SpellId,
        spec_id: SpellSpecId,
        entity_origin_id: EntityId,
        position: Vec2,
        step: Vec2,
        now: Instant,
    ) -> Spell {
        let spec = spells::spec(spec_id);
        Spell {
            id,
            spec_id,
            entity_origin_id,
            behaviour: RefCell::new(get_behaviour(spec.behaviour_name)),
            damage: spec.damage, /* Mul to entity effects */
            position,
            step,
            speed: spec.speed,
            next_move_time: now + Self::move_period(spec.speed),
            created: now,
            travelled: 0,
            affected_entities: HashSet::new(),
            destroyed: false,
        }
    }

    fn move_period(speed: f32) -> Duration {
        match speed > 0.0 {
            true => Duration::from_secs_f32(1.0 / speed),
            false => Duration::from_secs(3600),
        }
    }

    pub fn id(&self) -> SpellId {
        self.id
    }

    pub fn spec_id(&self) -> SpellSpecId {
        self.spec_id
    }

    pub fn spec(&self) -> &'static SpellSpec {
        spells::spec(self.spec_id)
    }

    pub fn entity_origin_id(&self) -> EntityId {
        self.entity_origin_id
    }

    pub fn damage(&self) -> i32 {
        self.damage
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    pub fn behaviour(&self) -> RefMut<'_, Box<dyn SpellBehaviour>> {
        self.behaviour.borrow_mut()
    }

    pub fn position(&self) -> Vec2 {
        self.position
    }

    /// The cell the spell moves by each time it moves.
    pub fn step(&self) -> Vec2 {
        self.step
    }

    #[allow(dead_code)]
    pub fn speed(&self) -> f32 {
        self.speed
    }

    pub fn created(&self) -> Instant {
        self.created
    }

    pub fn travelled(&self) -> u32 {
        self.travelled
    }

    pub fn is_obstacle(&self) -> bool {
        self.spec().obstacle
    }

    pub fn is_expired(&self, current: Instant) -> bool {
        match self.spec().lifetime {
            Some(lifetime) => current >= self.created + lifetime,
            None => false,
        }
    }

    #[allow(dead_code)]
    pub fn set_position(&mut self, position: Vec2) {
        self.position = position;
    }

    #[allow(dead_code)]
    pub fn displace(&mut self, displacement: Vec2) {
        self.position += displacement;
    }

    pub fn set_direction(&mut self, direction: Direction) {
        self.step = direction.to_vec2();
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed
    }

    /// Whether it is time to move. The move itself is `advance`, once the
    /// arena has checked the next cell is free.
    pub fn is_move_due(&self, current: Instant) -> bool {
        self.speed > 0.0 && current >= self.next_move_time
    }

    pub fn next_position(&self) -> Vec2 {
        self.position + self.step
    }

    pub fn advance(&mut self, current: Instant) {
        self.position += self.step;
        self.travelled += 1;
        let period = Self::move_period(self.speed);
        self.next_move_time += period;
        if self.next_move_time < current {
            self.next_move_time = current + period;
        }
    }

    pub fn add_affected_entity(&mut self, entity_id: EntityId) {
        self.affected_entities.insert(entity_id);
    }

    pub fn is_affected_entity(&self, entity_id: EntityId) -> bool {
        self.affected_entities.contains(&entity_id)
    }

    pub fn has_affected_anyone(&self) -> bool {
        !self.affected_entities.is_empty()
    }

    pub fn destroy(&mut self) {
        self.destroyed = true;
    }
}

//TODO: use std::any::type_name for build the string
fn get_behaviour(name: &'static str) -> Box<dyn SpellBehaviour> {
    match name {
        "Explotable ball" => Box::new(behaviour::ExplotableBall),
        "Spark" => Box::new(behaviour::Spark),
        "Stone wall" => Box::new(behaviour::None),
        "Mine" => Box::new(behaviour::Mine),
        "Fading" => Box::new(behaviour::None),
        "" => Box::new(behaviour::None),
        _ => panic!("Spell behaviour '{}' not found", name),
    }
}

mod behaviour {
    use super::super::entity::{Entity};
    use super::super::map::{Map};

    use super::{SpellBehaviour, SpellAction, SpellCast, Spell};

    use crate::ids::{EntityId};
    use crate::vec2::{Vec2};
    use crate::specification::spells::{BLAST};
    use crate::specification::skills::{MINE_ARMING};

    use std::time::{Instant};
    use std::collections::{HashMap};

    fn blast_at(positions: impl Iterator<Item = Vec2>) -> SpellAction {
        SpellAction::Cast(
            positions.map(|position| SpellCast { spec_id: BLAST, position, step: Vec2::zero() }).collect(),
        )
    }

    /// Lies still and harmless until its time is up (walls, trails, blasts).
    pub struct None;
    impl SpellBehaviour for None {
        fn entity_collision(
            &mut self,
            _time: Instant,
            _spell: &Spell,
            _entity: &Entity,
        ) -> (Vec<SpellAction>, bool) {
            (vec![], false)
        }

        fn destroyed(&mut self, _spell: &Spell) -> Vec<SpellAction> {
            vec![]
        }

        fn update(
            &mut self,
            _time: Instant,
            _spell: &Spell,
            _map: &Map,
            _entities: &HashMap<EntityId, Entity>,
        ) -> Vec<SpellAction> {
            vec![]
        }
    }

    pub struct ExplotableBall;
    impl SpellBehaviour for ExplotableBall {
        fn entity_collision(
            &mut self,
            _time: Instant,
            spell: &Spell,
            entity: &Entity,
        ) -> (Vec<SpellAction>, bool) {
            match entity.id() == spell.entity_origin_id() {
                true => (vec![], false),
                false => (vec![SpellAction::Destroy], true),
            }
        }

        fn destroyed(&mut self, spell: &Spell) -> Vec<SpellAction> {
            vec![blast_at(std::iter::once(spell.position()))]
        }

        fn update(
            &mut self,
            _time: Instant,
            _spell: &Spell,
            _map: &Map,
            _entities: &HashMap<EntityId, Entity>,
        ) -> Vec<SpellAction> {
            vec![SpellAction::Move]
        }
    }

    pub struct Spark;
    impl SpellBehaviour for Spark {
        fn entity_collision(
            &mut self,
            _time: Instant,
            spell: &Spell,
            entity: &Entity,
        ) -> (Vec<SpellAction>, bool) {
            match entity.id() == spell.entity_origin_id() {
                true => (vec![], false),
                false => (vec![SpellAction::Destroy], true),
            }
        }

        fn destroyed(&mut self, _spell: &Spell) -> Vec<SpellAction> {
            vec![]
        }

        fn update(
            &mut self,
            _time: Instant,
            spell: &Spell,
            _map: &Map,
            _entities: &HashMap<EntityId, Entity>,
        ) -> Vec<SpellAction> {
            match spell.spec().range {
                Some(range) if spell.travelled() + 1 >= range => {
                    // The cell it is cast in counts as the first of its range.
                    vec![SpellAction::Destroy]
                }
                _ => vec![SpellAction::Move],
            }
        }
    }

    pub struct Mine;
    impl SpellBehaviour for Mine {
        fn entity_collision(
            &mut self,
            time: Instant,
            spell: &Spell,
            entity: &Entity,
        ) -> (Vec<SpellAction>, bool) {
            let armed = time >= spell.created() + MINE_ARMING;
            match armed && entity.id() != spell.entity_origin_id() {
                true => (vec![SpellAction::Destroy], true),
                false => (vec![], false),
            }
        }

        fn destroyed(&mut self, spell: &Spell) -> Vec<SpellAction> {
            match spell.has_affected_anyone() {
                true => {
                    let center = spell.position();
                    let around = Vec2::AROUND.iter().step_by(2).map(move |step| center + *step);
                    vec![blast_at(std::iter::once(center).chain(around))]
                }
                false => vec![],
            }
        }

        fn update(
            &mut self,
            _time: Instant,
            _spell: &Spell,
            _map: &Map,
            _entities: &HashMap<EntityId, Entity>,
        ) -> Vec<SpellAction> {
            vec![]
        }
    }
}
