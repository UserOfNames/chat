use std::{io, net::SocketAddr};

use clap::Parser;
use futures::{SinkExt, StreamExt};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{
        broadcast::{self, error::RecvError},
        mpsc::{self, error::SendError},
    },
};
use tokio_util::codec::Framed;

use network_protocol::{ChatMessage, NetworkCommand, NetworkEvent, codecs::ServerCodec};

#[derive(Debug, Parser)]
#[command(author = "UserOfNames", version, about)]
struct Args {
    /// The address the TCP listener binds to for accepting new client connections.
    listener_addr: String,
}

#[derive(Debug)]
struct Connection {
    stream: Framed<TcpStream, ServerCodec>,
    cmd_tx: mpsc::Sender<NetworkCommand>,
    event_rx: broadcast::Receiver<NetworkEvent>,
}

// TODO: Clean up error models on all these methods. It seems unlikely that many of these are
// actually recoverable or that returning them is appropriate.
impl Connection {
    fn new(
        stream: TcpStream,
        cmd_tx: mpsc::Sender<NetworkCommand>,
        event_rx: broadcast::Receiver<NetworkEvent>,
    ) -> Self {
        let framed_stream = Framed::new(stream, ServerCodec);
        Self {
            stream: framed_stream,
            cmd_tx,
            event_rx,
        }
    }

    async fn send_command(
        &mut self,
        command: NetworkCommand,
    ) -> Result<(), SendError<NetworkCommand>> {
        self.cmd_tx.send(command).await
    }

    async fn send_event(&mut self, event: NetworkEvent) -> io::Result<()> {
        self.stream.send(event).await
    }

    async fn activate(mut self) {
        loop {
            // TODO: This is obviously abysmal. Fix this alongside the other error model touchups.
            tokio::select! {
                command = self.stream.next() => self.send_command(command.unwrap().unwrap()).await.unwrap(),
                event = self.event_rx.recv() => self.send_event(event.unwrap()).await.unwrap(),
            }
        }
    }
}

#[derive(Debug)]
struct Server {
    listener: TcpListener,
    cmd_rx: mpsc::Receiver<NetworkCommand>,
    master_cmd_tx: mpsc::Sender<NetworkCommand>,
    event_tx: broadcast::Sender<NetworkEvent>,
}

impl Server {
    fn new(listener: TcpListener) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel(128); // TODO: Buffer size
        let (event_tx, _) = broadcast::channel(128); // TODO: Buffer size

        Self {
            listener,
            cmd_rx,
            master_cmd_tx: cmd_tx,
            event_tx,
        }
    }

    async fn run(mut self) {
        loop {
            tokio::select! {
                conn = self.listener.accept() => match conn {
                    Ok((stream, addr)) => self.handle_new_connection(stream, addr),
                    Err(e) => todo!("Log errors"),
                },

                command = self.cmd_rx.recv() => {
                    // Because we hold a master copy of the Sender inside of self, recv() can
                    // never return None. If it does, it's because self was dropped somehow, so
                    // we should break anyways.
                    let Some(command) = command else {
                        break;
                    };

                    self.handle_command(command);
                }
            }
        }
    }

    fn handle_new_connection(&mut self, stream: TcpStream, addr: SocketAddr) {
        let _ = addr; // For now, we just discard the address. We may do something with it later.

        let cmd_tx = self.master_cmd_tx.clone();
        let event_rx = self.event_tx.subscribe();

        let connection = Connection::new(stream, cmd_tx, event_rx);
        tokio::spawn(connection.activate());
    }

    fn handle_command(&mut self, command: NetworkCommand) {
        match command {
            NetworkCommand::SendMessage(msg) => self
                .event_tx
                .send(NetworkEvent::ReceivedMessage(ChatMessage {
                    contents: msg,
                    sender: "placeholder".to_owned(),
                }))
                .unwrap(), // TODO: You know the drill. Fix this later.
        };
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();

    let listener = TcpListener::bind(&args.listener_addr).await?;

    let server = Server::new(listener);
    server.run().await;

    Ok(())
}
