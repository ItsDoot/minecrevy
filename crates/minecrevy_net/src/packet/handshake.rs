use bytes::Buf;

use crate::packet::io::{Decodable, PacketBuf, ProtocolState};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Handshake {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub next_state: NextState,
}

impl Decodable for Handshake {
    fn decode(buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Handshake {
            protocol_version: buf.try_get_var_i32()?,
            server_address: buf.try_get_var_string()?,
            server_port: buf.try_get_u16()?,
            next_state: NextState::decode(buf)?,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NextState {
    Status = 1,
    Login = 2,
}

impl Decodable for NextState {
    fn decode(buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let value = buf.try_get_var_i32()?;
        match value {
            1 => Ok(NextState::Status),
            2 => Ok(NextState::Login),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Invalid NextState value: {}", value),
            )),
        }
    }
}

impl From<NextState> for ProtocolState {
    fn from(next_state: NextState) -> Self {
        match next_state {
            NextState::Status => ProtocolState::Status,
            NextState::Login => ProtocolState::Login,
        }
    }
}
