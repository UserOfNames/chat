use std::io;

use prost::Message;
use prost::bytes::Buf;
use tokio_util::codec::{Decoder, Encoder};

use crate::protobuf_items::{CommandFrame, EventFrame};
use crate::{NetworkCommand, NetworkEvent};

#[derive(Debug)]
pub struct ClientCodec;

impl Encoder<NetworkCommand> for ClientCodec {
    type Error = io::Error;

    fn encode(
        &mut self,
        item: NetworkCommand,
        dst: &mut prost::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        let frame = CommandFrame::from(item);
        frame.encode_length_delimited(dst)?;
        Ok(())
    }
}

impl Decoder for ClientCodec {
    type Item = NetworkEvent;
    type Error = io::Error;

    fn decode(
        &mut self,
        src: &mut prost::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
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

        let event = NetworkEvent::try_from(frame).unwrap(); // TODO: Actual error handling
        Ok(Some(event))
    }
}
