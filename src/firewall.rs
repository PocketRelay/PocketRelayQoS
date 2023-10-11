use std::{net::Ipv4Addr, sync::Arc};

use log::{debug, info};
use tokio::net::UdpSocket;

use crate::{config::Config, service::QService};

pub async fn start_server(service: Arc<QService>, config: Arc<Config>) {
    // Socket for handling connections
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.udp_port_2))
        .await
        .unwrap();
    info!("Starting FireWall server on 0.0.0.0:{}", config.udp_port_2);

    // Buffer for the packet header
    let mut buffer = [0u8; 8];

    loop {
        // Read bytes from the socket
        let (length, addr) = socket.recv_from(&mut buffer).await.unwrap();
        // Ignore messages that are too short
        if length < 8 {
            continue;
        }

        let header = &buffer[0..8];

        let request_id = u32_from_slice(&header[0..4]);
        let request_secret = u32_from_slice(&header[4..8]);

        let rx = service
            .get_firewall_tx(request_id, request_secret)
            .await
            .expect("Missing request data for request");

        debug!(
            "Firewall Query: ID: {} SEC: {}  ADDR: {}",
            request_id, request_secret, addr
        );

        _ = rx.send(addr);
    }
}

fn u32_from_slice(slice: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(slice);
    u32::from_be_bytes(a)
}
