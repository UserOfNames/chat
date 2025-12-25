use network_protocol::NetworkCommand;

#[derive(Debug)]
pub enum ClientCommand {
    // Local-only
    Connect(String),
    Disconnect,
    Quit,

    // Network pass-through
    SendMessage(String),
}

impl TryFrom<ClientCommand> for NetworkCommand {
    // TODO: Actual error
    type Error = ();

    fn try_from(value: ClientCommand) -> Result<Self, Self::Error> {
        match value {
            ClientCommand::SendMessage(mes) => Ok(Self::SendMessage(mes)),
            _ => Err(()),
        }
    }
}
