use super::widgets::gui::{Gui, GuiWidget};

use crate::client::state::{State};

use crossterm::terminal::{self};
use crossterm::{ExecutableCommand};

use ratatui::{Terminal};
use ratatui::backend::{CrosstermBackend};
use ratatui::layout::{Position};

use std::io::{self, Stdout};

pub struct Renderer {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

fn restore() {
    let _ = io::stdout().execute(terminal::LeaveAlternateScreen);
    let _ = io::stdout().execute(crossterm::cursor::Show);
    let _ = terminal::disable_raw_mode();
}

impl Renderer {
    pub fn new() -> Renderer {
        terminal::enable_raw_mode().unwrap();
        io::stdout().execute(terminal::EnterAlternateScreen).unwrap();

        // A panic still gives the terminal back in a usable state.
        let hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            hook(info);
        }));

        Renderer { terminal: Terminal::new(CrosstermBackend::new(io::stdout())).unwrap() }
    }

    pub fn render(&mut self, state: &State, gui: &Gui) {
        let result = self.terminal.draw(|frame| {
            let main_widget = GuiWidget::new(state, gui);
            let area = frame.area();
            let mut cursor = Cursor::default();

            frame.render_stateful_widget(main_widget, area, &mut cursor);

            if let Some(position) = cursor.take() {
                frame.set_cursor_position(Position::new(position.0, position.1));
            }
        });
        if let Err(error) = result {
            log::error!("Draw: {}", error);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        restore();
    }
}

#[derive(Default)]
pub struct Cursor {
    position: Option<(u16, u16)>,
}

impl Cursor {
    pub fn set(&mut self, x: u16, y: u16) {
        self.position = Some((x, y));
    }

    fn take(&mut self) -> Option<(u16, u16)> {
        self.position.take()
    }
}
