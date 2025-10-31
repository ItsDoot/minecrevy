use bevy_ecs::message::Message;
use uuid::Uuid;

use crate::packet::{io::Decodable, types::GameProfile};

#[derive(Message)]
pub struct Start {
    pub username: String,
    pub uuid: Uuid,
}

#[derive(Message)]
pub struct Success(pub GameProfile);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Acknowledged;

impl Decodable for Acknowledged {
    fn decode(_buf: &mut bytes::BytesMut) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(Acknowledged)
    }
}
