use std::io::{self, Error, Write};
use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

use crossbeam::channel::{Receiver, Sender};
use godot::global::godot_print;
use network_types::connection::Packet;

pub fn client(
    server_addr: String,
    room_id: i32,
    tx: Sender<Packet>,
    rx: Receiver<Packet>,
    running: Arc<AtomicBool>,
) -> std::io::Result<Vec<JoinHandle<()>>> {
    // Bind to an ephemeral local port (0 = let OS choose)
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.connect(server_addr.as_str())?;

    // TODO add a timeout
    godot_print!("UDP client started. Connected to {}", server_addr);
    let mut connected = false;
    for _ in 0..10 {
        godot_print!("Waiting for handshake...");
        if let Err(_) = socket.send(Packet::JoinMatch { room_id }.serialize().as_slice()) {
            continue;
        }
        let mut buf = [0u8; 2048];
        if let Ok(len) = socket.recv(&mut buf) {
            if let Ok(packet) = Packet::try_from(&buf[..len]) {
                match packet {
                    Packet::Ping => {
                        godot_print!("Handshake received");
                        connected = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if !connected {
        return Err(Error::new(io::ErrorKind::Deadlock, "Timeout"));
    }

    // Clone socket for receiving thread
    let recv_socket = socket.try_clone()?;
    let main_running = running.clone();

    let mut join_handlers: Vec<JoinHandle<()>> = Vec::new();

    //
    // MAIN THREAD — Send user input to server
    //
    join_handlers.push(thread::spawn(move || {
        while main_running.load(Ordering::Relaxed) {
            match rx.recv() {
                Ok(packet) => {
                    let _ = socket.send(packet.serialize().as_slice());
                }
                Err(_) => {
                    break;
                }
            }
        }
    }));

    //
    // READER THREAD 1 — Receive messages from server
    //
    join_handlers.push(thread::spawn(move || {
        while running.load(Ordering::Relaxed) {
            let mut buf = [0u8; 2048];
            match recv_socket.recv(&mut buf) {
                Ok(len) => {
                    if let Ok(packet) = Packet::try_from(&buf[..len]) {
                        let _ = tx.send(packet);
                    }
                }
                Err(_) => {
                    // No data available (non-blocking would use sleep)
                    thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    }));

    Ok(join_handlers)
}
