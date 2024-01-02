pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use base64::{Engine as _, engine::{general_purpose}};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::sleep;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::{Channel, Endpoint};

use pb::{audio_stream_client::AudioStreamClient, StreamRequest};


#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    metadata: String,
    audio_data: String,
    action: String,
}

type Tx = UnboundedSender<StreamRequest>;
type SessionMap = Arc<Mutex<HashMap<String, Tx>>>;

#[tokio::main(worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    let channel = Endpoint::from_static("http://[::]:5557")
        .connect()
        .await?;

    let session_map = SessionMap::new(Mutex::new(HashMap::new()));

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
                let (tx, rx) = mpsc::unbounded_channel::<StreamRequest>();

                session_map.lock().unwrap().insert(data.call_leg_id, tx);
                tokio::spawn(async move { init_streaming_audio(&mut client, rx).await; });
            }
            "audio_stream" => {
                let bytes = general_purpose::STANDARD
                    .decode(data.audio_data).unwrap();

                match session_map.lock().unwrap().get(&data.call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: data.call_leg_id,
                            meta_data: data.metadata,
                            audio_stream: bytes,
                        }).expect("Failed to send Message");
                    }
                    _ => {
                        println!("No Client present to stream");
                    }
                }
            }
            "send_text" => {
                println!("{}", envelope);
                match session_map.lock().unwrap().get(&data.call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: data.call_leg_id,
                            meta_data: data.metadata,
                            audio_stream: Vec::<u8>::new(),
                        }).expect("Error Sending text message");
                    }
                    _ => {
                        println!("No Client present to stream");
                    }
                }
            }
            "close" => {
                println!("{}", envelope);
                match session_map.lock().unwrap().remove(&data.call_leg_id) {
                    Some(tx) => drop(tx),
                    _ => println!("No Client present to close")
                }
            }
            _ => println!("No matching case"),
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn init_streaming_audio(client: &mut AudioStreamClient<Channel>, rx: UnboundedReceiver<StreamRequest>) {
    let response = client
        .client_streaming_audio(UnboundedReceiverStream::new(rx))
        .await.unwrap().into_inner();
    println!("RESPONSE=\n{}", response.message);
}