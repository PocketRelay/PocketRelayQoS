use std::{collections::HashMap, net::SocketAddr, sync::atomic::AtomicU32};

use rand::{rngs::OsRng, RngCore};
use tokio::sync::{mpsc, RwLock};

type RequestId = u32;
type RequestSecret = u32;

#[derive(Default)]
pub struct QService {
    pub m1: RwLock<HashMap<(RequestId, RequestSecret), QRequestData>>,
    pub m2: RwLock<HashMap<(RequestId, RequestSecret), QFirewallData>>,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(2);

impl QService {
    pub async fn _get_request_data(
        &self,
        id: RequestId,
        secret: RequestSecret,
    ) -> Option<QRequestData> {
        let m1 = &*self.m1.read().await;
        m1.get(&(id, secret)).cloned()
    }

    pub async fn create_request_data(
        &self,
        q_type: u32,

        client_port: u16,
        version: u32,
    ) -> (RequestId, RequestSecret) {
        let m1 = &mut *self.m1.write().await;

        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let mut rand = OsRng;
        let secret: u32 = loop {
            let secret = (rand.next_u32() as u16) as u32;
            if m1.contains_key(&(id, secret)) {
                continue;
            }
            break secret;
        };

        let data = QRequestData {
            q_type,

            client_port,
            version,
        };

        m1.insert((id, secret), data);

        (id, secret)
    }

    pub async fn create_firewall_data(&self) -> (RequestId, RequestSecret) {
        let m2 = &mut *self.m2.write().await;

        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let mut rand = OsRng;
        let secret: u32 = loop {
            let secret = (rand.next_u32() as u16) as u32;
            if m2.contains_key(&(id, secret)) {
                continue;
            }
            break secret;
        };

        let (tx, rx) = mpsc::unbounded_channel();

        let data = QFirewallData { tx, rx: Some(rx) };

        m2.insert((id, secret), data);

        (id, secret)
    }

    pub async fn get_firewall_tx(
        &self,
        id: RequestId,
        secret: RequestSecret,
    ) -> Option<mpsc::UnboundedSender<SocketAddr>> {
        let m2 = &*self.m2.read().await;
        m2.get(&(id, secret)).map(|value| value.tx.clone())
    }

    pub async fn take_firewall_rx(
        &self,
        id: RequestId,
        secret: RequestSecret,
    ) -> Option<mpsc::UnboundedReceiver<SocketAddr>> {
        let m2 = &mut *self.m2.write().await;
        m2.get_mut(&(id, secret)).and_then(|value| value.rx.take())
    }
}

#[derive(Clone, Debug)]
pub struct QRequestData {
    pub q_type: u32,
    pub client_port: u16,
    pub version: u32,
}

pub struct QFirewallData {
    tx: mpsc::UnboundedSender<SocketAddr>,
    rx: Option<mpsc::UnboundedReceiver<SocketAddr>>,
}
