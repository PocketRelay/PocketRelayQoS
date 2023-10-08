use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use log::{debug, error};
use tokio::net::UdpSocket;

use crate::{constants::QOS_PORT, service::QService};

#[derive(Debug, Clone)]
pub struct QosHeader {
    // 0002, 0003, 0005,
    pub u1: u32,
    pub request_id: u32,
    pub request_secret: u32,
    pub probe_number: u32,
    pub u2: u32,
}

impl QosHeader {
    pub fn from_slice(header: &[u8]) -> QosHeader {
        let u1 = u32_from_slice(&header[0..4]);
        let request_id = u32_from_slice(&header[4..8]);
        let request_secret = u32_from_slice(&header[8..12]);
        let probe_number = u32_from_slice(&header[12..16]);
        let u2 = u32_from_slice(&header[16..20]);

        QosHeader {
            u1,
            request_id,
            request_secret,
            probe_number,
            u2,
        }
    }

    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.u1.to_be_bytes());
        out.extend_from_slice(&self.request_id.to_be_bytes());
        out.extend_from_slice(&self.request_secret.to_be_bytes());
        out.extend_from_slice(&self.probe_number.to_be_bytes());
        out.extend_from_slice(&self.u2.to_be_bytes());
    }
}

#[derive(Debug)]
pub struct QosResponseV1 {
    pub header: QosHeader,
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl QosResponseV1 {
    pub fn write(&self, out: &mut Vec<u8>) {
        self.header.write(out);
        out.extend_from_slice(&self.ip.octets());
        out.extend_from_slice(&self.port.to_be_bytes());
        out.extend_from_slice(&[0, 0, 0, 0])
    }
}

#[derive(Debug)]
pub struct QosResponseV2 {
    pub header: QosHeader,
    pub ubps: u32,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl QosResponseV2 {
    pub fn write(&self, out: &mut Vec<u8>) {
        self.header.write(out);
        out.extend_from_slice(&self.ubps.to_be_bytes());
        out.extend_from_slice(&self.port.to_be_bytes());
        out.extend_from_slice(&self.payload)
    }
}

pub async fn start_server(service: Arc<QService>) {
    // Socket for handling connections
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, QOS_PORT))
        .await
        .unwrap();

    let socket = Arc::new(socket);

    // Buffer for reciving messages
    let mut buffer = [0u8; 5048];

    loop {
        // Read bytes from the socket
        let (length, addr) = socket.recv_from(&mut buffer).await.unwrap();

        // Copy the request bytes from the buffer
        let buffer: Box<[u8]> = Box::from(&buffer[..length]);
        tokio::spawn(handle(service.clone(), socket.clone(), addr, buffer));
    }
}

fn u32_from_slice(slice: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(slice);
    u32::from_be_bytes(a)
}

/// Handles a new udp request
///
/// # Arguments
/// * socket - The udp socket bound for sending the response
/// * addr - The address of the message sender
/// * buffer - The received message buffer
async fn handle(
    service: Arc<QService>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    buffer: Box<[u8]>,
) {
    if buffer.len() < 20 {
        error!("Client didn't send a message long enough to be a header");
        return;
    }

    let addr = match addr {
        SocketAddr::V4(addr) => addr,
        SocketAddr::V6(_) => {
            error!("Got request from IPv6 address, don't know how to respond");
            return;
        }
    };

    let mut header = QosHeader::from_slice(&buffer[0..20]);
    debug!("RECV: {:?}", &header);

    let mut out: Vec<u8> = Vec::new();

    if header.request_id == 1 && header.request_secret == 0 {
        let response = QosResponseV1 {
            header,
            ip: *addr.ip(),
            port: addr.port(),
        };

        debug!("SEND: {:?}", &response);

        response.write(&mut out);
    } else {
        let request = service
            .get_request_data(header.request_id, header.request_secret)
            .await
            .expect("Missing request data for request");

        if request.q_type != 2 {
            error!("Unepxected qos request");
            return;
        }

        header.u2 = header.u2.swap_bytes();

        let payload = &buffer[20..(buffer.len() - 26)];
        let response = QosResponseV2 {
            header,
            ubps: u32::from_be_bytes([0x00, 0x5b, 0x8d, 0x80]),
            port: 38078,
            payload: payload.to_vec(),
        };

        debug!("SEND: {:?}", &response);

        response.write(&mut out);
    }

    if let Err(err) = socket.send_to(&out, addr).await {
        // TODO: Handle server unable to reach
        error!("Unable to return message to client {}: {}", addr, err);
    }
}
