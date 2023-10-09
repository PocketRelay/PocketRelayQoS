use serde::Deserialize;
use std::{net::Ipv4Addr, path::Path};

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub http_port: u16,
    pub udp_port_1: u16,
    pub udp_port_2: u16,
    pub self_address: Ipv4Addr,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_port: 17499,
            udp_port_1: 17500,
            udp_port_2: 17501,
            self_address: Ipv4Addr::new(127, 0, 0, 1),
        }
    }
}

pub async fn load_config() -> Config {
    let file = Path::new("config.json");
    if !file.exists() {
        return Config::default();
    }
    let bytes = tokio::fs::read(file).await.expect("Failed to read config");
    serde_json::from_slice(&bytes).expect("Failed to parse config")
}
