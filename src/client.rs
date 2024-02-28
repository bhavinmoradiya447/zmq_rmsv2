mod file_reader_stream;

pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::{fs, io};
use std::fs::File;
use std::path::Path;
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use tokio_stream::StreamExt;
use tonic::transport::{Channel};
use unix_named_pipe::FileFIFOExt;
use pb::{audio_stream_client::AudioStreamClient};


#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    host_name: String,
    metadata: String,
    audio_data: String,
    action: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    /*    let channel = Endpoint::from_static("http://10.192.133.169:5557")
            .connect()
            .await?;
    */

    loop {
        let envelope = subscriber
            .recv_string(0)
            .expect("failed receiving envelope")
            .expect("failed to convert to String");

        //println!("Received message with size: {} ", envelope);
        let data: Data = serde_json::from_str(&*envelope).unwrap();
        let host_name = data.host_name.clone();
        match data.action.as_str() {
            "init" => {
                println!("{}", envelope);
                let mut client = AudioStreamClient::connect(host_name).await?; //AudioStreamClient::new(channel.clone());
                let file_name = "/tmp/".to_owned() + &*data.call_leg_id.clone();
                let file = try_open(&file_name).expect("could not open pipe for reading");
                tokio::spawn(async move {
                    init_streaming_audio(&mut client, data, file).await;
                    drop(client);
                });
            }
            /*"audio_stream" => {
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
            }*/
            "close" => {
                println!("{}", envelope);
                /*match session_map.lock().unwrap().remove(&data.call_leg_id) {
                    Some(tx) => drop(tx),
                    _ => println!("No Client present to close")
                }*/
            }
            _ => println!("No matching case"),
        }
        sleep(Duration::from_millis(10)).await;
    }
}

async fn init_streaming_audio(client: &mut AudioStreamClient<Channel>, data: Data, file: File) {
    let reader = io::BufReader::new(file);

    let file_name = "/tmp/".to_owned() + &*data.call_leg_id.clone();

    let stream = file_reader_stream::FileReaderStream::new(reader, data.metadata, data.call_leg_id)
        .throttle(Duration::from_millis(80));

    let response = client
        .client_streaming_audio(stream)
        .await.unwrap().into_inner();
    println!("RESPONSE=\n{}", response.message);
    fs::remove_file(&file_name).expect(&*format!("could not remove pipe file {}", file_name));
    return;
}

fn try_open<P: AsRef<Path> + Clone>(pipe_path: P) -> io::Result<fs::File> {
    let pipe = unix_named_pipe::open_read(&pipe_path);
    if let Err(err) = pipe {
        match err.kind() {
            io::ErrorKind::NotFound => {
                println!("creating pipe at: {:?}", pipe_path.clone().as_ref());
                unix_named_pipe::create(&pipe_path, Some(0o660))?;

                // Note that this has the possibility to recurse forever if creation `open_write`
                // fails repeatedly with `io::ErrorKind::NotFound`, which is certainly not nice behaviour.
                return try_open(pipe_path);
            }
            _ => {
                return Err(err);
            }
        }
    }

    let pipe_file = pipe.unwrap();
    let is_fifo = pipe_file
        .is_fifo()
        .expect("could not read type of file at pipe path");
    if !is_fifo {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "expected file at {:?} to be fifo, is actually {:?}",
                &pipe_path.clone().as_ref(),
                pipe_file.metadata()?.file_type(),
            ),
        ));
    }

    Ok(pipe_file)
}

