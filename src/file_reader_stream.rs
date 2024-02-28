use std::fs::File;
use std::io::{BufReader, Read};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::thread;
use std::time::Duration;
use tokio::io;
use tokio_stream::Stream;
use crate::pb::StreamRequest;

#[derive(Debug)]
pub struct FileReaderStream {
    reader: BufReader<File>,
    meta_data: String,
    call_leg_id: String,
    is_channel_closed: bool,
}

impl FileReaderStream {
    pub fn new(file_buffer: BufReader<File>, metadata: String, leg_id: String) -> Self {
        Self { reader: file_buffer, meta_data: metadata, call_leg_id: leg_id, is_channel_closed: false }
    }

    pub fn into_inner(self) -> BufReader<File> {
        self.reader
    }
}

impl Stream for FileReaderStream {
    type Item = StreamRequest;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.is_channel_closed {
            true => Poll::Ready(None),
            false => {
                let mut line = Vec::new();
                let res = self.reader.read_to_end(&mut line);
                match res {
                    Err(err) => match err.kind() {
                        io::ErrorKind::WouldBlock => {
                            let waker = cx.waker().clone();
                            thread::spawn(move || {
                                thread::sleep(Duration::from_millis(20));
                                waker.wake();
                            });
                            Poll::Pending
                        }
                        _ => {
                            println!("error while reading from pipe: {:?}", err);
                            Poll::Ready(None)
                        }
                    }
                    Ok(count) => if count == 0 {
                        let waker = cx.waker().clone();
                        thread::spawn(move || {
                            thread::sleep(Duration::from_millis(20));
                            waker.wake();
                        });
                        Poll::Pending
                    } else {
                        // let payload: Message = json::from_str(&line).expect("could not deserialize line");

                        let mut data = line.clone();
                        if line.ends_with(&[99, 108, 111, 115, 101]) { //ends with close
                            data.truncate(line.len() - 5);
                            self.is_channel_closed = true;
                        }
                        let leg_id = self.call_leg_id.clone();
                        let metadata = self.meta_data.clone();
                        if self.is_channel_closed {
                            Poll::Ready(Some(StreamRequest {
                                call_leg_id: leg_id,
                                meta_data: "{\"action\" : \"stop\"}".parse().unwrap(),
                                audio_stream: data,
                            }))
                        } else {
                            Poll::Ready(Some(StreamRequest {
                                call_leg_id: leg_id,
                                meta_data: metadata,
                                audio_stream: data,
                            }))
                        }
                    }
                }
            }
        }
    }
}
/*
impl<T> AsRef<UnboundedReceiver<T>> for tokio_stream::wrappers::UnboundedReceiverStream<T> {
    fn as_ref(&self) -> &UnboundedReceiver<T> {
        &self.inner
    }
}

impl<T> AsMut<UnboundedReceiver<T>> for tokio_stream::wrappers::UnboundedReceiverStream<T> {
    fn as_mut(&mut self) -> &mut UnboundedReceiver<T> {
        &mut self.inner
    }
}

impl<T> From<UnboundedReceiver<T>> for tokio_stream::wrappers::UnboundedReceiverStream<T> {
    fn from(recv: UnboundedReceiver<T>) -> Self {
        Self::new(recv)
    }
}
*/