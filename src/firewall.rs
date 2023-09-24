use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use log::{debug, error};
use tokio::net::UdpSocket;

use crate::{constants::FIREWALL_PORT, service::QService};

pub async fn start_server(service: Arc<QService>) {
    // Socket for handling connections
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, FIREWALL_PORT))
        .await
        .unwrap();

    // Buffer for the packet header
    let mut buffer = [0u8; 5048];

    loop {
        // Read bytes from the socket
        let (length, addr) = socket.recv_from(&mut buffer).await.unwrap();
        if length < 8 {
            error!("Client didn't send a message long enough to be a header");
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

#[test]
fn a() {
    println!("{}", u32::from_be_bytes([0x00, 0x00, 0x04, 0x5b,]));
    println!("{}", u16::from_be_bytes([0x00, 0xe7,]));
    println!(
        "{}",
        Ipv4Addr::from(u32::from_be_bytes([0x00, 0xcc, 0x46, 0x01,]))
    );
}

fn parse() {
    let version = [0x00, 0x00, 0x00, 0x03];
    let request_id = [0x00, 0x00, 0x00, 0xea];
    let request_secret = [0x00, 0x00, 0x00, 0x71];
    let unknwon = [0x00, 0x00, 0x00, 0x71];
}

fn bytes_response() {}

fn bytes() {
    let packet_bytes: [u8; 8] = [
        0x00, 0x00, 0x02, 0x8e, // Request ID (654)
        0x00, 0x00, 0x04, 0x5b, // Request Secret (1115)
    ];
}
