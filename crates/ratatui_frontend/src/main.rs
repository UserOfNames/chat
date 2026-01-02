mod ui;

use std::io;

use crossterm::event::{Event, EventStream, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Clear},
};
use thiserror::Error;
use tokio::{
    sync::mpsc::{Receiver, Sender},
    time::{Duration, interval},
};
use tui_textarea::TextArea;

use chat_backend::ChatBackend;
use chat_backend::client_command::ClientCommand;
use chat_backend::client_event::{self, ClientEvent};

use ui::{
    Action, KeyHandler,
    focus::Focus,
    popups::{
        Popup,
        notice::{NoticeLevel, NoticePopup},
        popup_area,
    },
    sidebar::Sidebar,
    messages::Messages,
};

#[derive(Debug, Error)]
enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Backend died unexpectedly")]
    BackendDied,
}

type AppResult = Result<(), AppError>;

#[derive(Debug)]
struct App<'a> {
    backend_receiver: Receiver<client_event::Result>,
    backend_sender: Sender<ClientCommand>,
    event_stream: EventStream,
    is_quitting: bool,
    focus: Focus,
    popups: Vec<Box<dyn Popup>>,
    textbox: TextArea<'a>,
    sidebar: Sidebar,
    messages: Messages,
}

impl App<'_> {
    fn new(receiver: Receiver<client_event::Result>, sender: Sender<ClientCommand>) -> Self {
        let block = Block::bordered().title(" Input ");
        let mut textbox = TextArea::default();
        textbox.set_block(block);

        Self {
            backend_receiver: receiver,
            backend_sender: sender,
            event_stream: EventStream::new(),
            is_quitting: false,
            focus: Focus::Normal,
            popups: Vec::new(),
            textbox,
            sidebar: Sidebar::new(),
            messages: Messages::new(),
        }
    }

    async fn run(mut self, terminal: &mut DefaultTerminal) -> AppResult {
        let mut render_interval = interval(Duration::from_millis(250));

        loop {
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

    fn draw(&self, frame: &mut Frame) {
        let [message_part, sidebar] = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(80), Constraint::Percentage(20)])
            .areas(frame.area());

        let [messages, input] = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(75), Constraint::Percentage(25)])
            .areas(message_part);

        frame.render_widget(&self.sidebar, sidebar);
        frame.render_widget(&self.messages, messages);
        frame.render_widget(&self.textbox, input);

        if let Some(popup) = self.popups.last() {
            let (x_percent, y_percent) = popup.hint_size();

            let area = popup_area(frame.area(), x_percent, y_percent);

            frame.render_widget(Clear, area);
            popup.render(area, frame.buffer_mut());
        }
    }

    async fn handle_client_event(&mut self, event: ClientEvent) {
        match event {
            ClientEvent::Connected => {
                self.notify("Connected".to_owned(), NoticeLevel::Notification)
                    .await
            }

            ClientEvent::Disconnected => {
                self.notify("Disconnected".to_owned(), NoticeLevel::Notification)
                    .await
            }

            ClientEvent::ReceivedMessage(msg) => {
                // TODO: Implement message area
                self.notify(msg, NoticeLevel::Notification)
                    .await
            }
        }
    }

    async fn handle_client_event_error(&mut self, error: client_event::Error) {
        let message = error.to_string();
        self.notify(message, NoticeLevel::Error).await;
    }

    async fn handle_terminal_event(&mut self, event: Event) {
        if let Event::Key(k) = event
            && k.kind == KeyEventKind::Press
        {
            self.handle_key_event(k).await;
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) {
        let action = if let Some(popup) = self.popups.last_mut() {
            popup.handle_key(key)
        } else {
            self.focus.handle_key(key)
        };

        self.apply_action(action).await;
    }

    async fn apply_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Quit => self.quit().await,
            Action::PushPopup(popup) => self.popups.push(popup),
            Action::ChangeFocus(focus) => self.focus = focus,

            Action::PopPopup => {
                self.popups.pop();
            }

            Action::ForwardToInput(key) => {
                self.textbox.input(key);
            }

            Action::Connect(addr) => {
                self.send_to_backend(ClientCommand::Connect(addr)).await;
                self.popups.clear();
            }

            Action::SendMessage => {
                let message = self.textbox.lines().join("");
                self.send_to_backend(ClientCommand::SendMessage(message)).await;
            }
        }
    }

    async fn notify(&mut self, message: String, level: NoticeLevel) {
        let notice = NoticePopup::new(message, level);
        self.popups.push(Box::new(notice));
    }

    async fn quit(&mut self) {
        self.is_quitting = true;
        self.send_to_backend(ClientCommand::Quit).await;
    }

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
