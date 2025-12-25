use tokio::sync::mpsc::{Receiver, Sender};

use chat_backend::ChatBackend;
use chat_backend::client_command::ClientCommand;
use chat_backend::client_event::{self, ClientEvent};

struct App {
    backend_receiver: Receiver<client_event::Result>,
    backend_sender: Sender<ClientCommand>,
}

impl App {
    fn new(receiver: Receiver<client_event::Result>, sender: Sender<ClientCommand>) -> Self {
        Self {
            backend_receiver: receiver,
            backend_sender: sender,
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                event = self.backend_receiver.recv() => {
                    match event {
                        Some(Ok(evt)) => self.handle_event(evt).await,
                        Some(Err(e)) => self.handle_event_error(e).await,
                        None => break, // This indicates the backend has shut down
                    }
                }
            }
        }
    }

    async fn handle_event(&mut self, event: ClientEvent) {
        todo!("Implement event handling");
    }

    async fn handle_event_error(&self, error: client_event::Error) {
        todo!("Implement event errors");
    }
}

#[tokio::main]
async fn main() -> Result<(), tokio::task::JoinError> {
    let (backend, handle) = ChatBackend::new();
    let app = App::new(handle.event_rx, handle.cmd_tx);

    let backend_task = tokio::spawn(backend.run());

    app.run().await;

    // This is a NOP if the backend task is already done, so this doesn't affect clean exits.
    backend_task.abort();

    Ok(())
}
