use std::sync::Arc;

use http::firewall;
use service::QService;

mod constants;
mod firewall;
mod http;
mod service;
mod udp;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let service = Arc::new(QService::default());

    tokio::spawn(http::start_server(service.clone()));
    tokio::spawn(firewall::start_server(service.clone()));
    udp::start_server(service).await;
}
