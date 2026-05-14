use network_protocol::NetworkCommand;

/// A command from the UI to the client backend.
#[derive(Debug)]
pub enum ClientCommand {
    /// Connect to the server with host `String` and port `Option<u16>`. If no port is given, the
    /// [`default port`](network_protocol::DEFAULT_LISTENER_PORT) is used.
    Connect(String, Option<u16>),
    /// Disconnect from the currently connected server. This is a NOP if not connected to a server.
    Disconnect,
    /// Shut down the backend.
    Quit,
    /// Commands which pass on to the network.
    NetworkCommand(NetworkCommand),
}
