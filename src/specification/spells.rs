use crate::ids::{SpellSpecId};

use std::time::{Duration};

/// What a spell is: shared by server (behaviour) and client (drawing).
pub struct SpellSpec {
    pub name: &'static str,
    #[allow(dead_code)] // Documents the spell; nothing shows it yet.
    pub description: &'static str,
    pub damage: i32,
    /// Cells per second, 0 for spells that stay where they are cast.
    pub speed: f32,
    /// Cells the spell travels before it fades, if it fades.
    pub range: Option<u32>,
    /// How long the spell lasts, if it does not last until it hits something.
    pub lifetime: Option<Duration>,
    /// Blocks walking and other spells, like a wall.
    pub obstacle: bool,
    pub behaviour_name: &'static str,
}

pub const FIRE_BALL: SpellSpecId = SpellSpecId(1);
pub const SPARK: SpellSpecId = SpellSpecId(2);
pub const STONE_WALL: SpellSpecId = SpellSpecId(3);
pub const MINE: SpellSpecId = SpellSpecId(4);
pub const TRAIL: SpellSpecId = SpellSpecId(5);
pub const BLAST: SpellSpecId = SpellSpecId(6);

pub const SPELL_SPECIFICATIONS: [SpellSpec; 6] = [
    SpellSpec {
        name: "Fire ball",
        description: "A fire ball that cause burns when explode",
        damage: 10,
        speed: 15.0,
        range: None,
        lifetime: None,
        obstacle: false,
        behaviour_name: "Explotable ball",
    },
    SpellSpec {
        name: "Spark",
        description: "One of the eight sparks a nova throws out",
        damage: 10,
        speed: 18.0,
        range: Some(3),
        lifetime: None,
        obstacle: false,
        behaviour_name: "Spark",
    },
    SpellSpec {
        name: "Stone wall",
        description: "A piece of wall that stops walkers and spells for a while",
        damage: 0,
        speed: 0.0,
        range: None,
        lifetime: Some(Duration::from_secs(5)),
        obstacle: true,
        behaviour_name: "Stone wall",
    },
    SpellSpec {
        name: "Mine",
        description: "Waits on the floor and bursts under the next enemy foot",
        damage: 25,
        speed: 0.0,
        range: None,
        lifetime: Some(Duration::from_secs(25)),
        obstacle: false,
        behaviour_name: "Mine",
    },
    SpellSpec {
        name: "Trail",
        description: "The dust a dash leaves behind",
        damage: 0,
        speed: 0.0,
        range: None,
        lifetime: Some(Duration::from_millis(250)),
        obstacle: false,
        behaviour_name: "Fading",
    },
    SpellSpec {
        name: "Blast",
        description: "Flames of something that just exploded",
        damage: 0,
        speed: 0.0,
        range: None,
        lifetime: Some(Duration::from_millis(220)),
        obstacle: false,
        behaviour_name: "Fading",
    },
];

pub fn spec(id: SpellSpecId) -> &'static SpellSpec {
    &SPELL_SPECIFICATIONS[id.0 - 1]
}
