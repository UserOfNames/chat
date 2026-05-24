use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use network_protocol::{
    ChannelSync, NetworkCommand, NetworkEvent, ReceiveDestination, ReceivedMessage,
    SendDestination, SendMessage, ServerHello, UserSync, codecs::ServerCodec,
};
use rand::distr::{Alphanumeric, SampleString};
use tokio::{io::AsyncWriteExt, net::TcpStream, sync::mpsc};
use tokio_rustls::{TlsAcceptor, server::TlsStream};
use tokio_stream::{StreamMap, wrappers::BroadcastStream};
use tokio_util::{codec::Framed, sync::CancellationToken};

use crate::run::{ChannelId, ServerState, UserId};

/// RAII guard that automatically unregisters a user when dropped.
#[derive(Debug)]
struct ConnectionGuard {
    user_id: UserId,
    server_state: Arc<ServerState>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.server_state.users.remove(&self.user_id);
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

    /// Channel for events coming from elsewhere on the server. Typically outbound towards the
    /// client.
    event_rx: mpsc::Receiver<NetworkEvent>,

    /// Unified receiver stream for all channels on the server.
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
        let mut user_id = "DEFAULT-".to_owned();

        Alphanumeric.append_string(&mut rand::rng(), &mut user_id, 1);
        while server_state.users.contains_key(&user_id) {
            Alphanumeric.append_string(&mut rand::rng(), &mut user_id, 1);
        }

        let client_stream = match tls_acceptor.accept(client_stream).await {
            Ok(stream) => stream,
            Err(e) => {
                // TODO: log error
                return;
            }
        };
        let client_stream = Framed::new(client_stream, ServerCodec);

        // It's important that we create the guard before registering, or else there is a gap
        // between when the connection is registered and when the guard is active
        #[allow(clippy::used_underscore_binding)]
        let _guard = ConnectionGuard {
            user_id: user_id.clone(),
            server_state: server_state.clone(),
        };

        let (event_tx, event_rx) = mpsc::channel(128); // TODO: Buffer size

        // Subscribe to all the server's channels
        let channels: StreamMap<_, _> = server_state
            .channels
            .iter()
            .map(|pair| {
                let key = pair.key().clone();
                let stream = BroadcastStream::from(pair.value().subscribe());
                (key, stream)
            })
            .collect();

        // Register this connection in the ServerState
        server_state.users.insert(user_id.clone(), event_tx);

        let connection = Self {
            user_id,
            server_state,
            client_stream,
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
                network_cmd = self.client_stream.next() => match network_cmd {
                    Some(cmd) => match cmd {
                        Ok(cmd) => self.handle_command(cmd).await,
                        Err(e) => todo!("Log error, report to sender"),
                    }

                    None => {
                        // TODO: Log disconnect
                        break;
                    }
                },

                direct_msg = self.event_rx.recv() => match direct_msg {
                    Some(msg) => self.send_event_to_client(msg).await,
                    None => todo!(),
                },

                Some((channel_name, result)) = self.channels.next() => {
                    match result {
                        Ok(msg) => self.send_event_to_client(msg).await,
                        Err(e) => todo!("Log error, report to sender"),
                    }
                }

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
            NetworkCommand::ClientHello => {
                self.send_event_to_client(NetworkEvent::ServerHello(ServerHello {
                    your_id: self.user_id.clone(),
                    default_channel_id: self.server_state.default_channel_id.clone(),
                }))
                .await;
            }

            NetworkCommand::FetchChannels(fetch) => {
                self.send_event_to_client(NetworkEvent::ChannelSync(ChannelSync {
                    channel_ids: self
                        .server_state
                        .channels
                        .iter()
                        .map(|entry| entry.key().clone())
                        .collect(),
                }))
                .await;
            }

            NetworkCommand::FetchUsers(fetch) => {
                self.send_event_to_client(NetworkEvent::UserSync(UserSync {
                    user_ids: self
                        .server_state
                        .users
                        .iter()
                        .map(|entry| entry.key().clone())
                        .collect(),
                }))
                .await;
            }

            NetworkCommand::SendMessage(msg) => self.send_message(msg).await,
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
                    sender_id: self.user_id.clone(),
                    destination: ReceiveDestination::Channel(channel_id),
                });

                // Sending returns an error if there are no subscribed listeners (we don't care
                // about that), or if the channel is closed. The channel can never close, because
                // the Sender side (which is responsible for dropping) is held in the state struct,
                // which is held by tasks that last for the full duration of the program. So we just
                // ignore this error.
                let _: Result<_, _> = channel.send(event);
            }

            SendDestination::User(user_id) => {
                let Some(user) = self.server_state.users.get(&user_id) else {
                    todo!("Log error, report to sender");
                };

                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.user_id.clone(),
                    destination: ReceiveDestination::Direct,
                });

                if let Err(e) = user.send(event).await {
                    todo!("Log error, report to sender {e}");
                }
            }
        }
    }

    async fn send_event_to_client(&mut self, message: NetworkEvent) {
        if let Err(e) = self.client_stream.send(message).await {
            todo!("Log error, report to sender");
        }
    }
}
