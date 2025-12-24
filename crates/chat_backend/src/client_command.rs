use protocol::NetworkCommand;

#[derive(Debug)]
pub enum ClientCommand {
    Connect(String),
    Disconnect,
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
