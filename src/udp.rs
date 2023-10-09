use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::{Buf, BufMut, BytesMut};
use log::{debug, error};
use tokio::{net::UdpSocket, sync::RwLock};

use crate::{constants::QOS_PORT, service::QService};

#[derive(Debug, Clone)]
pub struct QosHeader {
    // 0002, 0003, 0005,
    pub u1: u32,
    pub request_id: u32,
    pub request_secret: u32,
    pub probe_number: u32,
}

impl QosHeader {
    pub fn from_buffer(header: &mut BytesMut) -> QosHeader {
        let u1 = header.get_u32();
        let request_id = header.get_u32();
        let request_secret = header.get_u32();
        let probe_number = header.get_u32();

        QosHeader {
            u1,
            request_id,
            request_secret,
            probe_number,
        }
    }

    pub fn write(&self, out: &mut BytesMut) {
        out.put_u32(self.u1);
        out.put_u32(self.request_id);
        out.put_u32(self.request_secret);
        out.put_u32(self.probe_number);
    }
}

#[derive(Debug)]
pub struct QosRequestV1 {
    pub timestamp: u32,
}

impl QosRequestV1 {
    pub fn from_buffer(buffer: &mut BytesMut) -> Self {
        let timestamp = buffer.get_u32();
        Self { timestamp }
    }
}

#[derive(Debug)]
pub struct QosRequestV2 {
    pub probe_count: u32,
    pub payload: Vec<u8>,
}

impl QosRequestV2 {
    pub fn from_buffer(buffer: &mut BytesMut) -> Self {
        let probe_count = buffer.get_u32();
        let padding = buffer.split().to_vec();
        Self {
            probe_count,
            payload: padding,
        }
    }
}

#[derive(Debug)]
pub struct QosResponseV1 {
    pub header: QosHeader,
    pub timestamp: u32,
    pub ip: Ipv4Addr,
    pub port: u16,
}

impl QosResponseV1 {
    pub fn write(&self, out: &mut BytesMut) {
        self.header.write(out);
        out.put_u32(self.timestamp);
        out.extend_from_slice(&self.ip.octets());
        out.put_u16(self.port);
        out.extend_from_slice(&[0, 0, 0, 0]);
    }
}

#[derive(Debug)]
pub struct QosResponseV2 {
    pub header: QosHeader,
    pub probe_count: u32,
    pub ubps: u32,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl QosResponseV2 {
    pub fn write(&self, out: &mut BytesMut) {
        self.header.write(out);
        out.put_u32_le(self.probe_count);
        out.put_u32(self.ubps);
        out.put_u16(self.port);
        out.extend_from_slice(&self.payload);
    }

    // 9774859
    // 9947171
    // 9947890
    // 10229015
    // 10229390
    // 10381765
    // 10381984
}

#[test]
fn test() {
    let times: [(u32, u64); 4] = [
        (10478125, 1696807213070),
        (10478140, 1696807213087),
        (10478156, 1696807213102),
        (10478218, 1696807213167),
    ];

    for (a, b) in times {
        println!("{}", b - a as u64);
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
        let buffer: BytesMut = BytesMut::from(&buffer[..length]);
        tokio::spawn(handle(service.clone(), socket.clone(), addr, buffer));
    }
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
    mut buffer: BytesMut,
) {
    if buffer.len() < 16 {
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

    let header = QosHeader::from_buffer(&mut buffer);
    debug!("RECV: {:?}", &header);
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    debug!("AT: {}", time.as_millis());

    let mut out: BytesMut = BytesMut::new();
    let public_ip = public_address().await.unwrap();

    if header.request_id == 1 && header.request_secret == 0 {
        let request = QosRequestV1::from_buffer(&mut buffer);

        debug!("RECV DATA: {:?}", &request);

        let response = QosResponseV1 {
            header,
            timestamp: request.timestamp,
            // ip: *addr.ip(),
            ip: public_ip,
            port: addr.port(),
        };

        debug!("SEND: {:?}", &response);

        response.write(&mut out);
    } else {
        let request = QosRequestV2::from_buffer(&mut buffer);

        debug!("RECV DATA: {:?}", &request);

        let mut payload = request.payload;
        // Drop 6 bytes from the payload to fit the ubps and port1
        payload.truncate(payload.len() - 6);

        let response = QosResponseV2 {
            header,
            probe_count: request.probe_count,
            ubps: u32::from_be_bytes([0x00, 0x5b, 0x8d, 0x80]),
            port: addr.port(),
            payload,
        };

        debug!("SEND: {:?}", &response);

        response.write(&mut out);
    }

    if let Err(err) = socket.send_to(&out, addr).await {
        // TODO: Handle server unable to reach
        error!("Unable to return message to client {}: {}", addr, err);
    }
}

/// Caching structure for the public address value
enum PublicAddrCache {
    /// The value hasn't yet been computed
    Unset,
    /// The value has been computed
    Set {
        /// The public address value
        value: Ipv4Addr,
        /// The system time the cache expires at
        expires: SystemTime,
    },
}

/// Cache value for storing the public address
static PUBLIC_ADDR_CACHE: RwLock<PublicAddrCache> = RwLock::const_new(PublicAddrCache::Unset);

/// Cache public address for 30 minutes
const ADDR_CACHE_TIME: Duration = Duration::from_secs(60 * 30);

/// Retrieves the public address of the server either using the cached
/// value if its not expired or fetching the new value from the one of
/// two possible APIs
async fn public_address() -> Option<Ipv4Addr> {
    {
        let cached = &*PUBLIC_ADDR_CACHE.read().await;
        if let PublicAddrCache::Set { value, expires } = cached {
            let time = SystemTime::now();
            if time.lt(expires) {
                return Some(*value);
            }
        }
    }

    // Hold the write lock to prevent others from attempting to update aswell
    let cached = &mut *PUBLIC_ADDR_CACHE.write().await;

    // API addresses for IP lookup
    let addresses = ["https://api.ipify.org/", "https://ipv4.icanhazip.com/"];
    let mut value: Option<Ipv4Addr> = None;

    // Try all addresses using the first valid value
    for address in addresses {
        let response = match reqwest::get(address).await {
            Ok(value) => value,
            Err(_) => continue,
        };

        let ip = match response.text().await {
            Ok(value) => value.trim().replace('\n', ""),
            Err(_) => continue,
        };

        if let Ok(parsed) = ip.parse() {
            value = Some(parsed);
            break;
        }
    }

    // If we couldn't connect to any IP services its likely
    // we don't have internet lets try using our local address
    if value.is_none() {
        if let Ok(IpAddr::V4(addr)) = local_ip_address::local_ip() {
            value = Some(addr)
        }
    }

    let value = value?;

    // Update cached value with the new address

    *cached = PublicAddrCache::Set {
        value,
        expires: SystemTime::now() + ADDR_CACHE_TIME,
    };

    Some(value)
}
