use crate::vec2::Vec2;
use crate::message::Terrain;

/// The arenas a game can be played in. The Pit is the original open square,
/// the others are drawn below, 26 by 20 cells, the same turned half round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaKind {
    Pit,
    Pillars,
    Crossroads,
    Ruins,
    Rings,
}

impl ArenaKind {
    pub const ALL: [ArenaKind; 5] =
        [ArenaKind::Pit, ArenaKind::Pillars, ArenaKind::Crossroads, ArenaKind::Ruins, ArenaKind::Rings];

    pub fn name(&self) -> &'static str {
        match self {
            ArenaKind::Pit => "The Pit",
            ArenaKind::Pillars => "Pillars",
            ArenaKind::Crossroads => "Crossroads",
            ArenaKind::Ruins => "Ruins",
            ArenaKind::Rings => "Rings",
        }
    }

    /// Accepts the name with or without "The", any case, and "pit" for The Pit.
    pub fn from_name(name: &str) -> Option<ArenaKind> {
        let name = name.trim().to_ascii_lowercase();
        let name = name.strip_prefix("the ").unwrap_or(&name);
        Self::ALL.into_iter().find(|kind| {
            let own = kind.name().to_ascii_lowercase();
            own == name || own.strip_prefix("the ") == Some(name)
        })
    }

    fn layout(&self) -> Option<&'static [&'static str]> {
        match self {
            ArenaKind::Pit => None,
            ArenaKind::Pillars => Some(&PILLARS),
            ArenaKind::Crossroads => Some(&CROSSROADS),
            ArenaKind::Ruins => Some(&RUINS),
            ArenaKind::Rings => Some(&RINGS),
        }
    }
}

/// Which arena each round is played in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArenaChoice {
    /// A different arena every round, in turn.
    Rotate,
    Only(ArenaKind),
}

impl ArenaChoice {
    pub fn name(&self) -> &'static str {
        match self {
            ArenaChoice::Rotate => "Rotate",
            ArenaChoice::Only(kind) => kind.name(),
        }
    }

    pub fn from_name(name: &str) -> Option<ArenaChoice> {
        if name.eq_ignore_ascii_case("rotate") {
            return Some(ArenaChoice::Rotate)
        }
        ArenaKind::from_name(name).map(ArenaChoice::Only)
    }

    pub fn all() -> impl Iterator<Item = ArenaChoice> {
        std::iter::once(ArenaChoice::Rotate).chain(ArenaKind::ALL.into_iter().map(ArenaChoice::Only))
    }
}

pub struct Map {
    name: &'static str,
    width: usize,
    height: usize,
    ground: Vec<Terrain>,
}

impl Map {
    /// The original arena: an open square of `size` cells with a wall around it.
    pub fn new(size: usize) -> Map {
        Map { name: ArenaKind::Pit.name(), width: size, height: size, ground: Self::build_ground(size, 0) }
    }

    pub fn of_kind(kind: ArenaKind, pit_size: usize) -> Map {
        match kind.layout() {
            None => Map::new(pit_size),
            Some(rows) => Map::from_rows(kind.name(), rows),
        }
    }

    /// `#` is wall, anything else floor.
    pub fn from_rows(name: &'static str, rows: &[&str]) -> Map {
        let width = rows[0].len();
        assert!(rows.iter().all(|row| row.len() == width), "arena rows of different lengths");
        let ground = rows
            .iter()
            .flat_map(|row| row.chars())
            .map(|c| if c == '#' { Terrain::Wall } else { Terrain::Floor })
            .collect();
        Map { name, width, height: rows.len(), ground }
    }

    fn build_ground(size: usize, _seed: usize) -> Vec<Terrain> {
        (0..size * size)
            .map(|index| {
                let x = index % size;
                let y = index / size;

                if x == 0 || y == 0 || x == size - 1 || y == size - 1 {
                    Terrain::Wall
                }
                else {
                    Terrain::Floor
                }
            })
            .collect()
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn ground(&self) -> &Vec<Terrain> {
        &self.ground
    }

    pub fn contains(&self, position: Vec2) -> bool {
        position.x >= 0
            && position.y >= 0
            && (position.x as usize) < self.width
            && (position.y as usize) < self.height
    }

    /// Anything outside the map counts as wall.
    pub fn terrain(&self, position: Vec2) -> Terrain {
        match self.contains(position) {
            true => self.ground[position.y as usize * self.width + position.x as usize],
            false => Terrain::Wall,
        }
    }

    pub fn is_floor(&self, position: Vec2) -> bool {
        self.terrain(position) == Terrain::Floor
    }

    pub fn position_of(&self, index: usize) -> Vec2 {
        assert!(index < self.width * self.height);
        Vec2::xy((index % self.width) as i32, (index / self.width) as i32)
    }

    pub fn floor_positions(&self) -> impl Iterator<Item = Vec2> + '_ {
        (0..self.ground.len()).filter(|&i| self.ground[i] == Terrain::Floor).map(|i| self.position_of(i))
    }

    /// How deep into the arena a cell is: 0 for the outer wall, 1 for the
    /// ring of floor along it, and so on. The storm closes in ring by ring.
    pub fn ring(&self, position: Vec2) -> u16 {
        ring_of(position, self.width, self.height)
    }

    /// The innermost ring, the last one the storm reaches.
    pub fn max_ring(&self) -> u16 {
        ((self.width.min(self.height) as u16).saturating_sub(1)) / 2
    }
}

pub fn ring_of(position: Vec2, width: usize, height: usize) -> u16 {
    let x = position.x.min(width as i32 - 1 - position.x);
    let y = position.y.min(height as i32 - 1 - position.y);
    x.min(y).max(0) as u16
}

const PILLARS: [&str; 20] = [
    "##########################",
    "#........................#",
    "#........................#",
    "#........................#",
    "#....##.....##.....##....#",
    "#....##.....##.....##....#",
    "#........................#",
    "#........................#",
    "#........................#",
    "#....##.....##.....##....#",
    "#....##.....##.....##....#",
    "#........................#",
    "#........................#",
    "#........................#",
    "#....##.....##.....##....#",
    "#....##.....##.....##....#",
    "#........................#",
    "#........................#",
    "#........................#",
    "##########################",
];

const CROSSROADS: [&str; 20] = [
    "##########################",
    "#........................#",
    "#........................#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#........................#",
    "#...#####........#####...#",
    "#...#####........#####...#",
    "#........................#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#...........##...........#",
    "#........................#",
    "#........................#",
    "##########################",
];

const RUINS: [&str; 20] = [
    "##########################",
    "#........................#",
    "#........................#",
    "#..###.......#.......#...#",
    "#..#.........#.......#...#",
    "#..#.....#........####...#",
    "#........#...............#",
    "#....#####.......#.......#",
    "#................#..##...#",
    "#..............#.....#...#",
    "#...#.....#..............#",
    "#...##..#................#",
    "#.......#.......#####....#",
    "#...............#........#",
    "#...####........#.....#..#",
    "#...#.......#.........#..#",
    "#...#.......#.......###..#",
    "#........................#",
    "#........................#",
    "##########################",
];

const RINGS: [&str; 20] = [
    "##########################",
    "#........................#",
    "#........................#",
    "#........................#",
    "#...########..########...#",
    "#...#................#...#",
    "#...#................#...#",
    "#...#....#..#####....#...#",
    "#...#...#........#...#...#",
    "#.......#........#.......#",
    "#.......#........#.......#",
    "#...#...#........#...#...#",
    "#...#....#####..#....#...#",
    "#...#................#...#",
    "#...#................#...#",
    "#...########..########...#",
    "#........................#",
    "#........................#",
    "#........................#",
    "##########################",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_arena_is_walled_and_connected() {
        for kind in ArenaKind::ALL {
            let map = Map::of_kind(kind, 20);
            assert!(map.width() <= 26 && map.height() <= 20, "{} too big", kind.name());
            for x in 0..map.width() as i32 {
                assert!(!map.is_floor(Vec2::xy(x, 0)));
                assert!(!map.is_floor(Vec2::xy(x, map.height() as i32 - 1)));
            }
            let floors: Vec<Vec2> = map.floor_positions().collect();
            let mut seen = vec![floors[0]];
            let mut stack = vec![floors[0]];
            while let Some(cell) = stack.pop() {
                for step in [Vec2::x(1), Vec2::x(-1), Vec2::y(1), Vec2::y(-1)] {
                    let next = cell + step;
                    if map.is_floor(next) && !seen.contains(&next) {
                        seen.push(next);
                        stack.push(next);
                    }
                }
            }
            assert_eq!(seen.len(), floors.len(), "{} has closed rooms", kind.name());
        }
    }

    #[test]
    fn names_and_rings() {
        assert_eq!(ArenaKind::from_name("pit"), Some(ArenaKind::Pit));
        assert_eq!(ArenaChoice::from_name("RUINS"), Some(ArenaChoice::Only(ArenaKind::Ruins)));
        assert_eq!(ArenaChoice::from_name("rotate"), Some(ArenaChoice::Rotate));
        let map = Map::of_kind(ArenaKind::Pillars, 20);
        assert_eq!(map.ring(Vec2::xy(0, 5)), 0);
        assert_eq!(map.ring(Vec2::xy(1, 1)), 1);
        assert_eq!(map.ring(Vec2::xy(12, 9)), 9);
        assert_eq!(map.max_ring(), 9);
        assert_eq!(map.terrain(Vec2::xy(-3, 40)), Terrain::Wall);
    }
}
