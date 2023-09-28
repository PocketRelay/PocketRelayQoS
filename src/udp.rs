use std::{
    net::{IpAddr, Ipv4Addr},
    sync::Arc,
};

use log::{debug, error};
use tokio::net::UdpSocket;

use crate::{constants::QOS_PORT, service::QService};

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

        let header = &buffer[0..20];

        let u1 = u32_from_slice(&header[0..4]);
        let request_id = u32_from_slice(&header[4..8]);
        let request_secret = u32_from_slice(&header[8..12]);
        let probe_number = u32_from_slice(&header[12..16]);
        let u2 = u32_from_slice(&header[16..20]);

        debug!(
            "Query: ID: {} SEC: {} NUM: {} ADDR: {}",
            request_id, request_secret, probe_number, addr
        );

        // let addr_fake = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(104, 28, 193, 125), 38078));
        let addr_fake = addr;

        if request_id == 1 && request_secret == 0 {
            debug!("Query type 1 responding with address to: {}", addr);
            let IpAddr::V4(ip) = addr_fake.ip() else {continue;};
            let port = addr_fake.port();
            let ip_bytes = ip.octets();
            let port_bytes = port.to_be_bytes();
            let mut output = Vec::new();
            output.extend_from_slice(header);
            output.extend_from_slice(&ip_bytes);
            output.extend_from_slice(&port_bytes);
            output.extend_from_slice(&[0, 0, 0, 0]);

            // debug!("Message: {:?}", output);

            if socket.send_to(&output, addr).await.is_err() {
                // TODO: Handle server unable to reach
                error!("Unable to return message to client: {}", addr);
            }
        } else {
            let request = service
                .get_request_data(request_id, request_secret)
                .await
                .expect("Missing request data for request");
            if request.q_type == 2 {
                debug!("Query type 2 responding with port and padding to: {}", addr);
                let padding = &buffer[20..length];
                let mut output = Vec::new();
                output.extend_from_slice(&buffer[0..16]);
                output.extend_from_slice(&[0x0a, 0x00, 0x00, 0x00]);
                output.extend_from_slice(&[0x00, 0x5b, 0x8d, 0x80]); // UBPS
                output.extend_from_slice(&request.client_port.to_be_bytes());
                output.extend_from_slice(padding);

                output.truncate(1200);

                // debug!("Message: {:?}", output);

                if socket.send_to(&output, addr).await.is_err() {
                    // TODO: Handle server unable to reach
                    error!("Unable to return message to client: {}", addr);
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
fn u16_from_slice(slice: &[u8]) -> u16 {
    let mut a = [0u8; 2];
    a.copy_from_slice(slice);
    u16::from_be_bytes(a)
}
