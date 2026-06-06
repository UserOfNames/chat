use network_protocol::NetworkCommand;

/// Parameters to connect to a server.
#[derive(Debug)]
pub struct ConnectParams {
    /// Host name of the server.
    pub host: String,

    /// Port number of the server. If no port is given, the [`default
    /// port`](network_protocol::DEFAULT_LISTENER_PORT) is used.
    pub port: Option<u16>,

    /// Initial username the user wishes to use for the session.
    pub initial_username: String,
}

/// A command from the UI to the client backend.
#[derive(Debug)]
pub enum ClientCommand {
    /// Connect to a server using the given parameters.
    Connect(ConnectParams),

    /// Disconnect from the currently connected server. This is a NOP if not connected to a server.
    Disconnect,

    /// Shut down the backend.
    Quit,

    /// Commands which pass on to the network.
    NetworkCommand(NetworkCommand),
}
