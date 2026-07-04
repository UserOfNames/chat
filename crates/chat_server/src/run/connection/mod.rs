mod guard;

use std::{net::SocketAddr, sync::Arc};

use anyhow::bail;
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
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};
use tokio_util::{codec::Framed, sync::CancellationToken};
use tracing::{Level, debug, info, instrument, warn};

use crate::run::ServerState;

type ClientStream = Framed<TlsStream<TcpStream>, ServerCodec>;

/// A connection task responsible for talking to one client.
#[derive(Debug)]
pub struct Connection {
    /// Shared server state.
    server_state: Arc<ServerState>,

    /// Stream of commands coming from the client, or sending back to the client.
    client_stream: ClientStream,

    /// Address of the client associated with this connection.
    client_addr: SocketAddr,

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
    #[instrument(skip_all, parent = None, fields(%client_addr))]
    pub async fn start(
        server_state: Arc<ServerState>,
        tls_acceptor: TlsAcceptor,
        client_stream: TcpStream,
        client_addr: SocketAddr,
        cancellation_token: CancellationToken,
    ) {
        debug!("New client connection starting");

        let client_stream = match tls_acceptor.accept(client_stream).await {
            Ok(stream) => stream,
            Err(e) => {
                warn!(error = %e, "TLS handshake failed");
                return;
            }
        };
        let mut client_stream = Framed::new(client_stream, ServerCodec);
        debug!("Client completed TLS handshake");

        // We want to finish the ClientHello -> ServerHello handshake before anything else.
        // NOTE: For now, if the handshake fails for any reason, we just abort the connection
        // entirely. This keeps the implementation far simpler, at the cost of potentially repeating
        // the TLS handshake. If this becomes a problem later, we'll fix it later.
        let (event_rx, guard) =
            match Self::handshake_client(&mut client_stream, server_state.clone()).await {
                Ok(output) => output,
                Err(e) => {
                    warn!(error = %e, "Client handshake failed");
                    return;
                }
            };
        debug!("Client completed application-level handshake");

        // Subscribe to all the server's channels
        let channels: select_all::SelectAll<_> = server_state
            .subscribe_to_channels()
            .into_iter()
            .map(BroadcastStream::from)
            .collect();

        // Subscribe to the global broadcast channel. We do this AFTER sending the join notification
        // because the client doesn't need to be reminded that they connected (they already know
        // that).
        let global_event_rx = server_state.subscribe_to_global();

        let connection = Self {
            server_state,
            client_stream,
            client_addr,
            global_event_rx,
            event_rx,
            channels,
            cancellation_token,
            guard,
        };

        connection.run().await;
    }

    /// Perform the application-level handshake.
    #[instrument(skip_all, err(level = Level::WARN))]
    async fn handshake_client(
        client_stream: &mut ClientStream,
        server_state: Arc<ServerState>,
    ) -> anyhow::Result<(mpsc::Receiver<NetworkEvent>, ConnectionGuard)> {
        let hello = match client_stream.next().await {
            Some(Ok(NetworkCommand::ClientHello(hello))) => hello,
            Some(Ok(other)) => bail!("unexpected command: {other:?}"),
            Some(Err(e)) => bail!("IO error: {e}"),
            None => bail!("client stream closed unexpectedly"),
        };
        debug!(?hello, "Received client hello");

        let (event_tx, event_rx) = mpsc::channel::<NetworkEvent>(128); // TODO: Buffer size

        let user_token = match server_state.handle_new_user(
            hello.requested_name,
            server_state.max_username_length(),
            event_tx,
        ) {
            Ok(token) => token,
            Err(e) => return Err(anyhow::anyhow!(e)),
        };
        debug!(user_id = %user_token.id(), "Username successfully registered, user token created");

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
            bail!("could not send server hello: {e}");
        }
        debug!("Server hello sent successfully");

        Ok((event_rx, guard))
    }

    /// Internal helper to actually run the connection task. Why make `Connection` a struct at all,
    /// instad of a pure function? Why have this chain of calls just to allow it to be a struct?
    /// Because I don't want to have to pass every variable in `self` to every single helper
    /// function, when I could just do this and call `self.helper()`. State structs are a good
    /// pattern, even if it's purely internal.
    #[instrument(skip_all, parent = None, fields(
        user_id = %self.guard.id(),
        client_addr = %self.client_addr,
    ))]
    async fn run(mut self) {
        info!("New connection started");

        'connection: loop {
            tokio::select! {
                // Commands from the client.
                network_cmd = self.client_stream.next() => {
                    let Some(res) = network_cmd else {
                        info!("Client disconnected");
                        break 'connection;
                    };

                    match res {
                        Ok(cmd) => if let Err(e) = self.handle_command(cmd).await {
                            warn!(error = %e, "Client connection broke unexpectedly");
                            break 'connection;
                        }

                        Err(e) => {
                            warn!(error = %e, "Error reading command from client");
                        }
                    }
                },

                // Global events.
                event = self.global_event_rx.recv() => match event {
                    Ok(event) => self.send_event_to_client(event).await,

                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("Client lagged by {skipped} global messages. Forcing disconnect.");
                        break 'connection;
                    }

                    Err(broadcast::error::RecvError::Closed) => {
                        unreachable!("Global broadcast channel only closes when the server shuts down");
                    }
                },

                // Direct messages.
                direct_msg = self.event_rx.recv() => match direct_msg {
                    Some(msg) => self.send_event_to_client(msg).await,
                    None => {
                        unreachable!("Sender side of our MPSC channel only closes when we unregister ourselves from the server");
                    }
                },

                // Channel messages.
                Some(result) = self.channels.next() => {
                    match result {
                        Ok(msg) => self.send_event_to_client(msg).await,
                        Err(BroadcastStreamRecvError::Lagged(skipped)) => {
                            warn!("Client lagged by {skipped} channel messages. Forcing disconnect.");
                            break 'connection;
                        }
                    }
                }

                // Cancellation signal.
                () = self.cancellation_token.cancelled() => {
                    info!("Received cancellation signal, disconnecting...");

                    if let Err(e) = self.client_stream.flush().await {
                        warn!(error = %e, "Could not flush client stream on shutdown");
                    }

                    if let Err(e) = self.client_stream.into_inner().shutdown().await {
                        warn!(error = %e, "Failed to shut down cleanly");
                    }

                    info!("Disconnected cleanly");
                    break 'connection;
                }
            }
        }
    }

    async fn handle_command(&mut self, command: NetworkCommand) -> anyhow::Result<()> {
        match command {
            NetworkCommand::ClientHello(_) => {
                warn!("Received second client hello while already connected");
            }

            NetworkCommand::FetchChannels(_fetch) => {
                debug!("Client requested channel sync");
                self.send_event_to_client(NetworkEvent::ChannelSync(ChannelSync {
                    channels: self.server_state.get_all_channel_info(),
                }))
                .await;
            }

            NetworkCommand::FetchUsers(_fetch) => {
                debug!("Client requested user sync");
                self.send_event_to_client(NetworkEvent::UserSync(UserSync {
                    users: self.server_state.get_all_user_info(),
                }))
                .await;
            }

            NetworkCommand::SendMessage(msg) => {
                debug!(destination = ?msg.destination, "Client sent message");
                self.send_message(msg).await;
            }

            NetworkCommand::UpdateInfo(info) => {
                debug!(?info, "Client requested to update info");
                self.update_info(info).await?;
            }
        }

        Ok(())
    }

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

                // TODO: handle `false` case
                self.server_state.send_event_to_channel(channel_id, event);
            }

            SendDestination::User(target_user_id) => {
                let event = NetworkEvent::ReceivedMessage(ReceivedMessage {
                    contents,
                    sender_id: self.guard.id(),
                    destination: ReceiveDestination::User(target_user_id),
                });

                // TODO: handle `false` case
                self.server_state
                    .send_event_to_user(target_user_id, event.clone())
                    .await;

                // We send back to the sender as well to include them in the loopback, such that
                // they can render their own message in the correct order relative to other messages.
                // However, if the sender is sending to themselves (a "note to self"), this would
                // result in a double send. As such, we filter that case out.
                if target_user_id != self.guard.id() {
                    self.send_event_to_client(event).await;
                }
            }
        }
    }

    /// Update our user info.
    async fn update_info(&mut self, new_info: UpdateInfo) -> anyhow::Result<()> {
        if let Err(e) = self.server_state.update_user_info(
            self.guard.token(),
            new_info,
            self.server_state.max_username_length(),
        ) {
            warn!(error = %e, "Failed to update user info");

            self.client_stream
                .send(NetworkEvent::ErrorEvent(e.into()))
                .await?;
        }

        Ok(())
    }

    /// Send an event to the client associated with this `Connection`.
    async fn send_event_to_client(&mut self, event: NetworkEvent) {
        let event_name = event.name();

        if let Err(e) = self.client_stream.send(event).await {
            warn!(error = %e, event_type = %event_name, "Failed to send event to client");
        }
    }
}
