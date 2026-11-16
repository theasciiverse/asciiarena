use super::util::{self};
use super::menu::{TitlePanelWidget};

use crate::client::configuration::{Config, MatchOptions};
use crate::client::state::{State};
use crate::client::store::{Store, Action};
use crate::client::terminal::input::{InputEvent};
use crate::message::{BotLevel};
use crate::server::{ArenaChoice};
use crate::specification::{skills};

use ratatui::buffer::{Buffer};
use ratatui::widgets::{Block, Borders, BorderType, Paragraph, Widget};
use ratatui::layout::{Layout, Constraint, Direction, Rect, Alignment, Margin};
use ratatui::style::{Style, Modifier, Color};
use ratatui::text::{Span, Line};

use crossterm::event::{KeyCode};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Item {
    Solo,
    Host,
    Join,
    Controls,
    Quit,
    Letter,
    Bots,
    Level,
    Arena,
    Rounds,
    LanPlayers,
}

const ITEMS: [Item; 11] = [
    Item::Solo,
    Item::Host,
    Item::Join,
    Item::Controls,
    Item::Quit,
    Item::Letter,
    Item::Bots,
    Item::Level,
    Item::Arena,
    Item::Rounds,
    Item::LanPlayers,
];

const ACTIONS: usize = 5;

/// The main menu: play alone, host or join a game on the local network, and
/// the match they set up.
pub struct Home {
    cursor: usize,
    options: MatchOptions,
    controls: bool,
}

impl Home {
    pub fn new(config: &Config) -> Home {
        Home { cursor: 0, options: config.options.clone(), controls: false }
    }

    fn item(&self) -> Item {
        ITEMS[self.cursor]
    }

    pub fn process_event(&mut self, store: &mut Store, event: InputEvent) {
        let key_event = match event {
            InputEvent::KeyPressed(key_event) => key_event,
            InputEvent::ResizeDisplay => return,
        };

        if self.controls {
            self.controls = false;
            return
        }

        let before = self.options.clone();
        match key_event.code {
            KeyCode::Up | KeyCode::BackTab => self.cursor = (self.cursor + ITEMS.len() - 1) % ITEMS.len(),
            KeyCode::Down | KeyCode::Tab => self.cursor = (self.cursor + 1) % ITEMS.len(),
            KeyCode::Left => self.change(-1),
            KeyCode::Right => self.change(1),
            KeyCode::Esc => return store.dispatch(Action::CloseApp),
            KeyCode::Enter => match self.item() {
                Item::Solo => store.dispatch(Action::StartSolo(self.options.clone(), true)),
                Item::Host => store.dispatch(Action::HostGame(self.options.clone())),
                Item::Join => store.dispatch(Action::JoinGame),
                Item::Controls => self.controls = true,
                Item::Quit => store.dispatch(Action::CloseApp),
                _ => self.change(1),
            },
            KeyCode::Char(c) if self.item() == Item::Letter && c.is_ascii_alphabetic() => {
                self.options.letter = c.to_ascii_uppercase();
            }
            KeyCode::Char('?') => self.controls = true,
            _ => (),
        }
        if self.options != before {
            log::debug!("Options: {:?}", self.options);
            store.dispatch(Action::SetOptions(self.options.clone()));
        }
    }

    fn change(&mut self, delta: i32) {
        let item = self.item();
        let options = &mut self.options;
        let step = |value: usize, len: usize| ((value as i32 + delta).rem_euclid(len as i32)) as usize;
        match item {
            Item::Letter => {
                let index = (options.letter as u8 - b'A') as usize;
                options.letter = (b'A' + step(index, 26) as u8) as char;
            }
            Item::Bots => {
                options.bots = (options.bots as i32 + delta).clamp(1, MatchOptions::MAX_BOTS as i32) as usize;
            }
            Item::Level => {
                let index = BotLevel::ALL.iter().position(|l| *l == options.level).unwrap_or(1);
                options.level = BotLevel::ALL[step(index, BotLevel::ALL.len())];
            }
            Item::Arena => {
                let all = ArenaChoice::all().collect::<Vec<_>>();
                let index = all.iter().position(|a| *a == options.arena).unwrap_or(0);
                options.arena = all[step(index, all.len())];
            }
            Item::Rounds => {
                let all = MatchOptions::ROUNDS;
                let index = all.iter().position(|r| *r >= options.rounds).unwrap_or(2);
                options.rounds = all[step(index, all.len())];
            }
            Item::LanPlayers => {
                options.lan_players = (options.lan_players as i32 + delta).clamp(2, 8) as usize;
            }
            _ => (),
        }
    }

    fn describe(&self) -> Vec<Span<'static>> {
        let o = &self.options;
        let gray = Style::default().fg(Color::Gray);
        let text = match self.item() {
            Item::Solo => format!(
                "You against {} {} bot{}, {} round{}",
                o.bots,
                o.level.name().to_lowercase(),
                if o.bots == 1 { "" } else { "s" },
                o.rounds,
                if o.rounds == 1 { "" } else { "s" },
            ),
            Item::Host => format!(
                "Serves a game to your local network for {} players, plus {} bot{}",
                o.lan_players,
                o.bots,
                if o.bots == 1 { "" } else { "s" }
            ),
            Item::Join => "Joins a game someone hosts on your local network".into(),
            Item::Controls => "The keys of the arena".into(),
            Item::Quit => "Leaves the arena for today".into(),
            Item::Letter => "Who you are in the arena: type any letter".into(),
            Item::Bots => "Computer players in your matches".into(),
            Item::Level => match o.level {
                BotLevel::Easy => "Slow to see a fire ball coming, and slow to aim one".into(),
                BotLevel::Normal => "Dodges, dashes and lays mines".into(),
                BotLevel::Hard => "Quick, sharp, and they raise walls against your fire".into(),
            },
            Item::Arena => match o.arena {
                ArenaChoice::Rotate => "A different arena every round".into(),
                ArenaChoice::Only(kind) => format!("Every round in {}", kind.name()),
            },
            Item::Rounds => "Arenas in a match: the most points at the end wins".into(),
            Item::LanPlayers => "People a hosted game waits for, you included".into(),
        };
        vec![Span::styled(text, gray)]
    }
}

#[derive(derive_new::new)]
pub struct HomeWidget<'a> {
    state: &'a State,
    home: &'a Home,
}

impl HomeWidget<'_> {
    pub fn dimension() -> (u16, u16) {
        let title = TitlePanelWidget::dimension();
        (title.0, title.1 + 1 + 1 + PANEL_HEIGHT + 1 + 3)
    }
}

const PANEL_HEIGHT: u16 = 8;

impl Widget for HomeWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let column = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(TitlePanelWidget::dimension().1),
                Constraint::Length(1), // Tagline
                Constraint::Length(1), // Margin
                Constraint::Length(PANEL_HEIGHT),
                Constraint::Length(1), // Margin
                Constraint::Length(3),
            ])
            .split(area);

        TitlePanelWidget.render(column[0], buffer);

        let tagline = Line::from(vec![
            Span::styled("Pick a letter. Be the last one standing.", Style::default().fg(Color::Indexed(180))),
            Span::styled(format!("   v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::DarkGray)),
        ]);
        Paragraph::new(tagline).alignment(Alignment::Center).render(column[1], buffer);

        if self.home.controls {
            ControlsWidget.render(column[3], buffer);
        }
        else {
            let row = Layout::default()
                .direction(Direction::Horizontal)
                .horizontal_margin(2)
                .constraints([Constraint::Length(26), Constraint::Length(4), Constraint::Min(1)])
                .split(column[3]);
            self.render_actions(row[0], buffer);
            self.render_options(row[2], buffer);
        }

        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut footer = vec![Line::from(self.home.describe()), Line::from("")];
        footer.push(match self.home.controls {
            true => Line::from(vec![Span::raw("Press any key to go back")]),
            false => Line::from(vec![
                Span::styled("<↑↓>", bold.fg(Color::Cyan)),
                Span::raw(" choose  "),
                Span::styled("<←→>", bold.fg(Color::Cyan)),
                Span::raw(" change  "),
                Span::styled("<Enter>", bold.fg(Color::Cyan)),
                Span::raw(" go  "),
                Span::styled("<Esc>", bold.fg(Color::Yellow)),
                Span::raw(" quit"),
            ]),
        });
        if let Some(problem) = &self.state.problem {
            footer[1] = Line::from(Span::styled(problem.clone(), Style::default().fg(Color::LightRed)));
        }
        Paragraph::new(footer).alignment(Alignment::Center).render(column[5], buffer);
    }
}

impl HomeWidget<'_> {
    fn render_actions(&self, area: Rect, buffer: &mut Buffer) {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(" Play ", Style::default().add_modifier(Modifier::BOLD)))
            .render(area, buffer);
        let inner = area.inner(Margin { vertical: 1, horizontal: 2 });
        let names = ["Play solo", "Host a LAN game", "Join a LAN game", "Controls", "Quit"];
        for (index, name) in names.iter().enumerate() {
            let selected = self.home.cursor == index;
            let style = match selected {
                true => Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                false => Style::default().fg(Color::White),
            };
            let y = inner.y + index as u16 + if index >= 3 { 1 } else { 0 };
            if y < inner.bottom() {
                let text = format!(" {:<width$}", name, width = inner.width as usize - 1);
                buffer.set_string(inner.x, y, text, style);
            }
        }
    }

    fn render_options(&self, area: Rect, buffer: &mut Buffer) {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(Span::styled(" Match ", Style::default().add_modifier(Modifier::BOLD)))
            .render(area, buffer);
        let inner = area.inner(Margin { vertical: 1, horizontal: 2 });
        let o = &self.home.options;
        let rows = [
            ("Your letter", o.letter.to_string()),
            ("Bots", o.bots.to_string()),
            ("Bot skill", o.level.name().to_string()),
            ("Arena", o.arena.name().to_string()),
            ("Rounds", o.rounds.to_string()),
            ("LAN players", o.lan_players.to_string()),
        ];
        for (index, (name, value)) in rows.iter().enumerate() {
            let selected = self.home.cursor == ACTIONS + index;
            let y = inner.y + index as u16;
            if y >= inner.bottom() {
                break
            }
            let name_style = match selected {
                true => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                false => Style::default().fg(Color::Gray),
            };
            let value_style = match selected {
                true => Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD),
                false => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
            };
            buffer.set_string(inner.x, y, *name, name_style);
            let value = match selected {
                true => format!("‹ {} ›", value),
                false => format!("  {}  ", value),
            };
            let x = inner.right().saturating_sub(value.chars().count() as u16);
            buffer.set_string(x, y, value, value_style);
        }
    }
}

struct ControlsWidget;

impl Widget for ControlsWidget {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let key = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        let text = Style::default().fg(Color::Gray);
        let mut lines = vec![Line::from(vec![
            Span::styled("W A S D", key),
            Span::styled(" or ", text),
            Span::styled("arrows", key),
            Span::styled("  walk and turn       ", text),
            Span::styled("Esc", key),
            Span::styled("  leave", text),
        ])];
        for (index, (_, spec)) in skills::all().enumerate() {
            let key_name = if spec.key == "Spc" { "Space".to_string() } else { spec.key.to_string() };
            lines.push(Line::from(vec![
                Span::styled(format!("{:>5}", key_name), key),
                Span::styled(" or ", text),
                Span::styled(format!("{}", index + 1), key),
                Span::styled(format!("  {:<10}", spec.name), Style::default().fg(Color::White)),
                Span::styled(format!("{:>3} energy  ", spec.energy), text),
                Span::styled(spec.description, text),
            ]));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Every rival who falls while you stand is a point for you.",
            Style::default().fg(Color::Indexed(180)),
        )));
        let area = util::centered_area(area, (area.width, lines.len() as u16));
        Paragraph::new(lines).alignment(Alignment::Left).render(area, buffer);
    }
}
