//! Codecs for communication between the client backend and a server.

use std::io;

use prost::Message;
use prost::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::proto::{CommandFrame, EventFrame};
use crate::{NetworkCommand, NetworkEvent};

/// A codec for a client-side `Framed` `TcpStream`. Internally, this uses protobuf with a varint
/// length prefix.
///
/// This codec sends `NetworkCommand`s and receives `NetworkEvent`s.
#[derive(Debug)]
pub struct ClientCodec;

impl Encoder<NetworkCommand> for ClientCodec {
    type Error = io::Error;

    fn encode(&mut self, item: NetworkCommand, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let frame = CommandFrame::from(item);
        frame.encode_length_delimited(dst)?;
        Ok(())
    }
}

impl Decoder for ClientCodec {
    type Item = NetworkEvent;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut peek_slice = src.as_ref();

        // Quoting prost's documentation:
        // An error may be returned in two cases:
        //
        // * If the supplied buffer contains fewer than 10 bytes, then an error indicates that more
        //   input is required to decode the full delimiter.
        // * If the supplied buffer contains 10 bytes or more, then the buffer contains an invalid
        //   delimiter, and typically the buffer should be considered corrupt.
        let len = match prost::decode_length_delimiter(&mut peek_slice) {
            Ok(len) => len,
            Err(_) if src.len() < 10 => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let delimiter_width = src.len() - peek_slice.len();

        if src.len() < delimiter_width + len {
            return Ok(None);
        }

        // Small optimization: avoid the redundant length delimiter calaculations that
        // decode_length_delimited() would invoke.
        src.advance(delimiter_width);
        let chunk = src.split_to(len);

        let frame = EventFrame::decode(chunk)?;

        let event = NetworkEvent::try_from(frame)?;

        Ok(Some(event))
    }
}

/// A codec for a client-side `Framed` `TcpStream`. Internally, this uses protobuf with a varint
/// length prefix.
///
/// This codec sends `NetworkEvent`s and receives `NetworkCommand`s.
#[derive(Debug)]
pub struct ServerCodec;

impl Encoder<NetworkEvent> for ServerCodec {
    type Error = io::Error;

    fn encode(&mut self, item: NetworkEvent, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let frame = EventFrame::from(item);
        frame.encode_length_delimited(dst)?;
        Ok(())
    }
}

impl Decoder for ServerCodec {
    type Item = NetworkCommand;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let mut peek_slice = src.as_ref();

        // Quoting prost's documentation:
        // An error may be returned in two cases:
        //
        // * If the supplied buffer contains fewer than 10 bytes, then an error indicates that more
        //   input is required to decode the full delimiter.
        // * If the supplied buffer contains 10 bytes or more, then the buffer contains an invalid
        //   delimiter, and typically the buffer should be considered corrupt.
        let len = match prost::decode_length_delimiter(&mut peek_slice) {
            Ok(len) => len,
            Err(_) if src.len() < 10 => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        let delimiter_width = src.len() - peek_slice.len();

        if src.len() < delimiter_width + len {
            return Ok(None);
        }

        // Small optimization: avoid the redundant length delimiter calaculations that
        // decode_length_delimited() would invoke.
        src.advance(delimiter_width);
        let chunk = src.split_to(len);

        let frame = CommandFrame::decode(chunk)?;

        let command = NetworkCommand::try_from(frame)?;
        Ok(Some(command))
    }
}

#[cfg(test)]
mod test {
    #![allow(clippy::expect_fun_call)]

    use uuid::Uuid;

    use crate::*;

    use super::*;

    fn mock_user_id() -> UserId {
        UserId(Uuid::max())
    }

    fn mock_channel_id() -> ChannelId {
        ChannelId(12345)
    }

    fn mock_user_info() -> UserInfo {
        UserInfo {
            id: mock_user_id(),
            name: "mocked user name".to_owned(),
        }
    }

    fn mock_channel_info() -> ChannelInfo {
        ChannelInfo {
            id: mock_channel_id(),
            name: "mocked channel name".to_owned(),
        }
    }

    fn mock_all_commands() -> Vec<NetworkCommand> {
        vec![
            NetworkCommand::ClientHello(ClientHello {
                requested_name: "my name".to_owned(),
            }),
            NetworkCommand::FetchChannels(FetchChannels),
            NetworkCommand::FetchUsers(FetchUsers),
            NetworkCommand::SendMessage(SendMessage {
                contents: "Hello World!".to_owned(),
                destination: SendDestination::Channel(mock_channel_id()),
            }),
            NetworkCommand::UpdateInfo(UpdateInfo {
                name: Some("my new name".to_owned()),
            }),
        ]
    }

    fn mock_all_events() -> Vec<NetworkEvent> {
        vec![
            NetworkEvent::ServerHello(ServerHello {
                your_id: mock_user_id(),
                default_channel_id: Some(mock_channel_id()),
            }),
            NetworkEvent::ChannelSync(ChannelSync {
                channels: vec![mock_channel_info()],
            }),
            NetworkEvent::UserSync(UserSync {
                users: vec![mock_user_info()],
            }),
            NetworkEvent::UserJoined(mock_user_info()),
            NetworkEvent::UserLeft(mock_user_id()),
            NetworkEvent::UserInfoUpdated(mock_user_info()),
            NetworkEvent::ReceivedMessage(ReceivedMessage {
                contents: "Hello World!".to_owned(),
                sender_id: mock_user_id(),
                destination: ReceiveDestination::Channel(mock_channel_id()),
            }),
            NetworkEvent::ErrorEvent(ErrorEvent {
                kind: ErrorKind::Unknown,
                message: "Some error occurred".to_owned(),
            }),
        ]
    }

    #[test]
    fn test_client_to_server_roundtrip() {
        let mut client_encoder = ClientCodec;
        let mut server_decoder = ServerCodec;
        let mut buffer = BytesMut::new();
        let commands = mock_all_commands();

        for command in commands {
            client_encoder
                .encode(command.clone(), &mut buffer)
                .expect(&format!("Client failed to encode {}", command.name()));

            let decoded_result = server_decoder
                .decode(&mut buffer)
                .expect(&format!("Server failed to decode {}", command.name()))
                .expect(&format!(
                    "Command {} should be completely decoded but isn't",
                    command.name()
                ));

            assert_eq!(
                decoded_result,
                command,
                "Decoded {} did not match original",
                command.name()
            );

            assert!(
                buffer.is_empty(),
                "Buffer was not fully consumed after decoding {}",
                command.name()
            );
        }
    }

    #[test]
    fn test_server_to_client_roundtrip() {
        let mut server_encoder = ServerCodec;
        let mut client_decoder = ClientCodec;
        let mut buffer = BytesMut::new();
        let events = mock_all_events();

        for event in events {
            server_encoder
                .encode(event.clone(), &mut buffer)
                .expect(&format!("Server failed to encode {}", event.name()));

            let decoded_result = client_decoder
                .decode(&mut buffer)
                .expect(&format!("Client failed to decode {}", event.name()))
                .expect(&format!(
                    "Event {} should be completely decoded but isn't",
                    event.name()
                ));

            assert_eq!(
                decoded_result,
                event.clone(),
                "Decoded {} did not match original",
                event.name()
            );

            assert!(
                buffer.is_empty(),
                "Buffer was not fully consumed after decoding {}",
                event.name()
            );
        }
    }

    #[test]
    fn test_server_decoder_waits_for_full_frame() {
        let mut server_encoder = ServerCodec;
        let mut client_decoder = ClientCodec;
        let events = mock_all_events();

        for event in events {
            let mut full_buffer = BytesMut::new();
            let mut partial_buffer = BytesMut::new();
            let event_name = event.name();

            server_encoder.encode(event, &mut full_buffer).unwrap();
            partial_buffer.extend_from_slice(&full_buffer[..1]);

            let result_of_first_byte = client_decoder
                .decode(&mut partial_buffer)
                .expect(&format!("Partial decode of {event_name} returned Err(_)"));
            assert!(
                result_of_first_byte.is_none(),
                "Partial decode of {event_name} returned Some(_)"
            );

            assert!(
                !partial_buffer.is_empty(),
                "Buffer unexpectedly fully consumed after partial decode of {event_name}"
            );

            partial_buffer.extend_from_slice(&full_buffer[1..]);
            client_decoder
                .decode(&mut partial_buffer)
                .expect(&format!("Full decode of {event_name} returned Err(_)"))
                .expect(&format!("Full decode of {event_name} returned None"));

            assert!(
                partial_buffer.is_empty(),
                "Buffer not was not fully consumed after decoding {event_name}"
            );
        }
    }

    #[test]
    fn test_client_decoder_waits_for_full_frame() {
        let mut client_encoder = ClientCodec;
        let mut server_decoder = ServerCodec;
        let commands = mock_all_commands();

        for command in commands {
            let mut full_buffer = BytesMut::new();
            let mut partial_buffer = BytesMut::new();
            let command_name = command.name();

            client_encoder.encode(command, &mut full_buffer).unwrap();
            partial_buffer.extend_from_slice(&full_buffer[..1]);

            let result_of_first_byte = server_decoder
                .decode(&mut partial_buffer)
                .expect(&format!("Partial decode of {command_name} returned Err(_)"));
            assert!(
                result_of_first_byte.is_none(),
                "Partial decode of {command_name} returned Some(_)"
            );

            assert!(
                !partial_buffer.is_empty(),
                "Buffer unexpectedly fully consumed after partial decode of {command_name}"
            );

            partial_buffer.extend_from_slice(&full_buffer[1..]);
            server_decoder
                .decode(&mut partial_buffer)
                .expect(&format!("Full decode of {command_name} returned Err(_)"))
                .expect(&format!("Full decode of {command_name} returned None"));

            assert!(
                partial_buffer.is_empty(),
                "Buffer not was not fully consumed after decoding {command_name}"
            );
        }
    }

    #[test]
    fn test_server_decoder_fails_on_bad_bytes() {
        let mut server_decoder = ServerCodec;
        let mut buffer = BytesMut::zeroed(1000);

        let result = server_decoder.decode(&mut buffer);

        assert!(
            result.is_err(),
            "Server decoder successfully decoded garbage data"
        );
    }

    #[test]
    fn test_client_decoder_fails_on_bad_bytes() {
        let mut client_decoder = ClientCodec;
        let mut buffer = BytesMut::zeroed(1000);

        let result = client_decoder.decode(&mut buffer);

        assert!(
            result.is_err(),
            "Client decoder successfully decoded garbage data"
        );
    }
}
