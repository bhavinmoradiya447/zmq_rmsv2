pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::net::ToSocketAddrs;
use std::fs::OpenOptions;
use std::io::Write;

use tonic::{transport::Server, Request, Response, Status, Streaming};

use pb::{StreamRequest, StreamResponse};

#[derive(Debug)]
pub struct StreamServer {}

#[tonic::async_trait]
impl pb::audio_stream_server::AudioStream for StreamServer {
    async fn client_streaming_audio(&self, request: Request<Streaming<StreamRequest>>) -> Result<Response<StreamResponse>, Status> {
        let mut stream = request.into_inner();
        // listening on stream
        while let Some(req) = stream.message().await? {
            let interaction_id = req.call_leg_id;
            let audio_stream = req.audio_stream;

            OpenOptions::new()
                .append(true)
                .create(true)
                .open("/tmp/".to_owned() + &*interaction_id + ".raw")
                .expect("Failed to open a file")
                .write(&*audio_stream)
                .expect("Failed to write into file");
        }
        // returning response
        Ok(Response::new(StreamResponse { message: "Done".to_string() }))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = StreamServer {};
    Server::builder()
        .add_service(pb::audio_stream_server::AudioStreamServer::new(server))
        .serve("[::]:5557".to_socket_addrs().unwrap().next().unwrap())
        .await
        .unwrap();

    Ok(())
}
