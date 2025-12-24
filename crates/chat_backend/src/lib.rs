pub mod client_command;
pub mod client_event;
mod connection;

use std::io;

use network_protocol::{NetworkCommand, NetworkEvent};
use tokio::sync::mpsc::{self, Receiver, Sender};

use client_command::ClientCommand;
use client_event::ClientEvent;
use connection::Connection;

#[derive(Debug)]
pub struct BackendHandle {
    pub cmd_tx: Sender<ClientCommand>,
    pub event_rx: Receiver<client_event::Result>,
}

#[derive(Debug)]
pub struct ChatBackend {
    connection: Option<Connection>,
    cmd_rx: Receiver<ClientCommand>,
    event_tx: Sender<client_event::Result>,
}

impl ChatBackend {
    #[must_use]
    pub fn new() -> (Self, BackendHandle) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<ClientCommand>(128); // TODO: Buffer size
        let (event_tx, event_rx) = mpsc::channel::<client_event::Result>(128); // TODO: Buffer size

        let controller = BackendHandle { cmd_tx, event_rx };

        let backend = Self {
            connection: None,
            cmd_rx,
            event_tx,
        };

        (backend, controller)
    }

    pub async fn run(mut self) -> io::Result<()> {
        loop {
            tokio::select! {
                Some(event_result) = async {
                    match self.connection.as_mut() {
                        Some(conn) => conn.receive_event().await,
                        None => None,
                    }
                } => {
                    match event_result {
                        Ok(event) => self.handle_event(event).await,
                        Err(e) => {
                            self.send_ui_error(client_event::Error::Io(e)).await;
                        }
                    }
                }

                Some(command) = self.cmd_rx.recv() => self.handle_command(command).await,

                // TODO: Consider an explicit else case. If the channel to the frontend (cmd_rx) is
                // lost, there's really nothing better to do than abort anyways, so panicking
                // (default behavior when all branches cancel) is fine. However, we can probably do
                // a better job logging the error, and maybe doing some cleanup. We may also need
                // to handle the case where the frontend is lost, but the connection is still live.
            }
        }
    }

    async fn handle_command(&mut self, command: ClientCommand) {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match command {
            ClientCommand::Connect(addr) => self.connect(addr).await,
            ClientCommand::Disconnect => self.disconnect().await,

            // If we get here, all remaining ClientCommands should have a NetworkComman equivalent
            _ => {
                let command = NetworkCommand::try_from(command)
                    .expect("Improper conversion from ClientCommand to NetworkCommand. You did not handle a special case.");
                self.send_network_command(command).await;
            }
        }
    }

    async fn handle_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::ReceivedMessage(_) => self.send_ui_event(event.into()).await,
        }
    }

    async fn connect(&mut self, addr: String) {
        let connection = match Connection::connect(addr).await {
            Ok(conn) => conn,
            Err(e) => {
                self.send_ui_error(e.into()).await;
                return;
            }
        };

        self.connection = Some(connection);
        self.send_ui_event(ClientEvent::Connected).await;
    }

    async fn disconnect(&mut self) {
        let Some(connection) = self.connection.take() else {
            // Disconnecting while already disconnected is a NOP
            return;
        };

        if let Err(e) = connection.disconnect().await {
            // TODO: Log error
        }

        self.send_ui_event(ClientEvent::Disconnected).await;
    }

    async fn send_network_command(&mut self, command: NetworkCommand) {
        let Some(connection) = &mut self.connection else {
            // TODO: Log error
            let kind = io::ErrorKind::NotConnected;
            let error = io::Error::from(kind);
            self.send_ui_error(error.into()).await;
            return;
        };

        if let Err(e) = connection.send_command(command).await {
            // TODO: Log error
            self.send_ui_error(e.into()).await;
        }
    }

    async fn send_ui_event(&mut self, event: ClientEvent) {
        // TODO: Log error
        self.event_tx.send(Ok(event))
            .await
            .expect("UI channel closed. This indicates that the UI ungracefully failed without the backend.");
    }

    async fn send_ui_error(&mut self, error: client_event::Error) {
        // TODO: Log error
        self.event_tx.send(Err(error))
            .await
            .expect("UI channel closed. This indicates that the UI ungracefully failed without the backend.");
    }
}
