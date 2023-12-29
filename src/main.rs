use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use std::thread::sleep;
use base64::{Engine as _, engine::{general_purpose}};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    metadata: String,
    audio_data: String,
    action: String,
}

fn main() {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    loop {
        let envelope = subscriber
            .recv_string(0)
            .expect("failed receiving envelope")
            .expect("failed to convert to String");

        //println!("Received message with size: {} ", envelope);
        let data: Data = serde_json::from_str(&*envelope).unwrap();

        match data.action.as_str() {
            "init" => println!("{}", envelope),
            "audio_stream" => {
                let bytes = general_purpose::STANDARD
                    .decode(data.audio_data).unwrap();

                OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("/tmp/".to_owned() + &*data.call_leg_id + ".raw")
                    .expect("Failed to open a file")
                    .write(&*bytes)
                    .expect("Failed to write into file");


                //    println!("Received message with size: [{}]", envelope.len());
                // sleep(Duration::from_millis(10))
            }
            "send_text" => println!("{}", envelope),
            "close" => println!("{}", envelope),
            _ => println!("No matching case"),
        }
    }
}