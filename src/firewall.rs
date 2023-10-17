use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use bytes::{Buf, BytesMut};
use log::{debug, error, info};
use tokio::net::UdpSocket;

use crate::{config::Config, service::QService};

pub async fn start_server(service: Arc<QService>, config: Arc<Config>) {
    // Socket for handling connections
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.udp_port_2))
        .await
        .unwrap();
    info!("Starting FireWall server on 0.0.0.0:{}", config.udp_port_2);
    let socket = Arc::new(socket);

    // Buffer for the packet header
    let mut buffer = [0u8; 65536 /* UDP allocated buffer size */];

    loop {
        // Read bytes from the socket
        let (length, addr) = socket.recv_from(&mut buffer).await.unwrap();

        // Copy the request bytes from the buffer
        let buffer: BytesMut = BytesMut::from(&buffer[..length]);
        tokio::spawn(handle(service.clone(), socket.clone(), addr, buffer));
    }
}

#[derive(Debug)]
pub struct FirewallRequest {
    pub request_id: u32,
    pub request_secret: u32,
}

impl FirewallRequest {
    pub fn from_buffer(buffer: &mut BytesMut) -> Self {
        let request_id = buffer.get_u32();
        let request_secret = buffer.get_u32();

        if !buffer.is_empty() {
            debug!(
                "Firewall message still had more bytes: {:?}",
                buffer.as_ref()
            );
        }

        Self {
            request_id,
            request_secret,
        }
    }
}

async fn handle(
    service: Arc<QService>,
    // We don't use the socket for responding
    _socket: Arc<UdpSocket>,
    addr: SocketAddr,
    mut buffer: BytesMut,
) {
    // Ignore messages that are too short
    if buffer.len() < 8 {
        error!(
            "Client didn't send a firewall message long enough to be a message: {:?}",
            buffer.as_ref()
        );
        return;
    }

    let message = FirewallRequest::from_buffer(&mut buffer);

    let rx = service
        .get_firewall_tx(message.request_id, message.request_secret)
        .await
        .expect("Missing request data for request");

    debug!("Firewall Query: MSG: {:?}  ADDR: {}", message, addr);

    _ = rx.send(addr);
}
