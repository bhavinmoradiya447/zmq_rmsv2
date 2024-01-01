use std::fs::OpenOptions;
use std::io::Write;
use futures_util::StreamExt;
use log::*;
use std::net::SocketAddr;
use base64::Engine;
use base64::engine::general_purpose;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error, Result},
};

#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    metadata: String,
    audio_data: String,
    action: String,
}

async fn accept_connection(peer: SocketAddr, stream: TcpStream) {
    if let Err(e) = handle_connection(peer, stream).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream) -> Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    info!("New WebSocket connection: {}", peer);

    while let Some(msg) = ws_stream.next().await {
        let msg = msg?;
        if msg.is_text() {
            let data: Data = serde_json::from_str(&*msg.to_string()).unwrap();
            let bytes = general_purpose::STANDARD
                .decode(data.audio_data).unwrap();
            OpenOptions::new()
                .append(true)
                .create(true)
                .open("/tmp/".to_owned() + &*data.call_leg_id + ".raw")
                .expect("Failed to open a file")
                .write(&*bytes)
                .expect("Failed to write into file");
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let addr = ":::5557";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream));
    }
}