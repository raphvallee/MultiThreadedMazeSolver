//! Binary entry point: `maze-server [host:port]` (default 127.0.0.1:8173).

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8173".to_string());
    let listener = TcpListener::bind(&addr).await.expect("failed to bind");
    println!("maze-server listening on http://{addr}");
    maze_server::serve(listener).await;
}