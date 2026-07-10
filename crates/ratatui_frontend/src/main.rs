mod connection_state;
mod ui;

use std::{io, path::PathBuf};

use anyhow::{Context, bail};
use chat_backend::{
    ChatBackend,
    client_command::ClientCommand,
    client_event::{self, ClientEvent},
    network_protocol::{ErrorEvent, NetworkCommand, SendDestination, SendMessage},
};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyEvent, KeyEventKind};
use figment::{
    Figment,
    providers::{Format, Serialized, Toml},
};
use futures::StreamExt;
use ratatui::{DefaultTerminal, Frame, widgets::Clear};
use serde::{Deserialize, Serialize};
use shared_utils::{files::NamedProjectDirs, first_match};
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{Duration, interval},
};
use tracing::{debug, info, instrument, warn};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use connection_state::{ConnectionState, MessageContext};
use ui::{
    Action, KeyHandler,
    main_panel::MainPanel,
    popups::{
        Popup,
        notice::{NoticeLevel, NoticePopup},
        popup_area,
    },
};

const DEFAULT_CONFIG: &str = include_str!("../data/config.toml");

#[derive(Debug)]
struct DefaultPaths {
    config: PathBuf,
    log_dir: PathBuf,
}

impl DefaultPaths {
    /// Initialize a `DefaultPaths` instance.
    ///
    /// # Default paths
    /// `config`: `NamedProjectDirs::config_dir()/config.toml`
    fn defaults(component: impl Into<PathBuf>) -> Option<Self> {
        let base = NamedProjectDirs::new(component)?;

        let config = base.config_dir().join("config.toml");

        let log_dir = base.state_dir().to_owned();

        Some(Self { config, log_dir })
    }
}

/// Ratatui client UI.
#[derive(Debug, Parser, Serialize, Deserialize)]
#[command(author = "UserOfNames", version, about)]
struct Args {
    /// Print the default config file path and exit
    #[arg(long)]
    get_default_config_path: bool,

    /// Override the default config file path
    #[arg(long)]
    config_file: Option<PathBuf>,
}

/// Configuration for the UI runtime.
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    /// Whether to write logs to a file.
    log_to_file: bool,

    /// Directory to store the log file if `log_to_file` is true.
    log_dir: PathBuf,
}

/// The main application struct, including widgets, internal state, and communication channels to
/// the backend.
#[derive(Debug)]
struct App {
    /// State for the current server connection, if any.
    connection_state: Option<ConnectionState>,

    /// Channel for receiving events (or errors) from the backend.
    backend_receiver: Receiver<client_event::Result>,

    /// Channel for sending commands to the backend.
    backend_sender: Sender<ClientCommand>,

    /// Async stream of `Crossterm` events.
    event_stream: EventStream,

    /// Boolean flag set when the user requests to quit the application. This is used to determine
    /// whether a backend shutdown was intentional (Ok) or not (Err).
    is_quitting: bool,

    /// The main panel: an input area, a list of message, and a sidebar.
    main_panel: MainPanel,

    /// A stack of `Popup`s.
    popups: Vec<Box<dyn Popup>>,
}

impl App {
    /// Create a new `App`. Because the `App` must be able to communicate with a `ChatBackend`,
    /// that should be created first, and the relevant channels should be given to this method.
    fn new(receiver: Receiver<client_event::Result>, sender: Sender<ClientCommand>) -> Self {
        Self {
            connection_state: None,
            backend_receiver: receiver,
            backend_sender: sender,
            event_stream: EventStream::new(),
            is_quitting: false,
            main_panel: MainPanel::new(),
            popups: Vec::new(),
        }
    }

    /// Run the application.
    #[instrument(skip_all, err, parent = None)]
    async fn run(mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        // The UI may need to update without any incoming network events or crossterm events, so we
        // set up a periodic tick to render it even if nothing else is happening. 250 isn't a
        // meaningful number, just a reasonable default value.
        let mut render_interval = interval(Duration::from_millis(250));

        'app: loop {
            // This goes at the top of the loop instead of inside the `render_interval.tick()`
            // `select!` arm so that the UI is also responsive to events, not JUST the tick.
            match terminal.draw(|frame| self.draw(frame)) {
                Ok(_) => {}

                Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                    debug!("Draw interrupted, retrying");
                    continue 'app;
                }

                Err(e) => bail!("IO error while drawing terminal frame: {e}"),
            }

            tokio::select! {
                // TODO: In the input handling loop, check for double quit and force break.

                _ = render_interval.tick() => {}

                event = self.backend_receiver.recv() => {
                    match event {
                        Some(Ok(evt)) => self.handle_client_event(evt),

                        Some(Err(e)) => self.handle_client_event_error(e),

                        // The self.is_quitting flag is only set if the user explicitly requested
                        // to exit; otherwise, the backend closing was unexpected.
                        None if self.is_quitting => {
                            info!("Exiting cleanly");
                            return Ok(());
                        }

                        None => bail!("Client backend closed unexpectedly"),
                    }
                }

                event = self.event_stream.next() => {
                    match event {
                        Some(Ok(evt)) => self.handle_terminal_event(evt).await,

                        Some(Err(e)) => bail!("IO error while listening for terminal events: {e}"),

                        // We treat this as a forced close, not an error
                        None => return Ok(()),
                    }
                }
            }
        }
    }

    /// Draw a single frame to the terminal.
    fn draw(&mut self, frame: &mut Frame) {
        self.main_panel.render(
            frame.area(),
            frame.buffer_mut(),
            self.connection_state.as_ref(),
        );

        // Since popups are a stack, we only render the 'top' one.
        if let Some(popup) = self.popups.last() {
            let area = popup_area(frame.area(), popup.hint_size());

            frame.render_widget(Clear, area);
            popup.render(area, frame.buffer_mut());
        }
    }

    /// Handle a `ClientEvent` coming from the backend.
    #[instrument(skip_all, fields(event = %event.name()))]
    fn handle_client_event(&mut self, event: ClientEvent) {
        debug!("UI received event from backend");

        match event {
            ClientEvent::InitialSync(sync) => {
                info!(addr = %sync.server_addr, "Connected to server, initialized UI state");
                self.connection_state = Some(ConnectionState::new(sync));
            }

            ClientEvent::Disconnected => {
                info!("Disconnected from server, dropping UI state");
                self.notify("Disconnected".to_owned(), NoticeLevel::Notification);
                self.connection_state = None;
            }

            ClientEvent::ServerShutDown => {
                warn!("Server shut down while connected, dropping UI state");
                self.notify("The server shut down.".to_owned(), NoticeLevel::Warning);
                self.connection_state = None;
            }

            ClientEvent::ErrorEvent(error_event) => self.handle_error_event(error_event),

            // Remaining events should all be auto-routable to the ConnectionState instance. If not,
            // we failed to handle a special case in this match statement. If there is no
            // connection, we treat it as a NOP.
            _ => {
                if let Some(connection_state) = &mut self.connection_state {
                    connection_state.update_from_event(event);
                }
            }
        }
    }

    /// Handle an [`ErrorEvent`](chat_backend::network_protocol::ErrorEvent).
    fn handle_error_event(&mut self, error_event: ErrorEvent) {
        self.notify(error_event.to_string(), NoticeLevel::Error);
    }

    /// Handle a `client_event::Error` coming from the backend.
    #[instrument(skip(self))]
    fn handle_client_event_error(&mut self, error: client_event::Error) {
        warn!("Received error from client backend. Assuming the connection is dead.");
        let message = error.to_string();
        self.notify(message, NoticeLevel::Error);

        // If we received an error, we can assume the connection is dead.
        self.connection_state = None;
    }

    /// Handle a `Crossterm` event. This forwards to a more specific method.
    async fn handle_terminal_event(&mut self, event: Event) {
        // On some platforms (such as Windows), key releases are tracked separately from presses.
        // To prevent double-responses to a single press, we only respond to the initial press.
        if let Event::Key(k) = event
            && k.kind == KeyEventKind::Press
        {
            self.handle_key_event(k).await;
        }
    }

    /// Handle a `Crossterm` keyboard event.
    async fn handle_key_event(&mut self, key: KeyEvent) {
        // Popups take full priority over the main panel for key handling.
        let action = if let Some(popup) = self.popups.last_mut() {
            popup.handle_key(key)
        } else {
            self.main_panel.handle_key(key)
        };

        self.apply_action(action).await;
    }

    /// Apply an `Action` from a popup or the main panel handling a `Crossterm` event.
    async fn apply_action(&mut self, action: Action) {
        match action {
            Action::None => {}

            Action::Quit => self.quit().await,

            Action::PushPopup(popup) => self.popups.push(popup),

            Action::PopPopup => {
                self.popups.pop();
            }

            Action::Connect(params) => {
                let command = ClientCommand::Connect(params);
                self.send_to_backend(command).await;
                self.popups.clear();
            }

            Action::SendMessage(message) => {
                let Some(state) = &self.connection_state else {
                    self.notify(
                        "Cannot send message: not connected to a server".to_owned(),
                        NoticeLevel::Error,
                    );
                    return;
                };

                let destination = match &state.message_context {
                    Some(MessageContext::Channel(id)) => SendDestination::Channel(*id),
                    Some(MessageContext::User(id)) => SendDestination::User(*id),
                    None => {
                        self.notify(
                            "Cannot send message: no user or channel is selected.".to_owned(),
                            NoticeLevel::Error,
                        );
                        return;
                    }
                };

                let message = SendMessage {
                    contents: message,
                    destination,
                };

                let command = NetworkCommand::SendMessage(message);

                self.send_to_backend(ClientCommand::NetworkCommand(command))
                    .await;
            }

            Action::UpdateInfo(info) => {
                let command = NetworkCommand::UpdateInfo(info);
                self.send_to_backend(ClientCommand::NetworkCommand(command))
                    .await;

                self.popups.clear();
            }

            // Yielding at the top-level focus is a NOP
            Action::YieldFocus => {}

            Action::SelectChannel(id) => {
                // Selecting a channel when not connected should be impossible, but even if it
                // somehow happens, it's a NOP.
                let Some(state) = &mut self.connection_state else {
                    return;
                };

                state.message_context = Some(MessageContext::Channel(id));
            }

            Action::SelectUser(id) => {
                // Selecting a user when not connected is a NOP.
                let Some(state) = &mut self.connection_state else {
                    return;
                };

                state.message_context = Some(MessageContext::User(id));
            }
        }
    }

    /// Create a notification, warning, or error popup.
    fn notify(&mut self, message: String, level: NoticeLevel) {
        let notice = NoticePopup::create(message, level);
        self.popups.push(notice);
    }

    /// Request a clean exit.
    async fn quit(&mut self) {
        info!("Quit requested, attempting a clean exit");
        self.is_quitting = true;
        self.send_to_backend(ClientCommand::Quit).await;
    }

    /// Send a `ClientCommand` to the backend.
    #[instrument(skip_all, fields(command = %command.name()))]
    async fn send_to_backend(&mut self, command: ClientCommand) {
        debug!("Sending command to backend");

        // If this fails, the backend is already closed, and the next select! loop will detect
        // that. As such, we don't care about the Result here.
        let _: Result<_, _> = self.backend_sender.send(command).await;
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut figment = Figment::new().merge(Toml::string(DEFAULT_CONFIG));

    let default_paths = DefaultPaths::defaults("ratatui");

    if args.get_default_config_path {
        println!(
            "Config path: {}",
            default_paths
                .expect("Could not resolve default config path")
                .config
                .display()
        );

        return Ok(());
    }

    // TODO: Consider how to handle writing the default config file and implement that logic.
    let config_path = first_match! {
        Some(path) = &args.config_file => path.clone(),
        Some(defaults) = &default_paths => defaults.config.clone(),
    };

    if let Some(path) = &config_path {
        figment = figment.merge(Toml::file(path));
    }

    if let Some(defaults) = &default_paths {
        figment = figment.merge(Serialized::default("log_dir", &defaults.log_dir));
    }

    let config: Config = figment.extract().context("Resolving config")?;

    let _log_file_guard = init_logging(&config);
    info!(config_path = ?config_path, "UI config resolved");

    // HACK: Config path override disabled due to complexity of implementation. The backend will not
    // be a library forever, so it isn't worth it.
    let (backend, handle) = match ChatBackend::new(None) {
        Ok((b, h)) => (b, h),
        Err(e) => bail!("Failed to initialize backend: {e}"),
    };

    let app = App::new(handle.event_rx, handle.cmd_tx);

    let mut terminal = ratatui::init();

    let backend_task = tokio::spawn(backend.run());
    debug!("Backend initialized");

    info!("Starting UI");
    let app_result = app.run(&mut terminal).await;

    ratatui::restore();

    // Fallback to kill the backend in case the user force-quits while the backend is hanging. This
    // is a NOP if the backend task is already done, so this doesn't affect clean exits.
    backend_task.abort();

    app_result
}

fn init_logging(config: &Config) -> Option<WorkerGuard> {
    let (file_layer, file_guard) = if config.log_to_file {
        let appender = rolling::daily(&config.log_dir, "ui.log");
        let (appender, guard) = tracing_appender::non_blocking(appender);

        let layer = tracing_subscriber::fmt::layer()
            .with_writer(appender)
            .with_ansi(false);

        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry().with(file_layer).init();

    file_guard
}
