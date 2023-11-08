use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::{Buf, BufMut, BytesMut};
use log::{debug, error, info};
use tokio::{net::UdpSocket, sync::RwLock};

use crate::{config::Config, service::QService};

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

        if !buffer.is_empty() {
            debug!("QoS v1 message still had more bytes: {:?}", buffer.as_ref());
        }

        Self { timestamp }
    }
}

#[derive(Debug)]
pub struct QosRequestV2 {
    pub probe_count: u32,
    pub payload: BytesMut,
}

impl QosRequestV2 {
    pub fn from_buffer(buffer: &mut BytesMut) -> Self {
        let probe_count = buffer.get_u32();
        let payload = buffer.split();
        Self {
            probe_count,
            payload,
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
    pub payload: BytesMut,
}

impl QosResponseV2 {
    pub fn write(&self, out: &mut BytesMut) {
        self.header.write(out);
        out.put_u32_le(self.probe_count);
        out.put_u32(self.ubps);
        out.put_u16(self.port);
        out.extend_from_slice(&self.payload);
    }
}

pub async fn start_server(service: Arc<QService>, config: Arc<Config>) {
    // Socket for handling connections
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, config.udp_port_1))
        .await
        .unwrap();

    info!("Starting QoS server on 0.0.0.0:{}", config.udp_port_1);

    let socket = Arc::new(socket);

    // Buffer for reciving messages
    let mut buffer = [0u8; 65536 /* UDP allocated buffer size */];

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
    _service: Arc<QService>,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    mut buffer: BytesMut,
) {
    if buffer.len() < 16 {
        error!(
            "Client didn't send a message long enough to be a header: {:?}",
            buffer.as_ref()
        );
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
    let time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let mut out: BytesMut = BytesMut::new();

    let mut public_ip = *addr.ip();
    // Only lookup public address of server if in debug mode
    if cfg!(debug_assertions) && (public_ip.is_loopback() || public_ip.is_private()) {
        if let Some(ip) = public_address().await {
            public_ip = ip;
        }
    }

    if header.request_id == 1 && header.request_secret == 0 {
        let request = QosRequestV1::from_buffer(&mut buffer);

        let response = QosResponseV1 {
            header: header.clone(),
            timestamp: request.timestamp,
            // ip: *addr.ip(),
            ip: public_ip,
            port: addr.port(),
        };
        debug!(
            "RECV: {:?} AT: {:?}  DATA: {:?} RESP: {:?}",
            &header,
            time.as_millis(),
            &request,
            &response
        );

        response.write(&mut out);
    } else {
        let request = QosRequestV2::from_buffer(&mut buffer);

        let mut payload = request.payload.clone();

        // Drop 6 bytes from the payload to fit the ubps and port1
        payload.truncate(payload.len() - 6);

        let response = QosResponseV2 {
            header: header.clone(),
            probe_count: request.probe_count,
            ubps: u32::from_be_bytes([0x00, 0x5b, 0x8d, 0x80]),
            port: addr.port(),
            payload,
        };

        debug!(
            "RECV: {:?} AT: {:?}  DATA: {:?} RESP: {:?}",
            &header,
            time.as_millis(),
            &request,
            &response
        );
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

    let value = value?;

    // Update cached value with the new address

    *cached = PublicAddrCache::Set {
        value,
        expires: SystemTime::now() + ADDR_CACHE_TIME,
    };

    Some(value)
}

#[test]
fn bytes() {
    println!("{}", Ipv4Addr::from(1987012967u32.to_be_bytes()));
    println!("{}", -2146697216i32 as u32);
}
