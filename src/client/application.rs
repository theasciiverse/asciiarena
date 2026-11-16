use super::configuration::{Config, Start};
use super::state::{State};
use super::store::{Store, Action};
use super::server_proxy::{ServerProxy, ServerEvent};

use super::terminal::input::{InputReceiver, InputEvent};
use super::terminal::renderer::{Renderer};
use super::terminal::widgets::gui::{Gui};

use crate::settings::{Settings};

use message_io::events::{EventReceiver};

use std::time::{Duration};

const APP_FRAME_DURATION: Duration = Duration::from_nanos(1_000_000_000 / 30);

/// Frames between two snapshots in the debug log, about half a second.
const SNAPSHOT_FRAMES: usize = 15;

#[derive(Debug)]
pub enum AppEvent {
    ServerEvent(ServerEvent),
    InputEvent(InputEvent),
    Draw,
}

pub struct Application {
    store: Store,
    gui: Gui,
    start: Start,
    _server: ServerProxy,
    _input: InputReceiver,
    event_queue: EventReceiver<AppEvent>,
}

impl Application {
    pub fn new(config: Config, settings: Settings) -> Application {
        let event_queue = EventReceiver::default();

        let sender = event_queue.sender().clone();
        let mut server =
            ServerProxy::new(move |server_event| sender.send(AppEvent::ServerEvent(server_event)));

        let sender = event_queue.sender().clone();
        let input =
            InputReceiver::new(move |input_event| sender.send(AppEvent::InputEvent(input_event)));

        Application {
            store: Store::new(State::new(&config), server.api(), settings),
            gui: Gui::new(&config),
            start: config.start,
            _server: server,
            _input: input,
            event_queue,
        }
    }

    pub fn run(&mut self) {
        match self.start {
            Start::Home => (),
            Start::Join => self.store.dispatch(Action::StartApp),
            Start::Solo => self.store.dispatch(Action::StartSolo(self.store.state().options.clone(), false)),
            Start::Demo => self.store.dispatch(Action::StartDemo),
        }
        if self.start == Start::Join && self.store.state().session.is_none() {
            self.store.dispatch(Action::JoinGame);
        }
        self.event_queue.sender().send(AppEvent::Draw);

        let mut renderer = Renderer::new();
        let mut frames = 0;
        loop {
            if self.store.should_close() {
                return log::info!("Closing client")
            }

            let event = self.event_queue.receive();
            match event {
                AppEvent::ServerEvent(server_event) => {
                    log::trace!("[Process server event] - {:?}", server_event);
                    self.store.dispatch(Action::ServerEvent(server_event));
                }
                AppEvent::InputEvent(input_event) => {
                    self.gui.process_event(&mut self.store, input_event);
                }
                AppEvent::Draw => {
                    self.gui.update(&mut self.store);
                    renderer.render(self.store.state(), &self.gui);
                    frames += 1;
                    if frames % SNAPSHOT_FRAMES == 0 {
                        log_snapshot(self.store.state(), &self.gui);
                    }
                    self.event_queue.sender().send_with_timer(AppEvent::Draw, APP_FRAME_DURATION);
                }
            }
        }
    }
}

fn log_snapshot(state: &State, gui: &Gui) {
    if !log::log_enabled!(log::Level::Debug) {
        return
    }
    let game = &state.server.game;
    let me = game.user_entity().map(|entity| {
        format!(
            "{} at {} hp {} en {} cooldowns {:?}",
            game.user_symbol().unwrap_or('?'),
            entity.position,
            entity.health,
            entity.energy,
            entity.cooldowns
        )
    });
    let others = game
        .arena
        .as_ref()
        .map(|arena| {
            let mut others = arena
                .entities
                .values()
                .filter_map(|entity| {
                    let symbol = game.characters.get(&entity.character_id)?.symbol();
                    Some(format!("{} {} {}", symbol, entity.position, entity.health))
                })
                .collect::<Vec<_>>();
            others.sort();
            format!("{} storm {} spells {} | {}", arena.name, arena.storm, arena.spells.len(), others.join(", "))
        })
        .unwrap_or_default();
    log::debug!(
        "snapshot: view {} session {:?} connection {:?} game {:?} round {}/{} me {} arena {}",
        gui.view_name(state),
        state.session,
        state.server.connection_status,
        game.status,
        game.arena_number,
        game.rounds,
        me.unwrap_or_else(|| "-".into()),
        others,
    );
}
