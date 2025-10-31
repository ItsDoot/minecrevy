use std::net::SocketAddr;

use bevy_ecs::component::Component;
use futures_util::{SinkExt, StreamExt};
use tokio::{
    net::TcpStream,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};
use tokio_util::codec::Framed;

use crate::packet::{
    codec::PacketCodec,
    config,
    handshake::{Handshake, NextState},
    io::{Decodable, Encodable, ProtocolState},
    login,
    status::{self, PingRequest},
};

#[derive(Component)]
pub struct Client {
    addr: SocketAddr,
    outgoing: UnboundedSender<Box<dyn Encodable>>,
    incoming: UnboundedReceiver<Box<dyn Decodable>>,
}

impl Client {
    /// Returns the client's socket address.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns true if the client is still connected.
    pub fn is_connected(&self) -> bool {
        // The outgoing channel gets closed when the I/O task ends.
        !self.outgoing.is_closed()
    }

    /// Returns an iterator over incoming packets.
    pub fn iter_incoming(&mut self) -> impl Iterator<Item = Box<dyn Decodable>> + '_ {
        std::iter::from_fn(move || self.incoming.try_recv().ok())
    }

    /// Sends a packet to the client.
    pub fn send(&self, packet: impl Encodable) {
        self.outgoing.send(Box::new(packet)).ok();
    }

    pub async fn spawn(
        addr: SocketAddr,
        mut stream: Framed<TcpStream, PacketCodec>,
        new_clients: UnboundedSender<Self>,
    ) {
        let (outgoing_tx, mut outgoing_rx) = unbounded_channel::<Box<dyn Encodable>>();
        let (incoming_tx, incoming_rx) = unbounded_channel::<Box<dyn Decodable>>();
        let mut incoming_rx = Some(incoming_rx);

        loop {
            tokio::select! {
                // Read packets from the outgoing channel, and send them to the stream.
                Some(packet) = outgoing_rx.recv() => {
                    let is_disconnect = packet.is_disconnect();
                    if let Err(e) = stream.send(&*packet).await {
                        tracing::error!(%addr, "Error sending packet: {}", e);
                        break;
                    }
                    if is_disconnect {
                        tracing::info!(%addr, "Disconnected");
                        break;
                    }
                }
                // Read packets from the stream, and forward them to the incoming channel.
                result = stream.next() => {
                    match result {
                        Some(Ok(packet)) => {
                            if let Some(handshake) = packet.downcast_ref::<Handshake>() {
                                stream.codec_mut().state = handshake.next_state.into();
                                if handshake.next_state == NextState::Login {
                                    // We don't want to spawn entities for clients that are just checking
                                    // server status. We'll wait until they enter the login stage.
                                    let client = Client {
                                        addr,
                                        outgoing: outgoing_tx.clone(),
                                        incoming: incoming_rx.take().unwrap_or_else(|| {
                                            unreachable!("Client re-entered handshake state somehow");
                                        })
                                    };
                                    if let Err(e) = new_clients.send(client) {
                                        tracing::error!(%addr, "Error registering new client: {}", e);
                                        break;
                                    }
                                }
                                // We don't need to forward handshake packets.
                                continue;
                            } else if let Some(ping) = packet.downcast_ref::<PingRequest>() {
                                if let Err(e) = stream.send(ping).await {
                                    tracing::error!(%addr, "Error sending ping response: {}", e);
                                    break;
                                }
                                // We don't need to forward ping packets.
                                continue;
                            } else if packet.is::<status::Request>() {
                                if let Err(e) = stream.send(&status::Response("{\"version\":{\"name\":\"1.20.4\",\"protocol\":763},\"players\":{\"max\":100,\"online\":0,\"sample\":[]},\"description\":{\"text\":\"A Minecrevy Server\"}}".to_string())).await {
                                    tracing::error!(%addr, "Error sending status response: {}", e);
                                    break;
                                }
                                // We don't need to forward status request packets.
                                continue;
                            } else if packet.is::<login::Acknowledged>() {
                                // Switch to the config state after login is acknowledged. We're still
                                // forwarding the packet though, so the game knows we've logged in.
                                stream.codec_mut().state = ProtocolState::Config;
                            } else if packet.is::<config::FinishAcknowledged>() {
                                // Switch to the play state after config is finished. We're still
                                // forwarding the packet though, so the game knows we're ready to play.
                                stream.codec_mut().state = ProtocolState::Play;
                            }

                            if let Err(e) = incoming_tx.send(packet) {
                                tracing::error!(%addr, "Error receiving packet: {}", e);
                                break;
                            }
                        }
                        Some(Err(e)) => {
                            tracing::error!(%addr, "Error reading packet: {}", e);
                            break;
                        }
                        None => {
                            tracing::info!(%addr, "Connection closed by remote host");
                            break;
                        }
                    }
                }
                // Timeout after 30s
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                    tracing::info!(%addr, "Connection timed out");
                    break;
                }
            }
        }
    }
}
