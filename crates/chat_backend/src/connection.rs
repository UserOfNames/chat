use std::io;

use futures::{SinkExt, StreamExt};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio_util::codec::Framed;

use network_protocol::codecs::ClientCodec;
use network_protocol::{NetworkCommand, NetworkEvent};

#[derive(Debug)]
pub struct Connection {
    stream: Framed<TcpStream, ClientCodec>,
}

impl Connection {
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let socket = TcpStream::connect(addr).await?;
        let framed_socket = Framed::new(socket, ClientCodec);

        Ok(Self {
            stream: framed_socket,
        })
    }

    pub async fn disconnect(self) -> io::Result<()> {
        self.stream.into_inner().shutdown().await
    }

    pub async fn send_command(&mut self, command: NetworkCommand) -> io::Result<()> {
        self.stream.send(command).await
    }

    pub async fn receive_event(&mut self) -> Option<io::Result<NetworkEvent>> {
        self.stream.next().await
    }
}
