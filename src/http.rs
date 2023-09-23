use std::net::{Ipv4Addr, SocketAddr};

use axum::{routing::get, Router, Server};
use axum_xml_up::Xml;
use log::{error, info};
use serde::Serialize;
use tokio::signal;

pub async fn start_server() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Create the server socket address while the port is still available
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, 25700).into();

    let router = Router::new().nest(
        "/qos",
        Router::new()
            .route("/qos", get(qos))
            .route("/firewall", get(firewall))
            .route("/firetype", get(firetype)),
    );

    info!("Starting server on {}", addr);

    if let Err(err) = Server::bind(&addr)
        .serve(router.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move {
            _ = signal::ctrl_c().await;
        })
        .await
    {
        error!("Failed to bind HTTP server on {}: {:?}", addr, err);
    }
}

#[derive(Debug, Serialize)]
#[serde(rename = "qos")]
pub struct QResponse {
    #[serde(rename = "numprobes")]
    pub num_probes: u32,
    #[serde(rename = "qosport")]
    pub qos_port: u16,
    #[serde(rename = "probesize")]
    pub probe_size: u32,
    #[serde(rename = "qosip")]
    pub qos_ip: u32,
    #[serde(rename = "requestid")]
    pub request_id: u32,
    #[serde(rename = "reqsecret")]
    pub request_secret: u32,
}

pub async fn qos() -> Xml<QResponse> {
    Xml(QResponse {
        num_probes: 0,
        qos_port: 17499,
        probe_size: 0,
        qos_ip: u32::from_be_bytes([127, 0, 0, 1]),
        request_id: 1,
        request_secret: 0,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename = "firewall")]
pub struct QFirewall {
    ips: QFirewallIps,
    #[serde(rename = "numinterfaces")]
    num_interfaces: u32,
    ports: QFirewallPorts,
    #[serde(rename = "requestid")]
    request_id: u32,
    #[serde(rename = "reqsecret")]
    request_secret: u32,
}

#[derive(Debug, Serialize)]
pub struct QFirewallIps {
    ip: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct QFirewallPorts {
    ports: Vec<u16>,
}

pub async fn firewall() -> Xml<QFirewall> {
    Xml(QFirewall {
        ips: QFirewallIps { ip: vec![1] },
        num_interfaces: 1,
        ports: QFirewallPorts { ports: vec![1] },
        request_id: 1,
        request_secret: 0,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename = "firetype")]
pub struct QFireType {
    #[serde(rename = "firetype")]
    fire_type: u32,
}

pub async fn firetype() -> Xml<QFireType> {
    Xml(QFireType { fire_type: 2 })
}
