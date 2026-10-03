//! Networking threads. Nothing here touches Godot: the threads talk to the
//! `NetworkClient` node through [`Event`]s, which it drains every frame, so the
//! game's main thread never blocks on the network.
//!
//! Every thread is registered so [`shutdown_all`] can stop and join them
//! before the extension library is unloaded; a thread still running then
//! would crash the engine on exit.

use std::io::{self, BufReader, BufWriter, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use mw_protocol::udp::{self, LatestWins, Relayed};
use mw_protocol::{PROTOCOL_VERSION, Packet, Vec3, read_frame};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FRAME_LEN: usize = 1024 * 1024;
const UDP_JOIN_INTERVAL: Duration = Duration::from_millis(250);
const UDP_JOIN_TRIES: u32 = 40;
/// How often idle threads wake up to check whether they should stop.
const POLL: Duration = Duration::from_millis(100);

/// State shared by the threads of one connection.
struct Shared {
    running: AtomicBool,
    tcp: Mutex<Option<TcpStream>>,
}

impl Shared {
    fn running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Says goodbye and makes every thread of the connection return.
    fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        let stream = self
            .tcp
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(mut stream) = stream {
            let _ = stream.write_all(&Packet::Disconnect.encode_frame());
            // Unblocks the reader thread.
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}

struct Registry {
    threads: Vec<JoinHandle<()>>,
    connections: Vec<Weak<Shared>>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    threads: Vec::new(),
    connections: Vec::new(),
});

fn registry() -> std::sync::MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(PoisonError::into_inner)
}

fn spawn(name: &str, f: impl FnOnce() + Send + 'static) {
    let handle = thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .expect("spawn network thread");
    let mut registry = registry();
    registry.threads.retain(|thread| !thread.is_finished());
    registry.threads.push(handle);
}

/// Stops every connection and waits for all network threads to return.
pub fn shutdown_all() {
    let (threads, connections) = {
        let mut registry = registry();
        (
            std::mem::take(&mut registry.threads),
            std::mem::take(&mut registry.connections),
        )
    };
    for shared in connections.iter().filter_map(Weak::upgrade) {
        shared.stop();
    }
    for thread in threads {
        let _ = thread.join();
    }
}

pub enum Event {
    Packet(Packet),
    /// The TCP connection failed or closed.
    Closed(String),
    /// The UDP join was acknowledged; the socket is used for sending.
    UdpReady(UdpSocket),
    UdpFailed(String),
    Location {
        sender: i32,
        object_id: i32,
        position: Vec3,
        rotation: Vec3,
    },
    Voice {
        sender: i32,
        frame: Vec<u8>,
    },
    Datagram {
        sender: i32,
        channel: u8,
        payload: Vec<u8>,
    },
}

/// One connection attempt. Dropping it does not stop the threads; call
/// [`Connection::close`].
pub struct Connection {
    pub events: Receiver<Event>,
    events_tx: Sender<Event>,
    outbound: Sender<Vec<u8>>,
    shared: Arc<Shared>,
    host: String,
    udp_port: u16,
    udp: Option<UdpSocket>,
    udp_sequence: u32,
}

impl Connection {
    /// Starts connecting in the background; the hello and login are queued
    /// and sent as soon as the socket is up.
    pub fn open(host: String, tcp_port: u16, udp_port: u16, name: String) -> Self {
        let (events_tx, events) = unbounded();
        let (outbound, outbound_rx) = unbounded::<Vec<u8>>();
        let shared = Arc::new(Shared {
            running: AtomicBool::new(true),
            tcp: Mutex::new(None),
        });
        registry().connections.push(Arc::downgrade(&shared));

        let _ = outbound.send(
            Packet::Hello {
                protocol_version: PROTOCOL_VERSION,
            }
            .encode_frame(),
        );
        let _ = outbound.send(Packet::LoginRequest { name }.encode_frame());

        let tx = events_tx.clone();
        let tcp_host = host.clone();
        let tcp_shared = shared.clone();
        spawn("net-tcp", move || {
            tcp_thread(tcp_host, tcp_port, tx, outbound_rx, tcp_shared);
        });

        Self {
            events,
            events_tx,
            outbound,
            shared,
            host,
            udp_port,
            udp: None,
            udp_sequence: 0,
        }
    }

    pub fn set_udp(&mut self, socket: UdpSocket) {
        self.udp = Some(socket);
    }

    pub fn udp_ready(&self) -> bool {
        self.udp.is_some()
    }

    /// Starts the UDP join with the token from `Welcome`.
    pub fn start_udp(&self, token: u64) {
        let tx = self.events_tx.clone();
        let host = self.host.clone();
        let port = self.udp_port;
        let shared = self.shared.clone();
        spawn("net-udp", move || udp_thread(host, port, token, tx, shared));
    }

    /// Queues a packet for the TCP writer thread (never blocks).
    pub fn send(&self, packet: &Packet) {
        let _ = self.outbound.send(packet.encode_frame());
    }

    /// Unreliable, newest-wins location update.
    pub fn send_state(&mut self, packet: &Packet) -> bool {
        let Some(socket) = &self.udp else {
            return false;
        };
        self.udp_sequence = self.udp_sequence.wrapping_add(1);
        socket
            .send(&udp::encode_state(self.udp_sequence, packet))
            .is_ok()
    }

    pub fn send_voice(&self, frame: &[u8]) {
        self.send_udp(udp::CHANNEL_VOICE, frame);
    }

    pub fn send_udp(&self, channel: u8, payload: &[u8]) {
        if let Some(socket) = &self.udp {
            let mut datagram = Vec::with_capacity(1 + payload.len());
            datagram.push(channel);
            datagram.extend_from_slice(payload);
            let _ = socket.send(&datagram);
        }
    }

    /// Says goodbye and stops every thread of this connection.
    pub fn close(&mut self) {
        self.shared.stop();
        self.udp = None;
    }
}

fn resolve(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    Ok((host, port).to_socket_addrs()?.collect())
}

fn connect(host: &str, port: u16) -> io::Result<TcpStream> {
    let mut last = io::Error::new(io::ErrorKind::NotFound, "host has no addresses");
    for addr in resolve(host, port)? {
        match TcpStream::connect_timeout(&addr, CONNECT_TIMEOUT) {
            Ok(stream) => return Ok(stream),
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn tcp_thread(
    host: String,
    port: u16,
    events: Sender<Event>,
    outbound: Receiver<Vec<u8>>,
    shared: Arc<Shared>,
) {
    let stream = match connect(&host, port) {
        Ok(stream) => stream,
        Err(err) => {
            let _ = events.send(Event::Closed(format!(
                "could not connect to {host}:{port}: {err}"
            )));
            return;
        }
    };
    let _ = stream.set_nodelay(true);
    let clones = (stream.try_clone(), stream.try_clone());
    let (Ok(writer), Ok(handle)) = clones else {
        let _ = events.send(Event::Closed("could not clone socket".into()));
        return;
    };
    {
        let mut tcp = shared.tcp.lock().unwrap_or_else(PoisonError::into_inner);
        if !shared.running() {
            // Closed while we were connecting.
            return;
        }
        *tcp = Some(handle);
    }

    let writer_shared = shared.clone();
    spawn("net-tcp-writer", move || {
        writer_thread(writer, outbound, writer_shared);
    });

    let mut reader = BufReader::with_capacity(16 * 1024, stream);
    let mut body = Vec::new();
    let reason = loop {
        if let Err(err) = read_frame(&mut reader, MAX_FRAME_LEN, &mut body) {
            break if shared.running() {
                format!("connection lost: {err}")
            } else {
                "disconnected".into()
            };
        }
        match Packet::decode(&body) {
            Ok(packet) => {
                if events.send(Event::Packet(packet)).is_err() {
                    return;
                }
            }
            Err(err) => break format!("server sent an undecodable packet: {err}"),
        }
    };
    let _ = events.send(Event::Closed(reason));
}

/// Writes queued frames, batching everything already queued into one write.
fn writer_thread(stream: TcpStream, outbound: Receiver<Vec<u8>>, shared: Arc<Shared>) {
    let mut writer = BufWriter::with_capacity(16 * 1024, stream);
    while shared.running() {
        let frame = match outbound.recv_timeout(POLL) {
            Ok(frame) => frame,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        };
        let batch = std::iter::once(frame).chain(outbound.try_iter());
        for frame in batch {
            if writer.write_all(&frame).is_err() {
                return;
            }
        }
        if writer.flush().is_err() {
            return;
        }
    }
}

fn udp_thread(host: String, port: u16, token: u64, events: Sender<Event>, shared: Arc<Shared>) {
    let socket = match open_udp(&host, port) {
        Ok(socket) => socket,
        Err(err) => {
            let _ = events.send(Event::UdpFailed(format!("udp: {err}")));
            return;
        }
    };
    let join = Packet::UdpJoin { token }.encode();
    let mut ready = false;
    let mut tries = 0;
    let mut next_join = Instant::now();
    let mut latest = LatestWins::default();
    let mut buf = vec![0u8; 64 * 1024];

    while shared.running() {
        if !ready && Instant::now() >= next_join {
            if tries == UDP_JOIN_TRIES {
                let _ = events.send(Event::UdpFailed("udp join was never acknowledged".into()));
                return;
            }
            tries += 1;
            next_join = Instant::now() + UDP_JOIN_INTERVAL;
            let _ = socket.send(&join);
        }
        let len = match socket.recv(&mut buf) {
            Ok(len) => len,
            // Timeouts, and ICMP errors while the server is unreachable.
            Err(_) => continue,
        };
        let datagram = &buf[..len];
        if udp::is_join_ack(datagram) {
            if !ready {
                ready = true;
                match socket.try_clone() {
                    Ok(sender) => {
                        let _ = events.send(Event::UdpReady(sender));
                    }
                    Err(err) => {
                        let _ = events.send(Event::UdpFailed(format!("udp: {err}")));
                        return;
                    }
                }
            }
            continue;
        }
        let Some(relayed) = Relayed::parse(datagram) else {
            continue;
        };
        let event = match relayed.channel {
            udp::CHANNEL_STATE => match udp::decode_state(relayed.payload) {
                Some((
                    sequence,
                    Packet::RemoteObjectLocation {
                        object_id,
                        position,
                        rotation,
                        ..
                    },
                )) if latest.accept(relayed.sender, object_id, sequence) => Event::Location {
                    // The sender id comes from the server, not from the packet.
                    sender: relayed.sender,
                    object_id,
                    position,
                    rotation,
                },
                _ => continue,
            },
            udp::CHANNEL_VOICE => Event::Voice {
                sender: relayed.sender,
                frame: relayed.payload.to_vec(),
            },
            channel => Event::Datagram {
                sender: relayed.sender,
                channel,
                payload: relayed.payload.to_vec(),
            },
        };
        if events.send(event).is_err() {
            return;
        }
    }
}

fn open_udp(host: &str, port: u16) -> io::Result<UdpSocket> {
    let server = resolve(host, port)?
        .into_iter()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "host has no addresses"))?;
    let local: SocketAddr = if server.is_ipv4() {
        "0.0.0.0:0".parse().expect("valid address")
    } else {
        "[::]:0".parse().expect("valid address")
    };
    let socket = UdpSocket::bind(local)?;
    socket.connect(server)?;
    socket.set_read_timeout(Some(POLL))?;
    Ok(socket)
}
