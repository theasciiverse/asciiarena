use crate::vec2::Vec2;

use serde::{Serialize, Deserialize};

use rand::{
    distr::{Distribution, StandardUniform},
    Rng,
};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Direction; 4] = [Direction::Up, Direction::Right, Direction::Down, Direction::Left];

    /// The direction of a straight line from the origin to `vec`, if there is one.
    pub fn from_vec2(vec: Vec2) -> Option<Direction> {
        match (vec.x.signum(), vec.y.signum()) {
            (0, -1) => Some(Direction::Up),
            (1, 0) => Some(Direction::Right),
            (0, 1) => Some(Direction::Down),
            (-1, 0) => Some(Direction::Left),
            _ => None,
        }
    }

    /// The two directions at a right angle to this one.
    pub fn perpendiculars(&self) -> [Direction; 2] {
        match *self {
            Direction::Up | Direction::Down => [Direction::Left, Direction::Right],
            Direction::Left | Direction::Right => [Direction::Up, Direction::Down],
        }
    }

    pub fn to_vec2(&self) -> Vec2 {
        match *self {
            Direction::Up => Vec2::y(-1),
            Direction::Right => Vec2::x(1),
            Direction::Down => Vec2::y(1),
            Direction::Left => Vec2::x(-1),
        }
    }

    pub fn opposite(&self) -> Direction {
        match *self {
            Direction::Up => Direction::Down,
            Direction::Right => Direction::Left,
            Direction::Down => Direction::Up,
            Direction::Left => Direction::Right,
        }
    }

    pub fn turn_left(&mut self) {
        match *self {
            Direction::Up => *self = Direction::Right,
            Direction::Right => *self = Direction::Down,
            Direction::Down => *self = Direction::Left,
            Direction::Left => *self = Direction::Up,
        }
    }

    pub fn turn_right(&mut self) {
        match *self {
            Direction::Up => *self = Direction::Left,
            Direction::Right => *self = Direction::Up,
            Direction::Down => *self = Direction::Right,
            Direction::Left => *self = Direction::Down,
        }
    }
}

impl Distribution<Direction> for StandardUniform {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Direction {
        match rng.random_range(0..4) {
            0 => Direction::Up,
            1 => Direction::Right,
            2 => Direction::Down,
            3 => Direction::Left,
            _ => unreachable!(),
        }
    }
}
