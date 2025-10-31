use std::{any::Any, fmt::Debug, io};

use bytes::{BufMut, BytesMut};
use tokio_util::bytes::Buf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, Default)]
pub enum ProtocolState {
    #[default]
    Handshake,
    Status,
    Login,
    Config,
    Play,
}

/// Trait for packets that can be encoded to bytes.
pub trait Encodable: Debug + Any + Send + Sync {
    /// Encode the packet into the provided buffer.
    fn encode(&self, buf: &mut BytesMut) -> io::Result<()>;

    /// Returns true if this packet is a disconnect packet.
    fn is_disconnect(&self) -> bool {
        false
    }
}

/// Trait for packets that can be decoded from bytes.
pub trait Decodable: Debug + Any + Send + Sync {
    /// Decode the packet from the provided buffer.
    fn decode(buf: &mut BytesMut) -> io::Result<Self>
    where
        Self: Sized;
}

impl dyn Decodable {
    pub fn downcast_ref<T: Decodable>(&self) -> Option<&T> {
        <dyn Any>::downcast_ref::<T>(self)
    }

    pub fn downcast_mut<T: Decodable>(&mut self) -> Option<&mut T> {
        <dyn Any>::downcast_mut::<T>(self)
    }

    pub fn is<T: Decodable>(&self) -> bool {
        <dyn Any>::is::<T>(self)
    }
}

pub trait PacketBuf: Buf {
    fn try_get_var_i32_limited<const MAX_BYTES: u8>(&mut self) -> io::Result<i32> {
        let readable = self.remaining();
        // Special case for empty buffer
        if readable == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Not enough bytes to read VarInt",
            ));
        }

        // We can read at least one byte, and this should be a common case
        let mut k = self.try_get_u8()?;
        if (k & 0x80) != 128 {
            return Ok(i32::from(k));
        }

        // In case decoding one byte was not enough, use a loop to decode up to the next 4 bytes
        let max_read = readable.min(MAX_BYTES as usize);
        let mut i: i32 = k as i32 & 0x7F;
        for j in 1..max_read {
            k = self.try_get_u8()?;
            i |= (k as i32 & 0x7F) << (j * 7);
            if (k & 0x80) != 128 {
                return Ok(i);
            }
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("VarInt exceeds maximum size of {} bytes", MAX_BYTES),
        ))
    }

    fn try_get_var_i32(&mut self) -> io::Result<i32> {
        self.try_get_var_i32_limited::<5>()
    }

    fn try_get_var_string(&mut self) -> io::Result<String> {
        let len = self.try_get_var_i32()?;
        let len = usize::try_from(len).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid string length: {}", len),
            )
        })?;
        let mut str_bytes = vec![0u8; len];
        self.try_copy_to_slice(&mut str_bytes)?;
        String::from_utf8(str_bytes).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

impl<T: Buf> PacketBuf for T {}

pub trait PacketBufMut: BufMut {
    fn put_var_i32(&mut self, value: i32) {
        let value = value as u32;
        if (value & (0xFFFFFFFF << 7)) == 0 {
            self.put_u8(value as u8);
        } else if (value & (0xFFFFFFFF << 14)) == 0 {
            let w = (value & 0x7F | 0x80) << 8 | (value >> 7);
            self.put_u16(w as u16);
        } else if (value & (0xFFFFFFFF << 21)) == 0 {
            let w = (value & 0x7F | 0x80) << 16 | ((value >> 7) & 0x7F | 0x80) << 8 | (value >> 14);
            self.put_u8((w >> 16) as u8);
            self.put_u8((w >> 8) as u8);
            self.put_u8(w as u8);
        } else if (value & (0xFFFFFFFF << 28)) == 0 {
            let w = (value & 0x7F | 0x80) << 24
                | (((value >> 7) & 0x7F | 0x80) << 16)
                | ((value >> 14) & 0x7F | 0x80) << 8
                | (value >> 21);
            self.put_u32(w);
        } else {
            let w = (value & 0x7F | 0x80) << 24
                | ((value >> 7) & 0x7F | 0x80) << 16
                | ((value >> 14) & 0x7F | 0x80) << 8
                | ((value >> 21) & 0x7F | 0x80);
            self.put_u32(w);
            self.put_u8((value >> 28) as u8);
        }
    }

    fn put_var_string(&mut self, value: &str) {
        let str_bytes = value.as_bytes();
        let len = str_bytes.len();
        self.put_var_i32(len as i32);
        self.put_slice(str_bytes);
    }
}

impl<T: BufMut> PacketBufMut for T {}

pub fn var_i32_size(mut value: i32) -> usize {
    let mut size = 0;
    loop {
        size += 1;
        if (value & !0x7F) == 0 {
            break;
        }
        value >>= 7;
    }
    size
}
