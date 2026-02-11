use network_protocol::codecs::ServerCodec;
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tokio_util::codec::Framed;

#[derive(Debug)]
pub struct Connection {
    stream: Framed<TlsStream<TcpStream>, ServerCodec>,
}
