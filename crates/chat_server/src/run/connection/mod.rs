mod guard;

use std::sync::Arc;

use futures::{
    SinkExt, StreamExt,
    stream::{SelectAll, select_all},
};
use guard::ConnectionGuard;
use network_protocol::{
    ChannelSync, NetworkCommand, NetworkEvent, ReceiveDestination, ReceivedMessage,
    SendDestination, SendMessage, ServerHello, UpdateInfo, UserSync, codecs::ServerCodec,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{broadcast, mpsc},
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::{codec::Framed, sync::CancellationToken};

use crate::run::ServerState;

/// A connection task responsible for talking to one client.
#[derive(Debug)]
pub struct Connection {
    /// Shared server state.
    server_state: Arc<ServerState>,

    /// Stream of commands coming from the client, or sending back to the client.
    client_stream: Framed<TlsStream<TcpStream>, ServerCodec>,

    /// Channel for events broadcast to all users on the server.
    global_event_rx: broadcast::Receiver<NetworkEvent>,

    /// Channel for events coming from elsewhere on the server. Typically outbound towards the
    /// client.
    event_rx: mpsc::Receiver<NetworkEvent>,

    /// Unified receiver stream for all channels on the server.
    channels: SelectAll<BroadcastStream<NetworkEvent>>,

    /// Cancellation token for the main task to signal for shutdown.
    cancellation_token: CancellationToken,

    /// RAII guard to ensure the `Connection` unregisters from the `server_state` when it drops.
    guard: ConnectionGuard,
}

impl Connection {
    /// Open the connection with the client. This starts with a TLS handshake, then the main
    /// communication loop. This should be spawned as a separate [`tokio`] task using
    /// [`tokio::spawn`].
    ///
    /// Note that this function is completely self-contained. It is responsible for both
    /// initializing and running the `Connection` task. This is because the typical `new()` ->
    /// `run()` pattern involves the parent `Listener` in the handshake resolution, which both slows
    /// it down and potentially allows DDOS attacks.
    pub async fn start(
        server_state: Arc<ServerState>,
        tls_acceptor: TlsAcceptor,
        client_stream: TcpStream,
        cancellation_token: CancellationToken,
    ) {
        let client_stream = match tls_acceptor.accept(client_stream).await {
            Ok(stream) => stream,
            Err(e) => {
                // TODO: log error
                return;
            }
        };
        let mut client_stream = Framed::new(client_stream, ServerCodec);

        // We want to finish the ClientHello -> ServerHello handshake before anything else.
        // NOTE: For now, if the handshake fails for any reason, we just abort the connection
        // entirely. This keeps the implementation far simpler, at the cost of potentially repeating
        // the TLS handshake. If this becomes a problem later, we'll fix it later.
        let hello = match client_stream.next().await {
            Some(Ok(NetworkCommand::ClientHello(hello))) => hello,
            Some(Ok(other)) => todo!("Log error: bad handshake: unexpected command {other:?}"),
            Some(Err(e)) => todo!("Log error: bad handshake: error {e}"),
            None => todo!("Log error: bad handshake: stream closed"),
        };

        let (event_tx, event_rx) = mpsc::channel::<NetworkEvent>(128); // TODO: Buffer size

        let user_token = match server_state.handle_new_user(
            hello.requested_name,
            server_state.max_username_length(),
            event_tx,
        ) {
            Ok(token) => token,
            Err(e) => todo!("Report error, log error {e}"),
        };

        // It's important that we create the guard before any more fallible operations, since
        // `handle_new_user` touched persistent state.
        #[allow(clippy::used_underscore_binding)]
        let guard = ConnectionGuard::new(user_token, server_state.clone());

        // Send Hello to the client.
        if let Err(e) = client_stream
            .send(NetworkEvent::ServerHello(ServerHello {
                your_id: guard.id(),
                default_channel_id: server_state.default_channel_id(),
            }))
            .await
        {
            todo!("Log error: error sending ServerHello: {e}");
        }

        // Subscribe to all the server's channels
        let channels: select_all::SelectAll<_> = server_state
            .subscribe_to_channels()
            .into_iter()
            .map(BroadcastStream::from)
            .collect();

        // Subscribe to the global broadcast channel. We do this AFTER sending the join notification
        // because the client doesn't need to be reminded that they connected (they already know
        // that).
        let global_broadcast_rx = server_state.subscribe_to_global();

        let connection = Self {
            server_state,
            client_stream,
            global_event_rx: global_broadcast_rx,
            event_rx,
            channels,
            cancellation_token,
            guard,
        };

        connection.run().await;
    }

    /// Internal helper to actually run the connection task. Why make `Connection` a struct at all,
    /// instad of a pure function? Why have this chain of calls just to allow it to be a struct?
    /// Because I don't want to have to pass every variable in `self` to every single helper
    /// function, when I could just do this and call `self.helper()`. State structs are a good
    /// pattern, even if it's purely internal.
    async fn run(mut self) {
        loop {
            tokio::select! {
                // Commands from the client.
                network_cmd = self.client_stream.next() => match network_cmd {
                    Some(cmd) => match cmd {
                        Ok(cmd) => self.handle_command(cmd).await,
                        Err(e) => todo!("Log error, report to sender {e}"),
                    }

                    None => {
                        // TODO: Log disconnect
                        break;
                    }
                },

                // Global events.
                event = self.global_event_rx.recv() => match event {
                    Ok(event) => self.send_event_to_client(event).await,
                    Err(e) => todo!("Log error receiving global event: {e}"),
                },

                // Direct messages.
                direct_msg = self.event_rx.recv() => match direct_msg {
                    Some(msg) => self.send_event_to_client(msg).await,
                    None => todo!(),
                },

                // Channel messages.
                // TODO: Do I need the channel ID at all, or can I remove it to save space?
                Some(result) = self.channels.next() => {
                    match result {
                        Ok(msg) => self.send_event_to_client(msg).await,
                        Err(e) => todo!("Log error, report to sender {e}"),
                    }
                }

                // Cancellation signal.
                () = self.cancellation_token.cancelled() => {
                    if let Err(e) = self.client_stream.flush().await {
                        todo!("Log error: failed to flush on shutdown {e}");
                    }

                    if let Err(e) = self.client_stream.into_inner().shutdown().await {
                        todo!("Log error: failed to shutdown {e}");
                    }

                    break;
                }
            }
        }
    }

    async fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::ClientHello(_) => todo!("Log error: double hello"),

            NetworkCommand::FetchChannels(_fetch) => {
                self.send_event_to_client(NetworkEvent::ChannelSync(ChannelSync {
                    channels: self.server_state.get_all_channel_info(),
                }))
                .await;
            }

            NetworkCommand::FetchUsers(_fetch) => {
                self.send_event_to_client(NetworkEvent::UserSync(UserSync {
                    users: self.server_state.get_all_user_info(),
                }))
                .await;
            }

            NetworkCommand::SendMessage(msg) => self.send_message(msg).await,

            NetworkCommand::UpdateInfo(info) => self.update_info(info),
        }
    }

    async fn send_event_to_client(&mut self, event: NetworkEvent) {
        if let Err(e) = self.client_stream.send(event).await {
            todo!("Log error, report to sender {e}");
        }
    }

    #[allow(clippy::unused_async)]
    async fn send_message(&mut self, message: SendMessage) {
        let SendMessage {
            destination,
            contents,
        } = message;

        match destination {
            SendDestination::Channel(channel_id) => {
                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.guard.id(),
                    destination: ReceiveDestination::Channel(channel_id),
                });

                self.server_state.send_event_to_channel(channel_id, event);
            }

            SendDestination::User(target_user_id) => {
                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.guard.id(),
                    destination: ReceiveDestination::User(target_user_id),
                });

                self.server_state
                    .send_event_to_user(target_user_id, event.clone())
                    .await;

                // Can't use send_event_to_client here due to a borrow checker conflict.
                // We send back to the sender as well to include them in the loopback, such that
                // they can render their own message in the correct order relative to other messages.
                // However, if the sender is sending to themselves (a "note to self"), this would
                // result in a double send. As such, we filter that case out.
                if target_user_id != self.guard.id()
                    && let Err(e) = self.client_stream.send(event).await
                {
                    todo!("Log error: sender disconnected while sending DM {e}");
                }
            }
        }
    }

    fn update_info(&mut self, new_info: UpdateInfo) {
        if let Err(e) = self.server_state.update_user_info(
            self.guard.token(),
            new_info,
            self.server_state.max_username_length(),
        ) {
            todo!("Log and report error: {e}");
        }
    }
}
