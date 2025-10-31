use std::io;

use bevy_ecs::resource::Resource;
use tokio::{
    net::{TcpListener, ToSocketAddrs},
    runtime::Handle,
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
};
use tokio_util::codec::Framed;

use crate::{client::Client, packet::codec::PacketCodec};

#[derive(Resource)]
pub struct Server {
    new_clients: UnboundedReceiver<Client>,
}

impl Server {
    pub fn new(runtime: &Handle, addr: impl ToSocketAddrs + Send + 'static) -> Self {
        let (incoming_tx, incoming_rx) = unbounded_channel::<Client>();
        runtime.spawn(Self::listener_task(addr, incoming_tx));

        Self {
            new_clients: incoming_rx,
        }
    }

    pub fn iter_new_clients(&mut self) -> impl Iterator<Item = Client> + '_ {
        std::iter::from_fn(move || self.new_clients.try_recv().ok())
    }

    async fn listener_task(
        addr: impl ToSocketAddrs,
        new_clients: UnboundedSender<Client>,
    ) -> io::Result<()> {
        let listener = match TcpListener::bind(addr).await {
            Ok(listener) => listener,
            Err(e) => {
                tracing::error!("Error binding to address: {}", e);
                return Err(e);
            }
        };
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    tracing::info!("Accepted connection from {}", addr);
                    tokio::spawn(Client::spawn(
                        addr,
                        Framed::new(stream, PacketCodec::new()),
                        new_clients.clone(),
                    ));
                }
                Err(e) => {
                    tracing::error!("Error accepting connection: {}", e);
                    return Err(e);
                }
            }
        }
    }
}
