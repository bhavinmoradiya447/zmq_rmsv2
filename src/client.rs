pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::collections::HashMap;
use std::{fs, io};
use std::io::BufRead;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::time::sleep;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tonic::transport::{Channel, Endpoint};
use unix_named_pipe::FileFIFOExt;

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

#[tokio::main]
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

                let call_leg_id = data.call_leg_id.clone();
                let path = "/tmp/".to_owned() + &*data.call_leg_id.clone();

                let meta_data = data.metadata.clone();
                session_map.lock().unwrap().insert(data.call_leg_id, tx);
                tokio::spawn(async move { init_streaming_audio(&mut client, rx).await; });
                tokio::spawn(async move { read_from_named_pipe(path, call_leg_id, meta_data, session_map.clone()) });
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

async fn init_streaming_audio(client: &mut AudioStreamClient<Channel>, rx: UnboundedReceiver<StreamRequest>) {
    let response = client
        .client_streaming_audio(UnboundedReceiverStream::new(rx))
        .await.unwrap().into_inner();
    println!("RESPONSE=\n{}", response.message);
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

fn read_from_named_pipe(file_name: String, call_leg_id: String, meta_data: String, session_map: SessionMap) {
    println!("server opening pipe: {}", file_name);

    // Set up a keyboard interrupt handler so we can remove the pipe when
    // the process is shut down.

    // Open the pipe file for reading
    let file = try_open(&file_name).expect("could not open pipe for reading");
    let mut reader = io::BufReader::new(file);

    // Loop reading from the pipe until a keyboard interrupt is received
    loop {

        // If an error occurs during read, panic
        let mut line = String::new();
        let res = reader.read_line(&mut line);
        if let Err(err) = res {
            // Named pipes, by design, only support nonblocking reads and writes.
            // If a read would block, an error is thrown, but we can safely ignore it.
            match err.kind() {
                io::ErrorKind::WouldBlock => continue,
                _ => panic!("error while reading from pipe: {:?}", err),
            }
        } else if let Ok(count) = res {
            if count == 0 {
                std::thread::sleep(Duration::from_millis(2));
                continue;
            } else {
                let mut data = line.clone();
                let mut close = false;
                // let payload: Message = json::from_str(&line).expect("could not deserialize line");
                if line.ends_with("close") {
                    data = data.replace("close", "");
                    close = true;
                }
                let leg_id = call_leg_id.clone();
                let metadata = meta_data.clone();
                match session_map.lock().unwrap().get(&call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: leg_id,
                            meta_data: metadata,
                            audio_stream: Vec::from(data),
                        }).expect("Error Sending text message");
                    }
                    _ => {
                        println!("No Client present to stream");
                    }
                }
                if close {
                    match session_map.lock().unwrap().remove(&call_leg_id) {
                        Some(tx) => drop(tx),
                        _ => println!("No Client present to close")
                    }
                    fs::remove_file(&file_name).expect("could not remove pipe during shutdown");
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}