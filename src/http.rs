use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use axum::{extract::Query, routing::get, Router, Server};
use axum_xml_up::Xml;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize)]
pub struct QQuery {
    #[serde(rename = "prpt")]
    pub port: u16,
    #[serde(rename = "vers")]
    pub version: u32,
    pub qtyp: u32,
}

pub async fn qos(Query(query): Query<QQuery>) -> Xml<QResponse> {
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
    pub ips: QFirewallIps,
    #[serde(rename = "numinterfaces")]
    pub num_interfaces: u32,
    pub ports: QFirewallPorts,
    #[serde(rename = "requestid")]
    pub request_id: u32,
    #[serde(rename = "reqsecret")]
    pub request_secret: u32,
}

#[derive(Debug, Serialize)]
pub struct QFirewallIps {
    pub ip: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct QFirewallPorts {
    pub ports: Vec<u16>,
}

#[derive(Debug, Deserialize)]
pub struct QFirewallQuery {
    #[serde(rename = "vers")]
    pub version: u32,
    #[serde(rename = "nint")]
    pub number_interfaces: u32,
}

pub async fn firewall(Query(query): Query<QFirewallQuery>) -> Xml<QFirewall> {
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
    pub fire_type: u32,
}

#[derive(Debug, Deserialize)]
pub struct QFireTypeQuery {
    #[serde(rename = "vers")]
    pub version: u32,
    #[serde(rename = "rqid")]
    pub request_id: u32,
    #[serde(rename = "rqsc")]
    pub request_secret: u32,
    #[serde(rename = "inip")]
    pub internal_ip: i32,
    #[serde(rename = "inpt")]
    pub internal_port: u16,
}

pub async fn firetype(Query(query): Query<QFireTypeQuery>) -> Xml<QFireType> {
    let internal_ip = Ipv4Addr::from(query.internal_ip as u32);
    let internal = SocketAddrV4::new(internal_ip, query.internal_port);

    debug!("Fire type internal: {}", internal);

    Xml(QFireType { fire_type: 2 })
}
