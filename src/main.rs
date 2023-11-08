use config::load_config;
use service::QService;
use std::sync::Arc;

mod config;
mod firewall;
mod http;
mod logging;
mod service;
mod udp;

#[tokio::main]
async fn main() {
    std::env::set_var("RUST_LOG", "trace");

    logging::setup();

    let config = Arc::new(load_config().await);

    let service = Arc::new(QService::default());

    tokio::spawn(http::start_server(service.clone(), config.clone()));
    tokio::spawn(firewall::start_server(service.clone(), config.clone()));
    udp::start_server(service, config).await;
}
