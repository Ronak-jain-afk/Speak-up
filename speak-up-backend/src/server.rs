use tokio::net::TcpListener;

pub async fn start_server(port: u16) {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr).await.unwrap();
    tracing::info!("Backend listening on {}", addr);

    while let Ok((stream, peer)) = listener.accept().await {
        tracing::debug!("Connection from {}", peer);
        tokio::spawn(handle_connection(stream));
    }
}

async fn handle_connection(_stream: tokio::net::TcpStream) {
    unimplemented!("Phase 3")
}
