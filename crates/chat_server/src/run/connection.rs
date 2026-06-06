use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use network_protocol::{
    ChannelSync, NetworkCommand, NetworkEvent, ReceiveDestination, ReceivedMessage,
    SendDestination, SendMessage, ServerHello, UpdateInfo, UserInfo, UserSync, codecs::ServerCodec,
};
use tokio::{
    io::AsyncWriteExt,
    net::TcpStream,
    sync::{broadcast, mpsc},
};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use tokio_stream::{StreamMap, wrappers::BroadcastStream};
use tokio_util::{codec::Framed, sync::CancellationToken};
use uuid::Uuid;

use crate::run::{ChannelId, ServerState, User, UserId};

/// RAII guard that automatically unregisters a user when dropped.
#[derive(Debug)]
struct ConnectionGuard {
    user_id: UserId,
    server_state: Arc<ServerState>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        if let Some((_, user)) = self.server_state.users.remove(&self.user_id) {
            self.server_state.taken_names.remove(&user.info.name);
        }

        // The only failure condition for sending through a broadcast channel is if there are no
        // receivers, but we don't actually care if nobody gets this message. As such, we ignore
        // this error.
        let _: Result<_, _> = self
            .server_state
            .global_broadcast
            .send(NetworkEvent::UserLeft(self.user_id));
    }
}

/// A connection task responsible for talking to one client.
#[derive(Debug)]
pub struct Connection {
    /// ID of the user associated with this connection
    user_id: UserId,

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
    // TODO: Do I need the channel ID at all, or can I remove it to save space?
    channels: StreamMap<ChannelId, BroadcastStream<NetworkEvent>>,

    /// Cancellation token for the main task to signal for shutdown.
    cancellation_token: CancellationToken,

    /// RAII guard to ensure the `Connection` unregisters from the `server_state` when it drops.
    _guard: ConnectionGuard,
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
        let user_id = UserId(Uuid::now_v7());

        // It's important that we create the guard before anything else, or else there may be a gap
        // that allows ghost state to accumulate.
        #[allow(clippy::used_underscore_binding)]
        let _guard = ConnectionGuard {
            user_id,
            server_state: server_state.clone(),
        };

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

        // Atomically insert our name + check if it's already taken.
        if !server_state
            .taken_names
            .insert(hello.requested_name.clone())
        {
            todo!("Report taken username to client, log, abort");
        }

        // Send Hello to the client.
        if let Err(e) = client_stream
            .send(NetworkEvent::ServerHello(ServerHello {
                your_id: user_id,
                default_channel_id: server_state.default_channel_id,
            }))
            .await
        {
            todo!("Log error: error sending ServerHello: {e}");
        }

        // Subscribe to all the server's channels
        let channels: StreamMap<_, _> = server_state
            .channels
            .iter()
            .map(|pair| {
                let key = *pair.key();
                let stream = BroadcastStream::from(pair.value().broadcast.subscribe());
                (key, stream)
            })
            .collect();

        let (event_tx, event_rx) = mpsc::channel(128); // TODO: Buffer size

        let user_info = UserInfo {
            id: user_id,
            name: hello.requested_name,
        };

        let user = User {
            info: user_info.clone(),
            sender: event_tx,
        };

        // Register this connection in the ServerState
        server_state.users.insert(user_id, user);

        // Notify all other users that you've joined. The only failure condition for sending through
        // a broadcast channel is if there are no receivers, but we don't actually care if nobody
        // gets this message. As such, we ignore this error.
        let _: Result<_, _> = server_state
            .global_broadcast
            .send(NetworkEvent::UserJoined(user_info));

        // Subscribe to the global broadcast channel. We do this AFTER sending the join notification
        // because the client doesn't need to be reminded that they connected (they already know
        // that).
        let global_broadcast_rx = server_state.global_broadcast.subscribe();

        let connection = Self {
            user_id,
            server_state,
            client_stream,
            global_event_rx: global_broadcast_rx,
            event_rx,
            channels,
            cancellation_token,
            _guard,
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
                Some((channel_id, result)) = self.channels.next() => {
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
                    channels: self
                        .server_state
                        .channels
                        .iter()
                        .map(|entry| entry.info.clone())
                        .collect(),
                }))
                .await;
            }

            NetworkCommand::FetchUsers(_fetch) => {
                self.send_event_to_client(NetworkEvent::UserSync(UserSync {
                    users: self
                        .server_state
                        .users
                        .iter()
                        .map(|entry| entry.info.clone())
                        .collect(),
                }))
                .await;
            }

            NetworkCommand::SendMessage(msg) => self.send_message(msg).await,

            NetworkCommand::UpdateInfo(info) => self.update_info(info),
        }
    }

    async fn send_event_to_client(&mut self, event: NetworkEvent) {
        if let Err(e) = self.client_stream.send(event).await {
            todo!("Log error, report to sender");
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
                let Some(channel) = self.server_state.channels.get(&channel_id) else {
                    todo!("Log error, report to sender");
                };

                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.user_id,
                    destination: ReceiveDestination::Channel(channel_id),
                });

                // Sending returns an error if there are no subscribed listeners (we don't care
                // about that), or if the channel is closed. The channel can never close, because
                // the Sender side (which is responsible for dropping) is held in the state struct,
                // which is held by tasks that last for the full duration of the program. So we just
                // ignore this error.
                let _: Result<_, _> = channel.broadcast.send(event);
            }

            SendDestination::User(target_user_id) => {
                let Some(user) = self.server_state.users.get(&target_user_id) else {
                    todo!("Log error, report to sender");
                };

                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.user_id,
                    destination: ReceiveDestination::User(target_user_id),
                });

                if let Err(e) = user.sender.send(event.clone()).await {
                    todo!("Log error, report to sender {e}");
                }

                // Can't use send_event_to_client here due to a borrow checker conflict.
                // We send back to the sender as well to include them in the loopback, such that
                // they can render their own message in the correct order relative to other messages.
                // However, if the sender is sending to themselves (a "note to self"), this would
                // result in a double send. As such, we filter that case out.
                if target_user_id != self.user_id
                    && let Err(e) = self.client_stream.send(event).await
                {
                    todo!("Log error: sender disconnected while sending DM {e}");
                }
            }
        }
    }

    fn update_info(&mut self, new_info: UpdateInfo) {
        let Some(mut user_entry) = self.server_state.users.get_mut(&self.user_id) else {
            todo!("Log error: unexplainable state mismatch? Probably unrecoverable?");
        };

        if let Some(new_name) = new_info.name {
            if !self.server_state.taken_names.insert(new_name.clone()) {
                todo!("Report taken username to client");
            }

            user_entry.info.name = new_name;
        }

        // The user's input was updated in-place, so it's now fully updated. We clone it out to send
        // back to the client for the update event.
        let event = NetworkEvent::UserInfoUpdated(user_entry.info.clone());

        // The only failure condition for sending through a broadcast channel is if there are no
        // receivers, but we don't actually care if nobody gets this message. As such, we ignore
        // this error.
        let _: Result<_, _> = self.server_state.global_broadcast.send(event);
    }
}
