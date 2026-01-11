use network_protocol::NetworkCommand;

/// A command from the UI to the client backend.
#[derive(Debug)]
pub enum ClientCommand {
    // === Local-only ===
    /// Connect to the server with address `String`.
    Connect(String),
    /// Disconnect from the currently connected server. This is a NOP if not connected to a server.
    Disconnect,
    /// Shut down the backend.
    Quit,

    // === Passed to nework ===
    /// Send a message to other clients connected to the server.
    SendMessage(String),
}

impl TryFrom<ClientCommand> for NetworkCommand {
    type Error = ();

    fn try_from(value: ClientCommand) -> Result<Self, Self::Error> {
        match value {
            ClientCommand::SendMessage(mes) => Ok(Self::SendMessage(mes)),
            _ => Err(()),
        }
    }
}
