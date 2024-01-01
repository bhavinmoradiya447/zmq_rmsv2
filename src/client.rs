pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::collections::HashMap;
use base64::{Engine as _, engine::{general_purpose}};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::{Channel, Endpoint};

use pb::{audio_stream_client::AudioStreamClient, StreamRequest};


#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    metadata: String,
    audio_data: String,
    action: String,
}

#[tokio::main(worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    let channel = Endpoint::from_static("http://[::1]:50051")
        .connect()
        .await?;

    let mut sender_map = HashMap::<String, Sender::<StreamRequest>>::new();

    loop {
        let envelope = subscriber
            .recv_string(0)
            .expect("failed receiving envelope")
            .expect("failed to convert to String");

        //println!("Received message with size: {} ", envelope);
        let data: Data = serde_json::from_str(&*envelope).unwrap();

        match data.action.as_str() {
            "init" => {
                println!("{}", envelope);
                let mut client = AudioStreamClient::new(channel.clone());
                let (tx, rx) = mpsc::channel::<StreamRequest>(200);
                sender_map.insert(data.call_leg_id, tx);
                tokio::spawn(async move { init_streaming_audio(&mut client, rx).await; });
            }
            "audio_stream" => {
                let bytes = general_purpose::STANDARD
                    .decode(data.audio_data).unwrap();

                match sender_map.get(&data.call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: data.call_leg_id,
                            meta_data: data.metadata,
                            audio_stream: bytes,
                        }).await.expect("Error sending Audio Stream");
                    }
                    _ => {
                        println!("No Client present to stream");
                    }
                }
            }
            "send_text" => {
                println!("{}", envelope);
                match sender_map.get(&data.call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: data.call_leg_id,
                            meta_data: data.metadata,
                            audio_stream: Vec::<u8>::new(),
                        }).await.expect("Error Sending text message");
                    }
                    _ => {
                        println!("No Client present to stream");
                    }
                }
            }
            "close" => {
                println!("{}", envelope);
                match sender_map.remove(&data.call_leg_id) {
                    Some(tx) => drop(tx),
                    _ => println!("No Client present to close")
                }
            }
            _ => println!("No matching case"),
        }
    }
}

async fn init_streaming_audio(client: &mut AudioStreamClient<Channel>, rx: Receiver<StreamRequest>) {
    let response = client
        .client_streaming_audio(ReceiverStream::new(rx))
        .await.unwrap().into_inner();
    println!("RESPONSE=\n{}", response.message);
}