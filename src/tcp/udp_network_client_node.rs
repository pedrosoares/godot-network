use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    godot_types::{raw_packet_to_variant, variant_to_raw_packet},
    udp,
};

use crossbeam::channel::{Receiver, Sender, unbounded};
use godot::prelude::*;
use network_types::connection::Packet;

struct UdpChannel {
    tx: Sender<Packet>,
    rx: Receiver<Packet>,
}

impl UdpChannel {
    fn new() -> Self {
        let (tx, rx): (Sender<Packet>, Receiver<Packet>) = unbounded();
        Self { tx, rx }
    }
}

#[derive(GodotClass)]
#[class(base=Node)]
pub struct UdpNetworkClientNode {
    base: Base<Node>,
    connected: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    writer: UdpChannel,
    reader: UdpChannel,
}

#[godot_api]
impl INode for UdpNetworkClientNode {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            connected: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
            threads: Vec::new(),
            writer: UdpChannel::new(),
            reader: UdpChannel::new(),
        }
    }

    fn process(&mut self, _delta: f64) {
        let mut counter = 0;
        loop {
            if counter > 40 {
                break;
            }
            counter += 1;

            match self.reader.rx.recv_timeout(Duration::from_millis(1)) {
                Ok(packet) => {
                    match packet {
                        Packet::RemoteObjectLocation {
                            id,
                            object_id,
                            position,
                            rotation,
                        } => {
                            let (px, py, pz) = position;
                            let vposition = Vector3::new(px, py, pz);
                            let (rx, ry, rz) = rotation;
                            let vrotation = Vector3::new(rx, ry, rz);

                            self.signals()
                                .on_remote_object_location()
                                .emit(id, object_id, vposition, vrotation);
                        }
                        Packet::RemoteObjectCall {
                            id,
                            object_id,
                            method,
                            params,
                            broadcast: _,
                        } => {
                            let mut g_params: Array<Variant> = Array::new();

                            params.iter().for_each(|param| {
                                let value = raw_packet_to_variant(param);
                                g_params.push(&value);
                            });

                            self.signals()
                                .on_remote_call()
                                .emit(id, object_id, method, &g_params);
                        }
                        val => {
                            godot_print!("UDP PASS: {:?}", val);
                        }
                    };
                }
                _ => {
                    break;
                }
            }
        }
    }
}

#[godot_api]
impl UdpNetworkClientNode {
    #[func]
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    #[func]
    pub fn connect_to_server(&mut self, server_addr: String, room_id: i32) {
        if let Ok(threads) = udp::client(
            server_addr,
            room_id,
            self.reader.tx.clone(),
            self.writer.rx.clone(),
            self.running.clone(),
        ) {
            self.threads = threads;
            self.connected.store(true, Ordering::Relaxed);
        } else {
            godot_print!("UDP Not connected");
        }
    }

    #[func]
    pub fn disconnect(&mut self) {
        // Tell UDP server that is disconnecting
        // TODO Handle it better
        let _ = self.writer.tx.send(Packet::Disconnect);
        // This will prevent the thread loop again
        self.running.store(false, Ordering::Relaxed);
        // This will end the close the crossbeam::channel forcing the thread closes
        self.writer = UdpChannel::new();
        self.reader = UdpChannel::new();

        // Clear threads and wait it to finish
        for t in self.threads.drain(..) {
            let _ = t.join();
        }

        // Store this Node as disconnected
        self.connected.store(false, Ordering::Relaxed);
    }

    #[func]
    pub fn remote_object_call(
        &mut self,
        id: i32,
        object_id: i32,
        method: GString,
        params: Array<Variant>,
        broadcast: bool,
    ) {
        let r_params = params
            .iter_shared()
            .map(|value| variant_to_raw_packet(value))
            .collect();

        self.send_message(Packet::RemoteObjectCall {
            id: id,
            object_id: object_id,
            method: method.to_string(),
            params: r_params,
            broadcast,
        });
    }

    #[signal]
    fn on_remote_object_location(
        player_id: i32,
        object_id: i32,
        position: Vector3,
        rotation: Vector3,
    );

    #[signal]
    fn on_remote_call(player_id: i32, object_id: i32, method: String, params: Array<Variant>);
}

impl UdpNetworkClientNode {
    pub fn send_message(&mut self, packet: Packet) -> Option<()> {
        if self.connected.load(Ordering::Relaxed) {
            self.writer.tx.send(packet).ok()
        } else {
            godot_print!("UDP is not connected");
            None
        }
    }
}
