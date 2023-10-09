use config::load_config;
use service::QService;
use std::sync::Arc;

mod config;
mod constants;
mod firewall;
mod http;
mod logging;
mod service;
mod udp;

#[tokio::main]
async fn main() {
    logging::setup();

    let config = Arc::new(load_config().await);

    let service = Arc::new(QService::default());

    tokio::spawn(http::start_server(service.clone(), config.clone()));
    tokio::spawn(firewall::start_server(service.clone(), config));
    udp::start_server(service).await;
}
