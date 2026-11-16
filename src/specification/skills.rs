use crate::ids::{SkillId};

use std::time::{Duration};

/// What a player can cast, and what it costs.
pub struct SkillSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// The main key, as shown in the game. The number of the skill works too.
    pub key: &'static str,
    pub energy: usize,
    pub cooldown: Duration,
}

pub const FIRE_BALL: SkillId = SkillId(1);
pub const DASH: SkillId = SkillId(2);
pub const NOVA: SkillId = SkillId(3);
pub const WALL: SkillId = SkillId(4);
pub const MINE: SkillId = SkillId(5);

pub const SKILL_COUNT: usize = 5;

pub const SKILL_SPECIFICATIONS: [SkillSpec; SKILL_COUNT] = [
    SkillSpec {
        name: "Fire ball",
        description: "flies where you face",
        key: "Spc",
        energy: 7,
        cooldown: Duration::from_millis(250),
    },
    SkillSpec {
        name: "Dash",
        description: "leaps four cells, past the fire",
        key: "Q",
        energy: 20,
        cooldown: Duration::from_millis(1500),
    },
    SkillSpec {
        name: "Nova",
        description: "eight sparks burst out of you",
        key: "E",
        energy: 30,
        cooldown: Duration::from_millis(2500),
    },
    SkillSpec {
        name: "Wall",
        description: "three cells of stone, 5 seconds",
        key: "R",
        energy: 25,
        cooldown: Duration::from_millis(4000),
    },
    SkillSpec {
        name: "Mine",
        description: "bursts under the next foot",
        key: "F",
        energy: 20,
        cooldown: Duration::from_millis(2000),
    },
];

pub fn spec(id: SkillId) -> Option<&'static SkillSpec> {
    match id.0 {
        1..=SKILL_COUNT => Some(&SKILL_SPECIFICATIONS[id.0 - 1]),
        _ => None,
    }
}

pub fn all() -> impl Iterator<Item = (SkillId, &'static SkillSpec)> {
    SKILL_SPECIFICATIONS.iter().enumerate().map(|(index, spec)| (SkillId(index + 1), spec))
}

/// Energy a character gets back per second.
pub const ENERGY_PER_SECOND: f32 = 15.0;

/// Mines one player can have on the floor at once.
pub const MAX_MINES: usize = 2;

/// Time from dropping a mine until it bursts under someone.
pub const MINE_ARMING: Duration = Duration::from_millis(800);

/// Cells a dash covers at most.
pub const DASH_LENGTH: i32 = 4;
