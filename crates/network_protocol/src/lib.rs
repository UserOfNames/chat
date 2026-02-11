/// Default port the server listens to for new connections.
pub const DEFAULT_LISTENER_PORT: u16 = 12345;

mod protobuf_items {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.items.rs"));
}

mod network_command;
pub use network_command::NetworkCommand;

mod network_event;
pub use network_event::{ChatMessage, NetworkEvent};

pub mod codecs;
