pub mod client_command;
pub mod client_event;
mod connection;

use std::io;
use std::ops::ControlFlow;

use network_protocol::{NetworkCommand, NetworkEvent};
use tokio::sync::mpsc::{self, Receiver, Sender};

use client_command::ClientCommand;
use client_event::ClientEvent;
use connection::Connection;

/// Contains channels through which to send `ClientCommand`s to the backend and from which to
/// receive `ClientEvent`s.
#[derive(Debug)]
pub struct BackendHandle {
    /// Sender for `ClientCommand`s.
    pub cmd_tx: Sender<ClientCommand>,
    /// Receiver of `ClientEvent`s.
    pub event_rx: Receiver<client_event::Result>,
}

/// The backend for the chat client. Frontends communicate with this via tokio channels by sending
/// `ClientCommand`s and receiving `ClientEvent`s.
///
/// To use the backend, first create it with `ChatBackend::new()`. Then, call the `run()` method.
/// For more information, see the documentation for those respective functions.
#[derive(Debug)]
pub struct ChatBackend {
    connection: Option<Connection>,
    cmd_rx: Receiver<ClientCommand>,
    event_tx: Sender<client_event::Result>,
}

impl ChatBackend {
    /// Create a new `ChatBackend` and a `BackendHandle` holding the necessary channels to
    /// communicate with the backend.
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

    /// Start the backend.
    ///
    /// Because this is an asynchronous function, it is recommended, if possible, to spawn it as a
    /// task. However, if this is not possible (for example, if the frontend expects a synchronous
    /// event loop), one approach is to spawn it in a separate thread using `block_on`, then use
    /// the channels' blocking methods when sending to/receiving from the backend.
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                // This structure is a bit odd, but it just makes it so we only listen for network
                // events if there's an active connection.
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

                command = self.cmd_rx.recv() => {
                    if let Some(cmd) = command {
                        if let ControlFlow::Break(()) = self.handle_command(cmd).await {
                            break;
                        }
                    } else {
                        self.handle_ui_crash().await;
                        break;
                    }
                }
            }
        }

        self.shutdown().await;
    }

    /// Handle any necessary logic after a UI crash, but before shutting down. Note that
    /// `self.shutdown()` is always called when the application is closing.
    async fn handle_ui_crash(&mut self) {
        todo!("Handle UI crash")
    }

    /// Handle a `ClientCommand` coming from the frontend.
    async fn handle_command(&mut self, command: ClientCommand) -> ControlFlow<()> {
        #[expect(clippy::match_wildcard_for_single_variants)]
        match command {
            // "Special case" arms.
            ClientCommand::Connect(addr) => self.connect(addr).await,
            ClientCommand::Disconnect => self.disconnect().await,
            ClientCommand::Quit => return ControlFlow::Break(()),

            // If we get here, all remaining ClientCommands should have a NetworkCommand
            // equivalent. This is for commands that only need to be passed to the server. If any
            // local handling is required, it should be handled in its own arm above. Between the
            // special arms and the `TryFrom` implementation on `NetworkCommand`, all possible
            // cases should be covered. If not, that must be fixed immediately, so we `expect()`
            // it.
            _ => {
                let command = NetworkCommand::try_from(command)
                    .expect("Improper conversion from ClientCommand to NetworkCommand. You did not handle a special case.");
                self.send_network_command(command).await;
            }
        }

        ControlFlow::Continue(())
    }

    /// Handle a `NetworkEvent` coming from the server.
    async fn handle_event(&mut self, event: NetworkEvent) {
        match event {
            NetworkEvent::ReceivedMessage(_) => self.send_ui_event(event.into()).await,
        }
    }

    /// Attempt to connect to the server at `addr`. The UI will be notified about whether the
    /// connection is successful or not.
    async fn connect(&mut self, addr: String) {
        let connection = match Connection::connect(&addr).await {
            Ok(conn) => conn,
            Err(e) => {
                self.send_ui_error(e.into()).await;
                return;
            }
        };

        self.connection = Some(connection);
        self.send_ui_event(ClientEvent::Connected(addr)).await;
    }

    /// Disconnect from the server.
    async fn disconnect(&mut self) {
        let Some(connection) = self.connection.take() else {
            // Disconnecting while already disconnected is a NOP
            return;
        };

        if let Err(e) = connection.disconnect().await {
            // TODO: Log error
        }

        // Even if the disconnected was not clean, by now, the connection has been consumed and
        // closed. As such, we unconditionally report success and only internally log the possible
        // error.
        self.send_ui_event(ClientEvent::Disconnected).await;
    }

    /// Send a `NetworkCommand` to the server. The UI will be notified if this fails.
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

    /// Send a `ClientEvent` to the UI.
    async fn send_ui_event(&mut self, event: ClientEvent) {
        if self.event_tx.send(Ok(event)).await.is_err() {
            self.handle_ui_crash().await;
            // TODO: Log error
        }
    }

    /// Send a `client_event::Error` to the UI.
    async fn send_ui_error(&mut self, error: client_event::Error) {
        // TODO: Log error
        if self.event_tx.send(Err(error)).await.is_err() {
            self.handle_ui_crash().await;
            // TODO: Log error
        }
    }

    /// Attempt a clean shutdown of the backend.
    async fn shutdown(mut self) {
        self.disconnect().await;
    }
}
