mod ui;

use std::io;

use crossterm::event::{Event, EventStream, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    widgets::Clear,
};
use thiserror::Error;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{Duration, interval},
};

use chat_backend::ChatBackend;
use chat_backend::client_command::ClientCommand;
use chat_backend::client_event::{self, ClientEvent};

use ui::{
    Action, KeyHandler,
    main_panel::MainPanel,
    popups::{
        Popup,
        notice::{NoticeLevel, NoticePopup},
        popup_area,
    },
};

#[derive(Debug, Error)]
enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Backend died unexpectedly")]
    BackendDied,
}

type AppResult = Result<(), AppError>;

/// The main application struct, including widgets, internal state, and communication channels to
/// the backend.
#[derive(Debug)]
struct App {
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
            backend_receiver: receiver,
            backend_sender: sender,
            event_stream: EventStream::new(),
            is_quitting: false,
            main_panel: MainPanel::new(),
            popups: Vec::new(),
        }
    }

    /// Run the application.
    async fn run(mut self, terminal: &mut DefaultTerminal) -> AppResult {
        // The UI may need to update without any incoming network events or crossterm events, so we
        // set up a periodic tick to render it even if nothing else is happening. 250 isn't a
        // meaningful number, just a reasonable default value.
        let mut render_interval = interval(Duration::from_millis(250));

        loop {
            // This goes at the top of the loop instead of inside the `render_interval.tick()`
            // `select!` arm so that the UI is also responsive to events, not JUST the tick.
            match terminal.draw(|frame| self.draw(frame)) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }

            tokio::select! {
                // TODO: In the input handling loop, check for double quit and force break.

                _ = render_interval.tick() => {}

                event = self.backend_receiver.recv() => {
                    match event {
                        Some(Ok(evt)) => self.handle_client_event(evt).await,
                        Some(Err(e)) => self.handle_client_event_error(e).await,

                        // The self.is_quitting flag is only set if the user explicitly requested
                        // to exit; otherwise, the backend closing was unexpected.
                        None if self.is_quitting => return Ok(()),
                        None => return Err(AppError::BackendDied),
                    }
                }

                event = self.event_stream.next() => {
                    match event {
                        Some(Ok(evt)) => self.handle_terminal_event(evt).await,
                        Some(Err(e)) => todo!(),
                        None => todo!(),
                    }
                }
            }
        }
    }

    /// Draw a single frame to the terminal.
    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(&self.main_panel, frame.area());

        // Since popups are a stack, we only render the 'top' one.
        if let Some(popup) = self.popups.last() {
            let (x_percent, y_percent) = popup.hint_size();

            let area = popup_area(frame.area(), x_percent, y_percent);

            frame.render_widget(Clear, area);
            popup.render(area, frame.buffer_mut());
        }
    }

    /// Handle a `ClientEvent` coming from the backend.
    async fn handle_client_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::Connected(addr) => {
                self.main_panel.connect(addr);
            }

            ClientEvent::Disconnected => {
                self.notify("Disconnected".to_owned(), NoticeLevel::Notification)
                    .await;
                self.main_panel.disconnect();
            }

            ClientEvent::ReceivedMessage(msg) => {
                self.main_panel.add_message(msg);
            }
        }
    }

    /// Handle a `client_event::Error` coming from the backend.
    async fn handle_client_event_error(&mut self, error: client_event::Error) {
        let message = error.to_string();
        self.notify(message, NoticeLevel::Error).await;
    }

    /// Handle a `Crossterm` event. This forwards to a more specific method.
    async fn handle_terminal_event(&mut self, event: Event) {
        // On some platforms (such as Windows), key releases are tracked separately from presses.
        // To prevent double-responses to a single press, we only respond to the initial press.
        if let Event::Key(k) = event
            && k.kind == KeyEventKind::Press // Check press vs. release
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

            Action::Connect(addr) => {
                self.send_to_backend(ClientCommand::Connect(addr)).await;
                self.popups.clear();
            }

            Action::SendMessage(msg) => {
                self.send_to_backend(ClientCommand::SendMessage(msg)).await;
            }
        }
    }

    /// Create a notification, warning, or error popup.
    async fn notify(&mut self, message: String, level: NoticeLevel) {
        let notice = NoticePopup::new(message, level);
        self.popups.push(Box::new(notice));
    }

    /// Request a clean exit.
    async fn quit(&mut self) {
        self.is_quitting = true;
        self.send_to_backend(ClientCommand::Quit).await;
    }

    /// Send a `ClientCommand` to the backend.
    async fn send_to_backend(&mut self, command: ClientCommand) {
        // If this fails, the backend is already closed, and the next select! loop will detect
        // that. As such, we don't care about the Result here.
        let _: Result<_, _> = self.backend_sender.send(command).await;
    }
}

#[tokio::main]
async fn main() -> AppResult {
    let (backend, handle) = ChatBackend::new();
    let app = App::new(handle.event_rx, handle.cmd_tx);

    let mut terminal = ratatui::init();

    let backend_task = tokio::spawn(backend.run());
    let app_result = app.run(&mut terminal).await;

    ratatui::restore();

    // Fallback to kill the backend in case the user force-quits while the backend is hanging. This
    // is a NOP if the backend task is already done, so this doesn't affect clean exits.
    backend_task.abort();

    app_result
}
