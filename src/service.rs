use std::{collections::HashMap, sync::atomic::AtomicU32};

use rand::{rngs::OsRng, RngCore};
use tokio::sync::RwLock;

type RequestId = u32;
type RequestSecret = u32;

#[derive(Default)]
pub struct QService {
    pub m1: RwLock<HashMap<(RequestId, RequestSecret), QRequestData>>,
    pub m2: RwLock<HashMap<(RequestId, RequestSecret), QFirewallData>>,
}

static NEXT_ID: AtomicU32 = AtomicU32::new(0);
static NEXT_SECRET: AtomicU32 = AtomicU32::new(0);

impl QService {
    pub async fn get_request_data(
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
        num_probes: u32,
        probe_size: u32,
        client_port: u16,
        version: u32,
    ) -> (RequestId, RequestSecret) {
        let m1 = &mut *self.m1.write().await;

        let id = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        let mut rand = OsRng;
        let secret: u32 = loop {
            let secret = rand.next_u32();
            if m1.contains_key(&(id, secret)) {
                continue;
            }
            break secret;
        };

        let data = QRequestData {
            q_type,
            num_probes,
            probe_size,
            client_port,
            version,
        };

        m1.insert((id, secret), data);

        (id, secret)
    }
}

#[derive(Clone, Debug)]
pub struct QRequestData {
    pub q_type: u32,
    pub num_probes: u32,
    pub probe_size: u32,
    pub client_port: u16,
    pub version: u32,
}

pub struct QFirewallData {}
