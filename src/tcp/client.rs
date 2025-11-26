use crate::godot_types::ClientConnected;
use crate::types::buffer_codec::BufferCodec;
use crossbeam::channel::{Receiver, Sender, unbounded};
use godot::global::godot_error;
use network_types::connection::Packet;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

pub struct TcpClient {
    running: Arc<AtomicBool>,
    tx: Sender<Vec<u8>>,
    rx: Receiver<Vec<u8>>,
    stream: Option<TcpStream>,
    reader_handler: Option<JoinHandle<std::io::Result<()>>>,
}

impl TcpClient {
    pub fn new() -> Self {
        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = unbounded();
        Self {
            running: Arc::new(AtomicBool::new(false)),
            tx,
            rx,
            stream: None,
            reader_handler: None,
        }
    }

    pub fn connect(&mut self, remote: String, name: String) -> std::io::Result<Packet> {
        let mut stream = TcpStream::connect(remote.as_str())?;
        println!("Connected to server.");

        self.running.store(true, Ordering::Relaxed);

        stream
            .write(Packet::LoginRequest { name }.serialize().as_slice())
            .unwrap();

        let packet = {
            let mut buffer = [0u8; 2048 * 4];
            let total = stream.read(&mut buffer)?;
            Packet::from(&buffer[..total])
        };

        let mut reader = stream.try_clone()?;
        self.stream = Some(stream);

        let reader_running = self.running.clone();
        let tx = self.tx.clone();
        self.reader_handler = Some(thread::spawn(move || -> std::io::Result<()> {
            while reader_running.load(Ordering::Relaxed) {
                // Read server response
                // TODO Change buffer size dinamically
                let mut buffer = [0u8; 2048 * 4];
                // TODO handle this better
                let bytes_read = reader.read(&mut buffer).unwrap();
                if bytes_read > 0 {
                    match tx.send(buffer[..bytes_read].to_vec()) {
                        Ok(_) => {}
                        Err(err) => {
                            eprintln!("{:?}", err);
                        }
                    }
                }
            }
            Ok(())
        }));

        Ok(packet)
    }

    pub fn send(&mut self, buffer: &[u8]) {
        if let Some(stream) = self.stream.as_mut() {
            match stream.write_all(buffer) {
                Err(err) => godot_error!("{:?}", err),
                _ => {}
            }
        } else {
            godot_error!("No stream to send message");
        }
    }

    pub fn get_receiver(&self) -> Receiver<Vec<u8>> {
        return self.rx.clone();
    }

    // pub fn join(&mut self) {
    //     if let Some(reader) = self.reader_handler.take() {
    //         reader.join().unwrap().unwrap();
    //     }
    // }
}
