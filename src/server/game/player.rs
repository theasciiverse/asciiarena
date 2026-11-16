use super::arena::entity::{EntityAction, EntityBehaviour, Entity};
use super::arena::{ArenaView};
use super::bot::{BotBrain};

use crate::character::{Character};
use crate::direction::{Direction};
use crate::ids::{SkillId, EntityId};
use crate::message::{BotLevel};

use std::rc::{Rc};
use std::time::{Instant};
use std::cell::{RefCell};

pub struct Player {
    character: Rc<Character>,
    entity_handler: Rc<RefCell<EntityHandler>>,
    points: usize,
    kills: usize,
    rounds_won: usize,
    /// Played by the computer at this level.
    bot: Option<BotLevel>,
    /// Joined from a client, even when the computer plays for them (`--demo`).
    human: bool,
}

impl Player {
    pub const MAX_LIFE: usize = 100;
    pub const MAX_ENERGY: usize = 100;
    pub const SPEED_BASE: f32 = 8.0;

    pub fn new(character: Rc<Character>, bot: Option<BotLevel>, human: bool) -> Player {
        Player {
            character,
            entity_handler: Rc::new(RefCell::new(EntityHandler::default())),
            points: 0,
            kills: 0,
            rounds_won: 0,
            bot,
            human,
        }
    }

    pub fn character(&self) -> &Rc<Character> {
        &self.character
    }

    pub fn entity_id(&self) -> EntityId {
        self.entity_handler.borrow().entity_id
    }

    pub fn points(&self) -> usize {
        self.points
    }

    pub fn kills(&self) -> usize {
        self.kills
    }

    pub fn rounds_won(&self) -> usize {
        self.rounds_won
    }

    pub fn bot(&self) -> Option<BotLevel> {
        self.bot
    }

    pub fn is_human(&self) -> bool {
        self.human
    }

    pub fn is_alive(&self) -> bool {
        self.entity_handler.borrow().entity_id != EntityId::NONE
    }

    pub fn walk(&mut self, direction: Direction) {
        self.entity_handler.borrow_mut().actions.push(EntityAction::Walk(direction))
    }

    pub fn cast(&mut self, direction: Direction, id: SkillId) {
        self.entity_handler.borrow_mut().actions.push(EntityAction::Cast(direction, id))
    }

    pub fn add_points(&mut self, points: usize) {
        self.points += points;
    }

    pub fn add_kill(&mut self) {
        self.kills += 1;
    }

    pub fn add_round_won(&mut self) {
        self.rounds_won += 1;
    }

    pub fn create_entity_behaviour(
        &mut self,
        entity_id: EntityId,
        seed: u64,
        now: Instant,
    ) -> Box<dyn EntityBehaviour> {
        {
            let mut handler = self.entity_handler.borrow_mut();
            handler.entity_id = entity_id;
            handler.actions.clear();
        }
        let entity_handler = self.entity_handler.clone();
        match self.bot {
            Some(level) => Box::new(BotBehaviour { entity_handler, brain: BotBrain::new(level, seed, now) }),
            None => Box::new(PlayerBehaviour { entity_handler }),
        }
    }
}

#[derive(Default)]
pub struct EntityHandler {
    entity_id: EntityId,
    actions: Vec<EntityAction>,
}

pub struct PlayerBehaviour {
    entity_handler: Rc<RefCell<EntityHandler>>,
}

impl EntityBehaviour for PlayerBehaviour {
    fn destroyed(&mut self) -> Vec<EntityAction> {
        self.entity_handler.borrow_mut().entity_id = EntityId::NONE;
        vec![]
    }

    fn update(&mut self, _time: Instant, _entity: &Entity, _arena: &ArenaView) -> Vec<EntityAction> {
        let actions = &mut self.entity_handler.borrow_mut().actions;
        let returned = actions.clone();
        actions.clear();
        returned
    }
}

/// A player the computer plays: keys from a client, if any, are ignored.
pub struct BotBehaviour {
    entity_handler: Rc<RefCell<EntityHandler>>,
    brain: BotBrain,
}

impl EntityBehaviour for BotBehaviour {
    fn destroyed(&mut self) -> Vec<EntityAction> {
        self.entity_handler.borrow_mut().entity_id = EntityId::NONE;
        vec![]
    }

    fn update(&mut self, time: Instant, entity: &Entity, arena: &ArenaView) -> Vec<EntityAction> {
        self.entity_handler.borrow_mut().actions.clear();
        self.brain.think(time, entity, arena)
    }
}
