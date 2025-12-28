use std::io;

use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::StreamExt;
use ratatui::{
    layout::{Constraint, Direction, Layout}, widgets::Block, DefaultTerminal, Frame
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

#[derive(Debug, PartialEq, Eq)]
enum Focus {
    None,
    TextBox,
}

#[derive(Debug)]
struct App<'a> {
    backend_receiver: Receiver<client_event::Result>,
    backend_sender: Sender<ClientCommand>,
    event_stream: EventStream,
    is_quitting: bool,
    focus: Focus,
    textbox: TextArea<'a>,
}

#[derive(Debug, Error)]
enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Backend died unexpectedly")]
    BackendDied,
}

type AppResult = Result<(), AppError>;

impl App<'_> {
    fn new(receiver: Receiver<client_event::Result>, sender: Sender<ClientCommand>) -> Self {
        let block = Block::bordered().title("Input");
        let mut textbox = TextArea::default();
        textbox.set_block(block);

        Self {
            backend_receiver: receiver,
            backend_sender: sender,
            event_stream: EventStream::new(),
            is_quitting: false,
            focus: Focus::None,
            textbox,
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
        let outer_splits = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(frame.area());

        let message_part = outer_splits[0];
        let sidebar = outer_splits[1];

        let message_part_splits = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(90), Constraint::Percentage(10)])
            .split(message_part);

        let messages = message_part_splits[0];
        let input = message_part_splits[1];

        frame.render_widget("sidebar", sidebar);
        frame.render_widget("messages", messages);
        frame.render_widget(&self.textbox, input);
    }

    async fn handle_client_event(&mut self, event: ClientEvent) {
        todo!("Implement event handling");
    }

    async fn handle_client_event_error(&self, error: client_event::Error) {
        todo!("Implement event errors");
    }

    async fn handle_terminal_event(&mut self, event: Event) {
        if let Event::Key(k) = event
            && k.kind == KeyEventKind::Press
        {
            self.handle_key_event(k).await;
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) {
        match self.focus {
            Focus::None => match key.code {
                KeyCode::Esc => self.quit().await,
                KeyCode::Char('i') => self.focus = Focus::TextBox,
                _ => {}
            },

            Focus::TextBox => match key.code {
                KeyCode::Esc => self.focus = Focus::None,
                _ => {
                    self.textbox.input(key);
                }
            },
        }
    }

    async fn quit(&mut self) {
        self.is_quitting = true;

        // If this fails, the backend is already closed, and the next select! loop will detect
        // that. As such, we don't care about the Result here.
        let _: Result<_, _> = self.backend_sender.send(ClientCommand::Quit).await;
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
