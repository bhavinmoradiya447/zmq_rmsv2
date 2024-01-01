mod server;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use futures_channel::mpsc::{unbounded, UnboundedSender};
use futures_util::StreamExt;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Serialize, Deserialize)]
struct Data {
    call_leg_id: String,
    metadata: String,
    audio_data: String,
    action: String,
}

type Tx = UnboundedSender<Message>;
type SessionMap = Arc<Mutex<HashMap<String, Tx>>>;


#[tokio::main]
async fn main() {
    let context = zmq::Context::new();
    let subscriber = context.socket(zmq::SUB).unwrap();
    subscriber
        .connect("tcp://127.0.0.1:9090")
        .expect("failed connecting subscriber");
    subscriber.set_subscribe(b"").expect("failed subscribing");

    let connect_addr = "ws://127.0.0.1:5557";


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
                let (tx, rx) = unbounded();
                let url = url::Url::parse(&connect_addr).unwrap();
                session_map.lock().unwrap().insert(data.call_leg_id, tx);
                let _ = tokio::spawn(async move {
                    let (ws_stream, _) = connect_async(url).await.expect("Failed to connect");
                    let (write, _read) = ws_stream.split();
                    rx.map(Ok).forward(write).await.expect("Send failed");
                });
                /* let ws_to_stdout = {
                     read.for_each(|message| async {
                         let data = message.unwrap().into_data();
                         tokio::io::stdout().write_all(&data).await.unwrap();
                     })
                 };*/

                //pin_mut!(stdin_to_ws);//, ws_to_stdout);
                //stdin_to_ws, ()).await;//, ws_to_stdout).await;
            }
            "audio_stream" => {
                match session_map.lock().unwrap().get(&data.call_leg_id) {
                    Some(tx) => { let _ = tx.unbounded_send(Message::Text(envelope)); }
                    _ => println!("tx now found")
                }
                //    println!("Received message with size: [{}]", envelope.len());
                // sleep(Duration::from_millis(10))
            }
            "send_text" => println!("{}", envelope),
            "close" => {
                match session_map.lock().unwrap().remove(&data.call_leg_id) {
                    None => println!("Call leg id not found"),
                    Some(tx) => drop(tx)
                };
            }
            _ => println!("No matching case"),
        }
    }
}