use std::io;

use ratatui::{DefaultTerminal, Frame};
use thiserror::Error;
use tokio::sync::mpsc::{Receiver, Sender};

use chat_backend::ChatBackend;
use chat_backend::client_command::ClientCommand;
use chat_backend::client_event::{self, ClientEvent};

struct App {
    backend_receiver: Receiver<client_event::Result>,
    backend_sender: Sender<ClientCommand>,
    is_quitting: bool,
}

#[derive(Debug, Error)]
enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("Backend connection lost")]
    BackendDied,
}

type AppResult = Result<(), AppError>;

impl App {
    fn new(receiver: Receiver<client_event::Result>, sender: Sender<ClientCommand>) -> Self {
        Self {
            backend_receiver: receiver,
            backend_sender: sender,
            is_quitting: false,
        }
    }

    async fn run(mut self, terminal: &mut DefaultTerminal) -> AppResult {
        loop {
            match terminal.draw(|frame| self.draw(frame)) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            }

            tokio::select! {
                // TODO: In the input handling loop, check for double quit and force break.

                event = self.backend_receiver.recv() => {
                    match event {
                        Some(Ok(evt)) => self.handle_event(evt).await,
                        Some(Err(e)) => self.handle_event_error(e).await,
                        None if self.is_quitting => return Ok(()),
                        None => return Err(AppError::BackendDied),
                    }
                }
            }
        }
    }

    fn draw(&self, terminal: &mut Frame) {
        todo!("Implement drawing");
    }

    async fn handle_event(&mut self, event: ClientEvent) {
        todo!("Implement event handling");
    }

    async fn handle_event_error(&self, error: client_event::Error) {
        todo!("Implement event errors");
    }

    async fn quit(&mut self) {
        self.is_quitting = true;

        // If this fails, the backend is already closed, and the next select! loop will detect
        // that. As such, we don't care about the Result here.
        let _: Result<_, _> = self.backend_sender.send(ClientCommand::Quit).await;
    }
}

#[tokio::main]
async fn main() -> Result<(), tokio::task::JoinError> {
    let (backend, handle) = ChatBackend::new();
    let app = App::new(handle.event_rx, handle.cmd_tx);

    let mut terminal = ratatui::init();

    let backend_task = tokio::spawn(backend.run());
    app.run(&mut terminal).await;

    ratatui::restore();

    // Fallback to kill the backend in case the user force-quits while the backend is hanging. This
    // is a NOP if the backend task is already done, so this doesn't affect clean exits.
    backend_task.abort();

    Ok(())
}
