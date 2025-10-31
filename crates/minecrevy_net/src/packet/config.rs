use bevy_ecs::message::Message;

use crate::packet::io::Decodable;

#[derive(Message)]
pub struct KnownPacks(pub Vec<KnownPack>);

pub struct KnownPack {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

// TODO
#[derive(Message)]
pub struct RegistryData;

pub struct Finish;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FinishAcknowledged;

impl Decodable for FinishAcknowledged {
    fn decode(_buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(FinishAcknowledged)
    }
}
