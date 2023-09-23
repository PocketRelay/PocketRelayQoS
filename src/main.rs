mod http;
mod udp;

#[tokio::main]
async fn main() {
    http::start_server().await;
}
