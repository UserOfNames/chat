use std::io;

use crate::protobuf_items::{CommandFrame, command_frame};

#[derive(Debug, Clone)]
pub enum NetworkCommand {
    SendMessage(String),
}

impl TryFrom<CommandFrame> for NetworkCommand {
    type Error = io::Error;

    fn try_from(value: CommandFrame) -> Result<Self, Self::Error> {
        match value.variant {
            Some(command_frame::Variant::SendMessage(message)) => {
                Ok(NetworkCommand::SendMessage(message))
            }

            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }
}

impl From<NetworkCommand> for CommandFrame {
    fn from(value: NetworkCommand) -> Self {
        match value {
            NetworkCommand::SendMessage(message) => CommandFrame {
                variant: Some(command_frame::Variant::SendMessage(message)),
            },
        }
    }
}
