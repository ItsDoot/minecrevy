use bytes::{Buf, BufMut};

use crate::packet::io::{Decodable, Encodable, PacketBufMut};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Request;

impl Decodable for Request {
    fn decode(_buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Request)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Response(pub String);

impl Encodable for Response {
    fn encode(&self, buf: &mut bytes::BytesMut) -> std::io::Result<()> {
        buf.put_var_string(&self.0);
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PingRequest(pub i64);

impl Encodable for PingRequest {
    fn encode(&self, buf: &mut bytes::BytesMut) -> std::io::Result<()> {
        buf.put_i64(self.0);
        Ok(())
    }
}

impl Decodable for PingRequest {
    fn decode(buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(PingRequest(buf.try_get_i64()?))
    }
}
