use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

use log::{debug, error};
use tokio::net::UdpSocket;

use crate::{constants::QOS_PORT, service::QService};

#[derive(Debug, Clone)]
pub struct QosHeader {
    pub u1: u32,
    pub request_id: u32,
    pub request_secret: u32,
    pub probe_number: u32,
    pub u2: u32,
}

impl QosHeader {
    pub fn from_slice(header: &[u8]) -> QosHeader {
        let u1 = u32_from_slice(&header[0..4]); // 0002, 0003, 0005
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

    // Buffer for the packet header
    let mut buffer = [0u8; 5048];

    loop {
        // Read bytes from the socket
        let (length, addr) = socket.recv_from(&mut buffer).await.unwrap();
        if length < 20 {
            error!("Client didn't send a message long enough to be a header");
            continue;
        }

        let mut header = QosHeader::from_slice(&buffer[0..20]);

        debug!("Query: {:?}", &header,);

        // let addr_fake = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(104, 28, 193, 125), 38078));
        let addr_fake = addr;

        if header.request_id == 1 && header.request_secret == 0 {
            debug!("Query type 1 responding with address to: {}", addr);

            let SocketAddr::V4(socket_addr) = addr_fake else {
                continue;
            };

            let response = QosResponseV1 {
                header,
                ip: *socket_addr.ip(),
                port: socket_addr.port(),
            };

            let mut out = Vec::new();
            response.write(&mut out);

            // debug!("Message: {:?}", output);

            debug!("SEND");

            if let Err(err) = socket.send_to(&out, addr).await {
                // TODO: Handle server unable to reach
                error!("Unable to return message to client {}: {}", addr, err);
            }
        } else {
            let request = service
                .get_request_data(header.request_id, header.request_secret)
                .await
                .expect("Missing request data for request");

            header.u2 = header.u2.swap_bytes();

            if request.q_type == 2 {
                debug!("Query type 2 responding with port and padding to: {}", addr);

                let payload = &buffer[20..(length - 26)];

                let response = QosResponseV2 {
                    header,
                    ubps: u32::from_be_bytes([0x00, 0x5b, 0x8d, 0x80]),
                    port: request.client_port,
                    payload: payload.to_vec(),
                };

                let mut out = Vec::new();
                response.write(&mut out);

                // debug!("Message: {:?}", output);
                debug!("SEND");

                if let Err(err) = socket.send_to(&out, addr).await {
                    // TODO: Handle server unable to reach
                    error!("Unable to return message to client {}: {}", addr, err);
                }
            }
        }
    }
}

fn u32_from_slice(slice: &[u8]) -> u32 {
    let mut a = [0u8; 4];
    a.copy_from_slice(slice);
    u32::from_be_bytes(a)
}
