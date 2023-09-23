use std::{net::Ipv4Addr, sync::Arc};

use service::QService;

mod http;
mod service;
mod udp;

#[tokio::main]
async fn main() {
    let service = Arc::new(QService::default());

    tokio::spawn(http::start_server(service.clone()));
    udp::start_server(service).await;
}
