use std::{
    future::Future,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    sync::Arc,
};

use axum::{extract::Query, routing::get, Extension, Router, Server};
use axum_xml_up::Xml;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use tokio::signal;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use crate::{config::Config, service::QService};

pub async fn start_server(service: Arc<QService>, config: Arc<Config>) {
    // Create the server socket address while the port is still available
    let addr: SocketAddr = (Ipv4Addr::UNSPECIFIED, config.http_port).into();

    let router = Router::new()
        .nest(
            "/qos",
            Router::new()
                .route("/qos", get(qos))
                .route("/firewall", get(firewall))
                .route("/firetype", get(firetype)),
        )
        .layer(Extension(service))
        .layer(Extension(config))
        .layer(
            TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::new().include_headers(true)),
        );

    info!("Starting HTTP server on {}", addr);

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

/// QoS type for public facing address information
pub const QOS_TYPE_ADDRESS: u32 = 1;
/// QoS type for checking latency
pub const QOS_TYPE_LATENCY: u32 = 2;

/// Number of probes the client should send when checking latency
pub const LATENCY_PROBE_COUNT: u32 = 5;
/// Size of the latency probes the client should send
pub const LATENCY_PROBE_SIZE: u32 = 60;

pub async fn qos(
    Query(query): Query<QQuery>,
    Extension(service): Extension<Arc<QService>>,
    Extension(config): Extension<Arc<Config>>,
) -> Xml<QResponse> {
    let qos_ip = u32::from_be_bytes(config.self_address.octets());
    let qos_port = config.udp_port_1;

    let response_fut: Pin<Box<dyn Future<Output = QResponse> + Send>> = match query.qtyp {
        QOS_TYPE_ADDRESS => Box::pin(qos_address(qos_ip, qos_port)),
        QOS_TYPE_LATENCY => Box::pin(qos_latency(service, query, qos_ip, qos_port)),
        _ => Box::pin(qos_unknown(query)),
    };

    let response = response_fut.await;
    Xml(response)
}

async fn qos_address(qos_ip: u32, qos_port: u16) -> QResponse {
    QResponse {
        num_probes: 0,
        qos_port,
        probe_size: 0,
        qos_ip,
        request_id: 1,
        request_secret: 0,
    }
}

async fn qos_latency(
    service: Arc<QService>,
    query: QQuery,
    qos_ip: u32,
    qos_port: u16,
) -> QResponse {
    let (request_id, request_secret) = service
        .create_request_data(query.qtyp, query.port, query.version)
        .await;

    debug!("QResponse: {} {}", request_id, request_secret);

    QResponse {
        num_probes: LATENCY_PROBE_COUNT,
        qos_port,
        probe_size: LATENCY_PROBE_SIZE,
        qos_ip,
        request_id,
        request_secret,
    }
}

async fn qos_unknown(query: QQuery) -> QResponse {
    debug!("Unknown qos type query: {:?}", query);

    QResponse {
        num_probes: 0,
        qos_port: 0,
        probe_size: 0,
        qos_ip: 0,
        request_id: 0,
        request_secret: 0,
    }
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

pub async fn firewall(
    Query(query): Query<QFirewallQuery>,
    Extension(service): Extension<Arc<QService>>,
    Extension(config): Extension<Arc<Config>>,
) -> Xml<QFirewall> {
    debug!("Firewall query: {:?}", query);

    let (request_id, request_secret) = service.create_firewall_data().await;

    Xml(QFirewall {
        ips: QFirewallIps {
            ip: vec![u32::from_be_bytes(config.self_address.octets())],
        },
        num_interfaces: 1,
        ports: QFirewallPorts {
            ports: vec![config.udp_port_2],
        },
        request_id,
        request_secret,
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

pub async fn firetype(
    Query(query): Query<QFireTypeQuery>,
    Extension(service): Extension<Arc<QService>>,
) -> Xml<QFireType> {
    debug!("Firetype query: {:?}", query);

    let internal_ip = Ipv4Addr::from(query.internal_ip as u32);
    let internal = SocketAddrV4::new(internal_ip, query.internal_port);
    debug!("Fire type internal: {}", internal);
    let mut rx = service
        .take_firewall_rx(query.request_id, query.request_secret)
        .await
        .expect("Missing firewall rx");
    debug!("Firetype got rx handle, waiting for connections..");

    let mut addrs: Vec<SocketAddr> = Vec::with_capacity(5);

    loop {
        let addr = match rx.recv().await {
            Some(value) => value,
            None => break,
        };
        addrs.push(addr);
        debug!("Firetype got connection: {}", addr);

        if addrs.len() >= 5 {
            break;
        }
    }
    debug!("Firetype connections complete: {:?}", addrs);

    Xml(QFireType { fire_type: 2 })
}
