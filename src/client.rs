pub mod pb {
    tonic::include_proto!("rmsv2.audiostream");
}

use std::collections::HashMap;
use std::{fs, io, thread};
use std::io::{Read};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use futures::channel::mpsc;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamExt;
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    console_subscriber::init();
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    let channel = futures::executor::block_on(Endpoint::from_static("http://10.192.133.169:5557")
        .connect()).expect("Error connecting channel");


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
                let (tx, rx) = mpsc::unbounded::<StreamRequest>();

                let call_leg_id = data.call_leg_id.clone();
                let path = "/tmp/".to_owned() + &*data.call_leg_id.clone();

                let meta_data = data.metadata.clone();
                session_map.lock().unwrap().insert(data.call_leg_id, tx);
                thread::spawn(move || { init_streaming_audio(&mut client, rx); });
                let map = session_map.clone();
                thread::spawn(move || { read_from_named_pipe(path, call_leg_id, meta_data, map) });
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
        thread::sleep(Duration::from_millis(10));
    }
}

fn init_streaming_audio(client: &mut AudioStreamClient<Channel>, rx: UnboundedReceiver<StreamRequest>) {
    let stream = rx.throttle(Duration::from_millis(2));
    //let stream = UnboundedReceiverStream::new(rx).throttle(Duration::from_millis(2));
    let response = futures::executor::block_on(client
        .client_streaming_audio(stream)).expect("Failed to get response").into_inner();
    println!("RESPONSE=\n{}", response.message);
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
        let mut line = Vec::new();
        let res = reader.read_to_end(&mut line);
        if let Err(err) = res {
            // Named pipes, by design, only support nonblocking reads and writes.
            // If a read would block, an error is thrown, but we can safely ignore it.
            match err.kind() {
                io::ErrorKind::WouldBlock => continue,
                _ => panic!("error while reading from pipe: {:?}", err),
            }
        } else if let Ok(count) = res {
            if count == 0 {
                thread::sleep(Duration::from_millis(2));
                continue;
            } else {
                let mut data = line.clone();
                let mut close = false;
                // let payload: Message = json::from_str(&line).expect("could not deserialize line");
                if line.ends_with(&[99, 108, 111, 115, 101]) { //ends with close
                    data.truncate(line.len() - 5);
                    close = true;
                }
                let leg_id = call_leg_id.clone();
                let metadata = meta_data.clone();
                match session_map.lock().unwrap().get(&call_leg_id) {
                    Some(tx) => {
                        tx.send(StreamRequest {
                            call_leg_id: leg_id,
                            meta_data: metadata,
                            audio_stream: data,
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
        thread::sleep(Duration::from_millis(20));
    }
}