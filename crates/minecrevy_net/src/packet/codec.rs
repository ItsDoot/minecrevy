use std::{any::TypeId, collections::HashMap, io};

use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::packet::{
    handshake::Handshake,
    io::{Decodable, Encodable, PacketBuf, PacketBufMut, ProtocolState, var_i32_size},
    status,
};

/// Type alias for type-erased [`Decodable`] packet decoder functions.
type PacketDecoder = Box<dyn Fn(&mut BytesMut) -> io::Result<Box<dyn Decodable>> + Send>;

/// [`Encoder`]/[`Decoder`] implementation for Minecraft packets.
#[derive(Default)]
pub struct PacketCodec {
    pub state: ProtocolState,
    /// Encoders mapped by (protocol state, packet type ID).
    encoders: HashMap<(ProtocolState, TypeId), i32>,
    /// Decoders mapped by (protocol state, packet ID).
    decoders: HashMap<(ProtocolState, i32), PacketDecoder>,
}

impl PacketCodec {
    // Creates a new codec with default packet registrations.
    pub fn new() -> Self {
        let mut codec = PacketCodec::default();

        codec.register_decoder::<Handshake>(ProtocolState::Handshake, 0x00);

        codec.register_encoder::<status::Response>(ProtocolState::Status, 0x00);
        codec.register_encoder::<status::PingRequest>(ProtocolState::Status, 0x01);
        codec.register_decoder::<status::Request>(ProtocolState::Status, 0x00);
        codec.register_decoder::<status::PingRequest>(ProtocolState::Status, 0x01);

        codec
    }

    /// Adds an [`Encodable`] packet encoder for the specified protocol state and packet ID.
    pub fn register_encoder<P: Encodable>(&mut self, state: ProtocolState, id: i32) {
        self.encoders.insert((state, TypeId::of::<P>()), id);
    }

    /// Adds a [`Decodable`] packet decoder for the specified protocol state and packet ID.
    pub fn register_decoder<P: Decodable>(&mut self, state: ProtocolState, id: i32) {
        self.decoders.insert(
            (state, id),
            Box::new(|buf| {
                let packet = P::decode(buf)?;
                Ok(Box::new(packet))
            }),
        );
    }
}

impl<'a> Encoder<&'a dyn Encodable> for PacketCodec {
    type Error = io::Error;

    fn encode(&mut self, item: &'a dyn Encodable, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let type_id = item.type_id();
        if let Some(&id) = self.encoders.get(&(self.state, type_id)) {
            let mut packet_buf = BytesMut::new();
            item.encode(&mut packet_buf)?;

            let packet_len = var_i32_size(id) + packet_buf.len();
            dst.reserve(var_i32_size(packet_len as i32) + packet_len);

            dst.put_var_i32(packet_len as i32);
            dst.put_var_i32(id);
            dst.extend_from_slice(&packet_buf);

            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "No encoder found for state {:?} and type id {:?}",
                    self.state, type_id
                ),
            ))
        }
    }
}

impl Decoder for PacketCodec {
    type Item = Box<dyn Decodable>;
    type Error = io::Error;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        let len = src.try_get_var_i32_limited::<3>()?;
        let len = usize::try_from(len).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid packet length: {}", len),
            )
        })?;

        if src.remaining() < len {
            return Ok(None);
        }

        let id = src.try_get_var_i32()?;

        if let Some(decoder) = self.decoders.get(&(self.state, id)) {
            let mut packet_bytes = src.split_to(len);
            Ok(Some(decoder(&mut packet_bytes)?))
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "No decoder found for state {:?} and packet id {}",
                    self.state, id
                ),
            ))
        }
    }
}
