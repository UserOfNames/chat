mod protobuf_items {
    include!(concat!(env!("OUT_DIR"), "/network_protocol.items.rs"));
}

mod network_command;
pub use network_command::NetworkCommand;

mod network_event;
pub use network_event::NetworkEvent;

pub mod codecs;
