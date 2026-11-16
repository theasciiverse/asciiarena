use super::util::{self};
use super::menu::{Menu, MenuWidget, TitlePanelWidget};
use super::arena::{Arena, ArenaWidget};
use super::home::{Home, HomeWidget};

use crate::client::configuration::{Config};
use crate::client::store::{Store, Action};
use crate::client::state::{State, Session};

use crate::client::terminal::input::{InputEvent};
use crate::client::terminal::renderer::{Cursor};

use ratatui::buffer::{Buffer};
use ratatui::widgets::{Widget, StatefulWidget, Paragraph};
use ratatui::layout::{Rect, Alignment};
use ratatui::style::{Style, Color, Modifier};
use ratatui::text::{Line, Span};

use crossterm::event::{KeyCode, KeyModifiers};

use std::time::{Instant};

enum View {
    Home,
    /// Waiting for the first arena of a game on this computer.
    Entering,
    Menu,
    Arena,
}

pub struct Gui {
    home: Home,
    menu: Menu,
    arena: Arena,
}

impl Gui {
    pub fn new(config: &Config) -> Gui {
        Gui { home: Home::new(config), menu: Menu::new(config), arena: Arena::new(config) }
    }

    fn view(&self, state: &State) -> View {
        match (&state.server.game.arena, &state.session) {
            (Some(_), _) => View::Arena,
            (None, None) => View::Home,
            (None, Some(Session::Solo(_))) | (None, Some(Session::Demo)) => View::Entering,
            (None, Some(_)) => View::Menu,
        }
    }

    pub fn view_name(&self, state: &State) -> &'static str {
        match self.view(state) {
            View::Home => "home",
            View::Entering => "entering",
            View::Menu => "lobby",
            View::Arena => "arena",
        }
    }

    pub fn process_event(&mut self, store: &mut Store, event: InputEvent) {
        match event {
            InputEvent::KeyPressed(key_event) => {
                if let KeyCode::Char(character) = key_event.code {
                    if character == 'c' && key_event.modifiers.contains(KeyModifiers::CONTROL) {
                        return store.dispatch(Action::CloseApp)
                    }
                }
            }
            InputEvent::ResizeDisplay => {}
        }

        match self.view(store.state()) {
            View::Home => self.home.process_event(store, event),
            View::Entering => {
                if let InputEvent::KeyPressed(key_event) = event {
                    if key_event.code == KeyCode::Esc {
                        match store.state().session {
                            Some(Session::Demo) => store.dispatch(Action::CloseApp),
                            _ => store.dispatch(Action::LeaveGame),
                        }
                    }
                }
            }
            View::Menu => self.menu.process_event(store, event),
            View::Arena => self.arena.process_event(store, event),
        }
    }

    pub fn update(&mut self, store: &mut Store) {
        match self.view(store.state()) {
            View::Home | View::Entering => (),
            View::Menu => self.menu.update(store.state()),
            View::Arena => self.arena.update(store),
        }
    }
}

#[derive(derive_new::new)]
pub struct GuiWidget<'a> {
    state: &'a State,
    gui: &'a Gui,
}

impl StatefulWidget for GuiWidget<'_> {
    type State = Cursor;
    fn render(self, area: Rect, buffer: &mut Buffer, cursor: &mut Cursor) {
        match self.gui.view(self.state) {
            View::Home => {
                let area = util::centered_area(area, HomeWidget::dimension());
                HomeWidget::new(self.state, &self.gui.home).render(area, buffer)
            }
            View::Entering => {
                let title = TitlePanelWidget::dimension();
                let area = util::centered_area(area, (title.0, title.1 + 3));
                EnteringWidget::new(self.state).render(area, buffer)
            }
            View::Menu => {
                let area = util::centered_area(area, MenuWidget::dimension());
                MenuWidget::new(self.state, &self.gui.menu).render(area, buffer, cursor)
            }
            View::Arena => {
                let dimension = ArenaWidget::dimension(self.state);
                let area = util::centered_area(area, dimension);
                ArenaWidget::new(self.state, &self.gui.arena).render(area, buffer)
            }
        }
    }
}

#[derive(derive_new::new)]
struct EnteringWidget<'a> {
    state: &'a State,
}

impl Widget for EnteringWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let title = TitlePanelWidget::dimension();
        TitlePanelWidget.render(Rect::new(area.x, area.y, area.width, title.1), buffer);
        let style = Style::default().fg(Color::LightCyan);
        let message = match self.state.server.game.next_arena_timestamp {
            Some(timestamp) => {
                let secs = timestamp.saturating_duration_since(Instant::now()).as_secs() + 1;
                Line::from(vec![
                    Span::styled("Into the arena in ", style),
                    Span::styled(secs.to_string(), style.add_modifier(Modifier::BOLD)),
                    Span::styled("...", style),
                ])
            }
            None => Line::from(Span::styled("Opening the arena...", style)),
        };
        let rest = Rect::new(area.x, area.y + title.1 + 2, area.width, 1).intersection(area);
        Paragraph::new(message).alignment(Alignment::Center).render(rest, buffer);
    }
}
