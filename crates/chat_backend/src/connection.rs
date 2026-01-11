use std::io;

use futures::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

use network_protocol::codecs::ClientCodec;
use network_protocol::{NetworkCommand, NetworkEvent};

/// A connection to a chat server.
#[derive(Debug)]
pub struct Connection {
    stream: Framed<TcpStream, ClientCodec>,
}

impl Connection {
    /// Create a new `Connection` to the server at `addr`.
    ///
    /// Returns an error if the connection failed for any reason (invalid address, connection
    /// refused, etc.).
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let socket = TcpStream::connect(addr).await?;
        let framed_socket = Framed::new(socket, ClientCodec);

        Ok(Self {
            stream: framed_socket,
        })
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
