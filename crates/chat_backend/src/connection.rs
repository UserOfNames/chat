use std::io;

use futures::{SinkExt, StreamExt};
use rustls::pki_types::ServerName;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_rustls::{TlsConnector, client::TlsStream};
use tokio_util::codec::Framed;

use network_protocol::codecs::ClientCodec;
use network_protocol::{NetworkCommand, NetworkEvent};

/// A connection to a chat server.
#[derive(Debug)]
pub struct Connection {
    stream: Framed<TlsStream<TcpStream>, ClientCodec>,
}

impl Connection {
    /// Create a new `Connection` to the server at `host:port`.
    ///
    /// If no port is provided, the [default port](network_protocol::DEFAULT_LISTENER_PORT) is used.
    ///
    /// Returns an error if the connection failed for any reason (invalid address, connection
    /// refused, etc.).
    pub async fn connect(
        host: &str,
        port: Option<u16>,
        tls_connector: &TlsConnector,
    ) -> io::Result<Self> {
        let port = port.unwrap_or(network_protocol::DEFAULT_LISTENER_PORT);

        let domain = ServerName::try_from(host.to_owned()).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid TLS host: {e}"),
            )
        })?;

        let stream = TcpStream::connect((host, port)).await?;
        let stream = tls_connector.connect(domain, stream).await?;
        let stream = Framed::new(stream, ClientCodec);

        Ok(Self { stream })
    }

    /// Consume the `Connection` and attempt a clean disconnect.
    ///
    /// Returns an error if the disconnect could not be done cleanly.
    pub async fn disconnect(self) -> io::Result<()> {
        self.stream.into_inner().shutdown().await
    }

    /// Send a command to the connected server.
    ///
    /// Returns an error if the connection closed.
    pub async fn send_command(&mut self, command: NetworkCommand) -> io::Result<()> {
        self.stream.send(command).await
    }

    /// Listen for an event from the connected server.
    ///
    /// Returns `None` if the connection closed.
    /// Returns an error if the message was received, but was corrupted in some way.
    pub async fn receive_event(&mut self) -> Option<io::Result<NetworkEvent>> {
        self.stream.next().await
    }
}
