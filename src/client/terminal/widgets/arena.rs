use super::util::{self};

use crate::client::state::{State, GameStatus, Player, Session, FeedItem};
use crate::client::store::{Store, Action};
use crate::client::terminal::input::{InputEvent};
use crate::client::configuration::{Config};

use crate::direction::{Direction};
use crate::character::{CharacterId, Character};
use crate::message::{EntityData, Terrain, Cause};
use crate::ids::{EntityId};
use crate::specification::{skills, spells};
use crate::vec2::{Vec2};

use ratatui::buffer::{Buffer};
use ratatui::widgets::{Paragraph, Block, Borders, BorderType, Widget, Clear};
use ratatui::layout::{Layout, Constraint, Direction as Dir, Rect, Alignment, Margin};
use ratatui::style::{Style, Modifier, Color};
use ratatui::text::{Span, Line};

use crossterm::event::{KeyCode};

use std::time::{Instant, Duration};
use std::collections::{HashMap};

/// One colour per letter, in the order of the panels.
const PALETTE: [Color; 8] = [
    Color::Indexed(117),
    Color::Indexed(221),
    Color::Indexed(213),
    Color::Indexed(120),
    Color::Indexed(141),
    Color::Indexed(209),
    Color::Indexed(87),
    Color::Indexed(230),
];

/// The arena drawn before the first one arrives: the size of the largest.
const DEFAULT_DIMENSION: (usize, usize) = (26, 20);

const FEED_TIME: Duration = Duration::from_secs(5);
const REMAINS_TIME: Duration = Duration::from_millis(2500);
const DEMO_REMATCH: Duration = Duration::from_secs(6);

pub fn player_color(index: usize) -> Color {
    PALETTE[index % PALETTE.len()]
}

fn color_of(state: &State, symbol: char) -> Color {
    let game = &state.server.game;
    game.players
        .iter()
        .find(|player| player.character_id == CharacterId::Player(symbol))
        .map(|player| player_color(player.id))
        .unwrap_or(Color::White)
}

fn letter(state: &State, symbol: char) -> Span<'static> {
    Span::styled(symbol.to_string(), Style::default().fg(color_of(state, symbol)).add_modifier(Modifier::BOLD))
}

pub struct Arena {
    previous_entities: HashMap<EntityId, EntityData>,
    damaged_entities: HashMap<EntityId, Instant>,
    remains: Vec<(Vec2, CharacterId, Instant)>,
    arena_number: usize,
    confirm_leave: bool,
    finished_at: Option<Instant>,
}

impl Arena {
    pub fn new(_config: &Config) -> Arena {
        Arena {
            previous_entities: HashMap::new(),
            damaged_entities: HashMap::new(),
            remains: Vec::new(),
            arena_number: 0,
            confirm_leave: false,
            finished_at: None,
        }
    }

    pub fn process_event(&mut self, store: &mut Store, event: InputEvent) {
        let session = store.state().server.game.status;
        let local = store.state().session.as_ref().map(|s| s.is_local()).unwrap_or(false);
        let demo = store.state().session == Some(Session::Demo);
        let key_event = match event {
            InputEvent::KeyPressed(key_event) => key_event,
            InputEvent::ResizeDisplay => return,
        };

        if self.confirm_leave {
            self.confirm_leave = false;
            if matches!(key_event.code, KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter) {
                log::info!("Leaving the match");
                store.dispatch(Action::LeaveGame);
            }
            return
        }

        match session {
            GameStatus::Finished => match key_event.code {
                KeyCode::Enter | KeyCode::Char(' ') => match local {
                    true => store.dispatch(Action::Rematch),
                    false => store.dispatch(Action::CloseGame),
                },
                KeyCode::Esc => match demo {
                    true => store.dispatch(Action::CloseApp),
                    false => store.dispatch(Action::LeaveGame),
                },
                _ => (),
            },
            GameStatus::Started | GameStatus::NotStarted => {
                if key_event.code == KeyCode::Esc {
                    match demo {
                        true => store.dispatch(Action::CloseApp),
                        false => self.confirm_leave = true,
                    }
                    return
                }
                if demo {
                    return
                }
                let action = match key_event.code {
                    KeyCode::Up => Some(Action::MovePlayer(Direction::Up)),
                    KeyCode::Left => Some(Action::MovePlayer(Direction::Left)),
                    KeyCode::Down => Some(Action::MovePlayer(Direction::Down)),
                    KeyCode::Right => Some(Action::MovePlayer(Direction::Right)),
                    KeyCode::Char(c) => match c.to_ascii_lowercase() {
                        'w' => Some(Action::MovePlayer(Direction::Up)),
                        'a' => Some(Action::MovePlayer(Direction::Left)),
                        's' => Some(Action::MovePlayer(Direction::Down)),
                        'd' => Some(Action::MovePlayer(Direction::Right)),
                        ' ' | '1' => Some(Action::CastSkill(skills::FIRE_BALL)),
                        'q' | '2' => Some(Action::CastSkill(skills::DASH)),
                        'e' | '3' => Some(Action::CastSkill(skills::NOVA)),
                        'r' | '4' => Some(Action::CastSkill(skills::WALL)),
                        'f' | '5' => Some(Action::CastSkill(skills::MINE)),
                        _ => None,
                    },
                    _ => None,
                };
                if let Some(action) = action {
                    if store.state().server.game.status == GameStatus::Started {
                        store.dispatch(action);
                    }
                }
            }
        }
    }

    pub fn update(&mut self, store: &mut Store) {
        let state = store.state();
        let game = &state.server.game;
        let arena = match game.arena.as_ref() {
            Some(arena) => arena,
            None => return,
        };
        let now = Instant::now();

        if game.arena_number != self.arena_number {
            self.arena_number = game.arena_number;
            self.remains.clear();
            self.previous_entities.clear();
            self.damaged_entities.clear();
        }

        const ENTITY_DAMAGE_ANIMATION_TIME: Duration = Duration::from_millis(66);
        self.damaged_entities.retain(|_, from| now - *from < ENTITY_DAMAGE_ANIMATION_TIME);
        self.remains.retain(|(.., at)| now - *at < REMAINS_TIME);

        for (id, entity) in &self.previous_entities {
            match arena.entities.get(id) {
                Some(current) if current.health < entity.health => {
                    self.damaged_entities.insert(*id, now);
                }
                None if game.status == GameStatus::Started => {
                    self.remains.push((entity.position, entity.character_id, now));
                }
                _ => (),
            }
        }
        self.previous_entities = arena.entities.clone();

        match game.status {
            GameStatus::Finished => {
                let finished_at = *self.finished_at.get_or_insert(now);
                if state.session == Some(Session::Demo) && now - finished_at > DEMO_REMATCH {
                    self.finished_at = None;
                    store.dispatch(Action::Rematch);
                }
            }
            _ => self.finished_at = None,
        }
    }
}

#[derive(derive_new::new)]
pub struct ArenaWidget<'a> {
    state: &'a State,
    arena: &'a Arena,
}

fn arena_dimension(state: &State) -> (usize, usize) {
    match &state.server.game.arena {
        Some(arena) => (arena.width.max(DEFAULT_DIMENSION.0), arena.height.max(DEFAULT_DIMENSION.1)),
        None => DEFAULT_DIMENSION,
    }
}

impl<'a> ArenaWidget<'a> {
    pub fn dimension(state: &State) -> (u16, u16) {
        let (width, height) = arena_dimension(state);
        let map_dim = MapWidget::dimension(width as u16, height as u16);

        (
            PlayerPanelListWidget::WIDTH + 1 + map_dim.0,
            ArenaInfoLabelWidget::HEIGHT + map_dim.1 + SkillBarWidget::HEIGHT + NotificationLabelWidget::HEIGHT,
        )
    }
}

impl Widget for ArenaWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let arena = match &self.state.server.game.arena {
            Some(arena) => arena,
            None => return,
        };
        let (width, height) = arena_dimension(self.state);
        let map_dim = MapWidget::dimension(width as u16, height as u16);

        let column = Layout::default()
            .direction(Dir::Vertical)
            .constraints([
                Constraint::Length(ArenaInfoLabelWidget::HEIGHT),
                Constraint::Length(map_dim.1),
                Constraint::Length(SkillBarWidget::HEIGHT),
                Constraint::Length(NotificationLabelWidget::HEIGHT),
            ])
            .split(area);

        ArenaInfoLabelWidget::new(self.state).render(column[0], buffer);

        let row = Layout::default()
            .direction(Dir::Horizontal)
            .constraints([
                Constraint::Length(PlayerPanelListWidget::WIDTH),
                Constraint::Length(1), //Margin
                Constraint::Length(map_dim.0),
            ])
            .split(column[1]);

        PlayerPanelListWidget::new(self.state).render(row[0], buffer);

        // A smaller arena sits in the middle of the room the largest one takes.
        let own = MapWidget::dimension(arena.width as u16, arena.height as u16);
        let map_area = util::centered_area(row[2], own);
        MapWidget::new(self.state, self.arena).render(map_area, buffer);

        if self.arena.confirm_leave {
            LeaveWidget.render(row[2], buffer);
        }

        SkillBarWidget::new(self.state).render(column[2], buffer);

        NotificationLabelWidget::new(self.state).render(column[3], buffer);
    }
}

#[derive(derive_new::new)]
struct ArenaInfoLabelWidget<'a> {
    state: &'a State,
}

impl ArenaInfoLabelWidget<'_> {
    pub const HEIGHT: u16 = 1;
}

impl Widget for ArenaInfoLabelWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let game = &self.state.server.game;
        let arena = match &game.arena {
            Some(arena) => arena,
            None => return,
        };
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::Gray);

        let mut title = vec![
            Span::styled("Arena ", dim),
            Span::styled(game.arena_number.to_string(), bold),
            Span::styled(format!(" of {}", game.rounds), dim),
            Span::styled(" · ", dim),
            Span::styled(arena.name.clone(), bold.fg(Color::Indexed(180))),
        ];
        if game.winner_points > 0 {
            title.push(Span::styled(" · first to ", dim));
            title.push(Span::styled(game.winner_points.to_string(), bold));
            title.push(Span::styled(" points", dim));
        }
        if game.status == GameStatus::Started && game.arena_winner.is_none() {
            title.push(Span::styled(" · ", dim));
            match arena.storm {
                0 => {
                    let left = (arena.started + arena.storm_after).saturating_duration_since(Instant::now());
                    title.push(Span::styled("storm in ", dim));
                    title.push(Span::styled(format!("{}s", left.as_secs() + 1), bold));
                }
                _ => title.push(Span::styled(
                    "the storm closes in!",
                    bold.fg(Color::Indexed(203)),
                )),
            }
        }

        Paragraph::new(Line::from(title)).alignment(Alignment::Center).render(area, buffer);
    }
}

#[derive(derive_new::new)]
struct PlayerPanelListWidget<'a> {
    state: &'a State,
}

impl PlayerPanelListWidget<'_> {
    pub const WIDTH: u16 = PlayerPanelWidget::DIMENSION.0;
}

impl Widget for PlayerPanelListWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let game = &self.state.server.game;
        let players = &game.players;
        let arena = match &game.arena {
            Some(arena) => arena,
            None => return,
        };

        let full = players.len() as u16 * PlayerPanelWidget::DIMENSION.1 < area.height;
        let height = match full {
            true => PlayerPanelWidget::DIMENSION.1,
            false => CompactPanelWidget::HEIGHT,
        };

        let mut constraints = vec![Constraint::Length(if full { 1 } else { 0 })]; //Top margin
        constraints.extend(players.iter().map(|_| Constraint::Length(height)));
        constraints.push(Constraint::Min(0)); // Bottom margin

        let row = Layout::default().direction(Dir::Vertical).constraints(constraints).split(area);

        for (index, player) in players.iter().enumerate() {
            let character = match game.characters.get(&player.character_id) {
                Some(character) => character,
                None => continue,
            };
            let entity = arena.entities.get(&player.entity_id);
            let panel_area = row[index + 1];
            match full {
                true => PlayerPanelWidget::new(self.state, player, character, entity).render(panel_area, buffer),
                false => CompactPanelWidget::new(self.state, player, character, entity).render(panel_area, buffer),
            }
        }
    }
}

fn tag(state: &State, player: &Player) -> &'static str {
    let is_user = state.server.game.arena.as_ref().map(|a| a.user_player.player_id == player.id).unwrap_or(false);
    match (is_user, player.bot) {
        (true, _) if state.session == Some(Session::Demo) => "demo",
        (true, _) => "you",
        (false, Some(_)) => "bot",
        (false, None) => "",
    }
}

#[derive(derive_new::new)]
struct PlayerPanelWidget<'a> {
    state: &'a State,
    player: &'a Player,
    character: &'a Character,
    entity: Option<&'a EntityData>,
}

impl<'a> PlayerPanelWidget<'a> {
    pub const DIMENSION: (u16, u16) = (27, 5);
}

impl Widget for PlayerPanelWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let is_user = self.player.id == self.state.server.game.arena().user_player.player_id;
        let box_border_style = match is_user {
            true => Style::default()
                .fg(if self.entity.is_some() { Color::White } else { Color::DarkGray })
                .add_modifier(Modifier::BOLD),
            false => Style::default().fg(if self.entity.is_some() {
                Color::Gray
            }
            else {
                Color::DarkGray
            }),
        };

        // Symbol panel
        let symbol_area = Rect::new(area.x, area.y, 5, 3).intersection(area);
        Block::default()
            .borders(Borders::ALL)
            .border_style(box_border_style)
            .border_type(BorderType::Rounded)
            .render(symbol_area, buffer);

        let player_color = match self.entity {
            Some(_) => player_color(self.player.id),
            None => Color::DarkGray,
        };
        let player_style = Style::default().fg(player_color).add_modifier(Modifier::BOLD);

        let symbol = self.character.symbol().to_string();
        buffer.set_string(area.x + 2, area.y + 1, symbol, player_style);

        // Main panel
        let panel_area = Rect::new(symbol_area.right(), area.y, 22, 4).intersection(area);
        let points_style = Style::default().fg(Color::White);
        Block::default()
            .title(Line::from(vec![
                Span::raw("──"),
                Span::styled(" Pts: ", points_style),
                Span::styled(self.player.points.to_string(), points_style.add_modifier(Modifier::BOLD)),
                Span::styled(" · K: ", points_style),
                Span::styled(self.player.kills.to_string(), points_style.add_modifier(Modifier::BOLD)),
                Span::raw(" "),
            ]))
            .borders(Borders::ALL)
            .border_style(box_border_style)
            .border_type(BorderType::Rounded)
            .render(panel_area, buffer);

        // Bars
        let content = panel_area.inner(Margin { vertical: 1, horizontal: 1 });

        let bar_area = Rect::new(content.x, content.y, content.width, 1).intersection(area);
        let health = self.entity.map(|e| e.health).unwrap_or(0);
        BarWidget::new(health, self.character.max_health(), Color::Green).render(bar_area, buffer);

        let bar_area = Rect::new(content.x, content.y + 1, content.width, 1).intersection(area);
        let energy = self.entity.map(|e| e.energy).unwrap_or(0);
        BarWidget::new(energy, self.character.max_energy(), Color::Cyan).render(bar_area, buffer);

        // Bottom
        let arrow = box_border_style;
        let bottom = Rect::new(panel_area.x, panel_area.bottom() - 1, panel_area.width, 1)
            .intersection(area);
        let bottom = bottom.inner(Margin { vertical: 0, horizontal: 2 });

        let clean_row = (0..bottom.width).map(|_| " ").collect::<String>();
        buffer.set_string(bottom.x, bottom.y, clean_row, arrow);

        buffer.set_string(bottom.x, bottom.y, ">", arrow);
        buffer.set_string(bottom.right() - 1, bottom.y, "<", arrow);

        let tag = tag(self.state, self.player);
        let x = bottom.x + (bottom.width.saturating_sub(tag.len() as u16)) / 2;
        buffer.set_string(x, bottom.y, tag, arrow);
    }
}

/// Two lines per player, for matches too big for the full panels.
#[derive(derive_new::new)]
struct CompactPanelWidget<'a> {
    state: &'a State,
    player: &'a Player,
    character: &'a Character,
    entity: Option<&'a EntityData>,
}

impl CompactPanelWidget<'_> {
    const HEIGHT: u16 = 2;
}

impl Widget for CompactPanelWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if area.height == 0 {
            return
        }
        let alive = self.entity.is_some();
        let color = if alive { player_color(self.player.id) } else { Color::DarkGray };
        let is_user = self.player.id == self.state.server.game.arena().user_player.player_id;
        let symbol_style = match is_user {
            true => Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD),
            false => Style::default().fg(color).add_modifier(Modifier::BOLD),
        };
        let text = Style::default().fg(if alive { Color::Gray } else { Color::DarkGray });
        let line = Line::from(vec![
            Span::styled(format!(" {} ", self.character.symbol()), symbol_style),
            Span::styled(format!(" {:<4}", tag(self.state, self.player)), text),
            Span::styled(format!("{:>3}", self.player.points), text.add_modifier(Modifier::BOLD).fg(Color::White)),
            Span::styled(" pts ", text),
            Span::styled(format!("{:>2}", self.player.kills), text.add_modifier(Modifier::BOLD).fg(Color::White)),
            Span::styled(" kills", text),
        ]);
        buffer.set_line(area.x, area.y, &line, area.width);

        if area.height > 1 {
            let health = self.entity.map(|e| e.health).unwrap_or(0);
            let energy = self.entity.map(|e| e.energy).unwrap_or(0);
            let bars = Line::from(vec![
                Span::raw("    "),
                mini_bar(health, self.character.max_health(), 12, Color::Green),
                Span::raw(" "),
                mini_bar(energy, self.character.max_energy(), 8, Color::Cyan),
            ]);
            buffer.set_line(area.x, area.y + 1, &bars, area.width);
        }
    }
}

fn mini_bar(current: usize, max: usize, length: usize, color: Color) -> Span<'static> {
    let filled = (current * length).div_ceil(max.max(1)).min(length);
    let bar = "=".repeat(filled) + &"·".repeat(length - filled);
    Span::styled(bar, Style::default().fg(color))
}

#[derive(derive_new::new)]
struct BarWidget {
    current: usize,
    max: usize,
    color: Color,
}

impl Widget for BarWidget {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let bar_style = Style::default().fg(self.color);
        let bar_off_style = Style::default().fg(self.color);

        let bar_len = 10_u16;
        let current_len = ((self.current + 9) as f32 / bar_len as f32) as usize;
        let bar = (0..bar_len as usize)
            .map(|index| {
                if index + 1 == current_len {
                    let partial = self.current as u16 % bar_len;
                    if partial > bar_len / 2 || partial == 0 {
                        Span::styled("=", bar_style.add_modifier(Modifier::BOLD))
                    }
                    else {
                        Span::styled("-", bar_style.add_modifier(Modifier::BOLD))
                    }
                }
                else if index < current_len {
                    Span::styled("=", bar_style.add_modifier(Modifier::BOLD))
                }
                else {
                    Span::styled("·", bar_off_style)
                }
            })
            .collect::<Vec<_>>();

        let limit_style = Style::default().fg(Color::White);
        buffer.set_string(area.x, area.y, "[", limit_style);
        buffer.set_line(area.x + 1, area.y, &Line::from(bar), 10);
        buffer.set_string(area.x + bar_len + 1, area.y, "]", limit_style);

        let numbers = Line::from(vec![
            Span::styled(format!("{:>3}", self.current), bar_style.add_modifier(Modifier::BOLD)),
            Span::styled("/", limit_style),
            Span::styled(self.max.to_string(), bar_style.add_modifier(Modifier::BOLD)),
        ]);
        buffer.set_line(area.x + bar_len + 3, area.y, &numbers, 7);
    }
}

#[derive(derive_new::new)]
struct MapWidget<'a> {
    state: &'a State,
    arena: &'a Arena,
}

impl MapWidget<'_> {
    pub fn dimension(width: u16, height: u16) -> (u16, u16) {
        (width * 2 - 1, height)
    }
}

fn spell_style(spec_id: crate::ids::SpellSpecId, owner_color: Color) -> (char, Style) {
    let style = Style::default();
    match spec_id {
        spells::FIRE_BALL => ('o', style.fg(Color::Indexed(208)).add_modifier(Modifier::BOLD)),
        spells::SPARK => ('*', style.fg(Color::Indexed(226)).add_modifier(Modifier::BOLD)),
        spells::STONE_WALL => ('▓', style.fg(Color::Indexed(137))),
        spells::MINE => ('^', style.fg(owner_color)),
        spells::TRAIL => ('·', style.fg(Color::Indexed(244))),
        spells::BLAST => ('✶', style.fg(Color::Indexed(214)).add_modifier(Modifier::BOLD)),
        _ => ('?', style),
    }
}

impl Widget for MapWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let game = &self.state.server.game;
        let arena = game.arena();
        let inside = |x: i32, y: i32| -> Option<(u16, u16)> {
            let (cx, cy) = (area.x as i32 + x, area.y as i32 + y);
            match cx >= area.left() as i32 && cx < area.right() as i32 && cy >= area.top() as i32 && cy < area.bottom() as i32 {
                true => Some((cx as u16, cy as u16)),
                false => None,
            }
        };
        let put = |buffer: &mut Buffer, x: i32, y: i32, symbol: &str, style: Style| {
            if let Some((cx, cy)) = inside(x, y) {
                buffer[(cx, cy)].set_symbol(symbol).set_style(style);
            }
        };

        // Storm: the floor it covers glows red, gaps between cells too.
        if arena.storm > 0 {
            let storm = Style::default().fg(Color::Indexed(124)).bg(Color::Indexed(52));
            for y in 0..arena.height as i32 {
                for x in 0..arena.width as i32 {
                    let cell = Vec2::xy(x, y);
                    if arena.terrain(cell) == Terrain::Floor && arena.in_storm(cell) {
                        put(buffer, x * 2, y, "░", storm);
                        if arena.in_storm(cell + Vec2::x(1)) || arena.terrain(cell + Vec2::x(1)) == Terrain::Wall {
                            put(buffer, x * 2 + 1, y, "░", storm);
                        }
                        if arena.terrain(cell - Vec2::x(1)) == Terrain::Wall {
                            put(buffer, x * 2 - 1, y, "░", storm);
                        }
                    }
                }
            }
        }

        // Player sight
        let user_player = &arena.user_player;
        if let Some(player) = game.players.get(user_player.player_id) {
            if let Some(entity) = arena.entities.get(&player.entity_id) {
                // Keys turn you at once; when the computer plays you, it is the server's word.
                let facing = match self.state.session {
                    Some(Session::Demo) => entity.direction,
                    _ => user_player.direction,
                };
                let pos = entity.position + facing.to_vec2();
                let style = Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD);
                if arena.terrain(pos) == Terrain::Floor {
                    put(buffer, pos.x * 2, pos.y, "·", style);
                }
            }
        }

        // Border
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White))
            .border_type(BorderType::Rounded)
            .render(area, buffer);

        // Walls inside the border, joined to their neighbours.
        let wall = Style::default().fg(Color::Indexed(247));
        for y in 1..arena.height as i32 - 1 {
            for x in 1..arena.width as i32 - 1 {
                let cell = Vec2::xy(x, y);
                if arena.terrain(cell) == Terrain::Wall {
                    put(buffer, x * 2, y, "█", wall);
                    if arena.terrain(cell + Vec2::x(1)) == Terrain::Wall {
                        put(buffer, x * 2 + 1, y, "█", wall);
                    }
                    if x == 1 {
                        put(buffer, x * 2 - 1, y, "█", wall);
                    }
                }
            }
        }

        // Remains of the fallen
        for (position, character_id, _) in &self.arena.remains {
            let color = game
                .players
                .iter()
                .find(|player| player.character_id == *character_id)
                .map(|player| player_color(player.id))
                .unwrap_or(Color::Gray);
            put(buffer, position.x * 2, position.y, "x", Style::default().fg(color).add_modifier(Modifier::DIM));
        }

        // Spells
        let mut spells = arena.spells.values().collect::<Vec<_>>();
        spells.sort_by_key(|spell| (spell.spec_id == spells::FIRE_BALL || spell.spec_id == spells::SPARK, spell.id.0));
        for spell in spells {
            let owner_color = game
                .players
                .iter()
                .find(|player| player.entity_id == spell.owner)
                .map(|player| player_color(player.id))
                .unwrap_or(Color::Gray);
            let (symbol, style) = spell_style(spell.spec_id, owner_color);
            let (x, y) = (spell.position.x, spell.position.y);
            put(buffer, x * 2, y, &symbol.to_string(), style);
            if spell.spec_id == spells::STONE_WALL {
                let joined = arena.spells.values().any(|other| {
                    other.spec_id == spells::STONE_WALL && other.position == spell.position + Vec2::x(1)
                });
                if joined {
                    put(buffer, x * 2 + 1, y, "▓", style);
                }
            }
        }

        // Entities
        for (id, entity) in &arena.entities {
            let player = game.players.iter().find(|player| player.entity_id == *id);
            let color = match (self.arena.damaged_entities.get(id), player) {
                (Some(_), _) => Color::LightRed,
                (None, Some(player)) => player_color(player.id),
                (None, None) => Color::White,
            };
            let is_user = player.map(|p| p.id == user_player.player_id).unwrap_or(false);
            let style = match is_user {
                true => Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD),
                false => Style::default().fg(color).add_modifier(Modifier::BOLD),
            };
            let symbol = game.characters.get(&entity.character_id).map(|c| c.symbol()).unwrap_or('?');
            put(buffer, entity.position.x * 2, entity.position.y, &symbol.to_string(), style);
        }

        FinishGameMessageWidget::new(self.state).render(area, buffer);
    }
}

#[derive(derive_new::new)]
struct FinishGameMessageWidget<'a> {
    state: &'a State,
}

impl Widget for FinishGameMessageWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let game = &self.state.server.game;
        if game.status != GameStatus::Finished || game.standings.is_empty() {
            return
        }
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let dim = Style::default().fg(Color::Gray);
        let top = &game.standings[0];
        let shared = game.standings.iter().filter(|s| s.points == top.points && s.kills == top.kills).count() > 1;

        let mut message = vec![
            Line::from(match shared {
                false => vec![Span::raw("Player "), letter(self.state, top.symbol), Span::styled(" wins the match!", bold)],
                true => vec![Span::styled("A shared victory!", bold)],
            }),
            Line::from(""),
            Line::from(Span::styled("     Pts  Kills  Rounds", dim)),
        ];
        for (place, standing) in game.standings.iter().enumerate().take(area.height.saturating_sub(7) as usize) {
            message.push(Line::from(vec![
                Span::styled(format!("{}. ", place + 1), dim),
                letter(self.state, standing.symbol),
                Span::styled(format!(" {:>4} {:>6} {:>7}", standing.points, standing.kills, standing.rounds_won), bold),
                Span::styled(if standing.bot { "  bot" } else { "     " }, dim),
            ]));
        }
        message.push(Line::from(""));
        let local = self.state.session.as_ref().map(|s| s.is_local()).unwrap_or(false);
        let enter = Span::styled(" <Enter> ", bold.fg(Color::Cyan));
        let esc = Span::styled(" <Esc> ", bold.fg(Color::Yellow));
        message.push(Line::from(match (self.state.session.as_ref(), local) {
            (Some(Session::Demo), _) => vec![Span::raw("Another match in a moment"), esc, Span::raw("quit")],
            (_, true) => vec![enter, Span::raw("fight again"), esc, Span::raw("menu")],
            (_, false) => vec![enter, Span::raw("back to the lobby")],
        }));

        let height = message.len() as u16;
        let width = 46.min(area.width.saturating_sub(2));
        let inner = util::centered_area(area, (width, height + 2));
        Clear.render(inner, buffer);
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::White))
            .render(inner, buffer);
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .render(inner.inner(Margin { vertical: 1, horizontal: 1 }), buffer);
    }
}

struct LeaveWidget;

impl Widget for LeaveWidget {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let message = vec![
            Line::from(Span::styled("Leave the match?", bold)),
            Line::from(vec![
                Span::styled(" <Y> ", bold.fg(Color::Cyan)),
                Span::raw("leave  "),
                Span::styled(" <any key> ", bold.fg(Color::Yellow)),
                Span::raw("fight on"),
            ]),
        ];
        let inner = util::centered_area(area, (36, 4));
        Clear.render(inner, buffer);
        Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).render(inner, buffer);
        Paragraph::new(message)
            .alignment(Alignment::Center)
            .render(inner.inner(Margin { vertical: 1, horizontal: 1 }), buffer);
    }
}

#[derive(derive_new::new)]
struct SkillBarWidget<'a> {
    state: &'a State,
}

impl SkillBarWidget<'_> {
    const HEIGHT: u16 = 1;
}

impl Widget for SkillBarWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let entity = self.state.server.game.user_entity();
        let mut spans = Vec::new();
        for (index, (_, spec)) in skills::all().enumerate() {
            if index > 0 {
                spans.push(Span::raw("   "));
            }
            let cooldown = entity.and_then(|e| e.cooldowns.get(index).copied()).unwrap_or(0);
            let affordable = entity.map(|e| e.energy >= spec.energy).unwrap_or(false);
            let (key_style, name_style) = match (entity.is_some(), cooldown, affordable) {
                (false, ..) => (Style::default().fg(Color::DarkGray), Style::default().fg(Color::DarkGray)),
                (true, 0, true) => (
                    Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                    Style::default().fg(Color::White),
                ),
                (true, 0, false) => (
                    Style::default().fg(Color::Black).bg(Color::Red),
                    Style::default().fg(Color::Indexed(203)),
                ),
                (true, _, _) => (
                    Style::default().fg(Color::Black).bg(Color::DarkGray),
                    Style::default().fg(Color::DarkGray),
                ),
            };
            spans.push(Span::styled(format!(" {} ", spec.key), key_style));
            spans.push(Span::styled(format!(" {}", spec.name), name_style));
            if cooldown > 300 {
                spans.push(Span::styled(format!(" {:.0}", (cooldown as f32 / 1000.0).ceil()), name_style));
            }
        }
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center).render(area, buffer);
    }
}

#[derive(derive_new::new)]
struct NotificationLabelWidget<'a> {
    state: &'a State,
}

impl NotificationLabelWidget<'_> {
    const HEIGHT: u16 = 2;
}

fn feed_line(state: &State, item: &FeedItem) -> Line<'static> {
    let text = Style::default().fg(Color::Gray);
    let victim = letter(state, item.victim);
    let mut spans = vec![victim];
    match (item.cause, item.killer) {
        (Cause::Storm, _) => spans.push(Span::styled(" was swallowed by the storm", text)),
        (Cause::Spell(spell), Some(killer)) => {
            let (before, after) = match spell {
                spells::FIRE_BALL => (" was burned to ashes by ", "'s fire ball"),
                spells::SPARK => (" was torn apart by ", "'s nova"),
                spells::MINE => (" stepped on ", "'s mine"),
                _ => (" fell to ", ""),
            };
            spans.push(Span::styled(before, text));
            spans.push(letter(state, killer));
            spans.push(Span::styled(after, text));
        }
        (Cause::Spell(_), None) => spans.push(Span::styled(" fell", text)),
    }
    Line::from(spans)
}

impl Widget for NotificationLabelWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let game = &self.state.server.game;
        let style = Style::default().fg(Color::LightCyan);
        let now = Instant::now();

        let messages = match (game.next_arena_timestamp, game.status) {
            (_, GameStatus::Finished) => vec![],
            (Some(timestamp), _) => {
                let secs = timestamp.saturating_duration_since(now).as_secs() + 1;
                let winner_message = match game.arena_winner.flatten() {
                    Some(symbol) => Line::from(vec![
                        Span::styled("Player ", style),
                        letter(self.state, symbol),
                        Span::styled(format!(" wins arena {}!", game.arena_number), style),
                    ]),
                    None => Line::from(vec![Span::styled("No one survived", style)]),
                };

                vec![
                    winner_message,
                    Line::from(vec![
                        Span::styled(format!("Arena {} of {} in ", game.arena_number + 1, game.rounds), style),
                        Span::styled(secs.to_string(), style.add_modifier(Modifier::BOLD)),
                        Span::styled("...", style),
                    ]),
                ]
            }
            (None, _) => {
                let mut lines = game
                    .feed
                    .iter()
                    .filter(|item| now - item.at < FEED_TIME)
                    .map(|item| feed_line(self.state, item))
                    .collect::<Vec<_>>();
                let user_out = game.user_entity().is_none() && game.status == GameStatus::Started;
                let hint = match self.state.session {
                    Some(Session::Demo) => Some(Line::from(vec![
                        Span::styled("The computer plays every letter · ", Style::default().fg(Color::Gray)),
                        Span::styled("<Esc>", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(" quit", Style::default().fg(Color::Gray)),
                    ])),
                    _ if user_out => Some(Line::from(Span::styled(
                        "You are out. The others fight on...",
                        Style::default().fg(Color::Gray),
                    ))),
                    _ => None,
                };
                if let Some(hint) = hint {
                    if lines.len() >= 2 {
                        lines.remove(0);
                    }
                    lines.push(hint);
                }
                lines
            }
        };

        Paragraph::new(messages).alignment(Alignment::Center).render(area, buffer);
    }
}
