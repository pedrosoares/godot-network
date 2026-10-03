use godot::prelude::*;
use mw_protocol::Packet;

use crate::connection::{Connection, Event};
use crate::convert::{raw_packet_to_variant, variant_to_raw_packet};
use crate::types::{Match, Player};

/// Client for mw-server: lobby and reliable messages over TCP, locations and
/// voice over UDP. All networking runs on background threads; events are
/// turned into signals in `process`, so nothing here blocks a frame.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct NetworkClient {
    base: Base<Node>,
    connection: Option<Connection>,
    player: Option<(i32, String)>,
    room: Option<RoomState>,
    /// `MatchJoined` packets received while joining, before our own one.
    joining: Vec<(i32, String)>,
}

#[derive(Clone)]
struct RoomState {
    id: i32,
    owner_id: i32,
    name: String,
}

fn vec3((x, y, z): (f32, f32, f32)) -> Vector3 {
    Vector3::new(x, y, z)
}

fn tuple(v: Vector3) -> (f32, f32, f32) {
    (v.x, v.y, v.z)
}

#[godot_api]
impl INode for NetworkClient {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            connection: None,
            player: None,
            room: None,
            joining: Vec::new(),
        }
    }

    fn process(&mut self, _delta: f64) {
        // Drain everything that arrived since the last frame.
        loop {
            let Some(event) = self
                .connection
                .as_ref()
                .and_then(|c| c.events.try_recv().ok())
            else {
                break;
            };
            self.handle(event);
        }
    }

    fn exit_tree(&mut self) {
        self.close();
    }
}

#[godot_api]
impl NetworkClient {
    #[signal]
    fn connected(id: i32);
    #[signal]
    fn disconnected();
    /// Connection-level failure (could not connect, connection lost).
    #[signal]
    fn error(message: GString);
    /// The server refused a request; `code` is an `ErrorCode` index.
    #[signal]
    fn server_error(code: i32, message: GString);
    #[signal]
    fn udp_ready();
    #[signal]
    fn on_match_list(matches: Array<Gd<Match>>);
    #[signal]
    fn on_match_created(match_room: Gd<Match>);
    #[signal]
    fn on_match_joined(match_room: Gd<Match>);
    #[signal]
    fn on_player_joined(player: Gd<Player>);
    #[signal]
    fn on_player_leaved(player: Gd<Player>);
    #[signal]
    fn on_match_deleted();
    /// The room's owner left and `owner_id` owns it now (servers running
    /// with `--host-migration`). `get_match()` already reports the new owner.
    #[signal]
    fn on_owner_changed(owner_id: i32);
    #[signal]
    fn on_match_started(map: GString);
    #[signal]
    fn on_spawn(position: Vector3);
    #[signal]
    fn on_spawn_remote_object(player_id: i32, object_id: i32, position: Vector3, rotation: Vector3);
    #[signal]
    fn on_despawn_remote_object(player_id: i32, object_id: i32);
    #[signal]
    fn on_remote_object_location(
        player_id: i32,
        object_id: i32,
        position: Vector3,
        rotation: Vector3,
    );
    #[signal]
    fn on_remote_call(player_id: i32, object_id: i32, method: GString, params: VarArray);
    #[signal]
    fn on_voice(player_id: i32, frame: PackedByteArray);
    /// Datagram on a game channel (16..=255), sender vouched for by the server.
    #[signal]
    fn on_datagram(player_id: i32, channel: i32, payload: PackedByteArray);
    #[signal]
    fn on_chat(player_id: i32, player_name: GString, text: GString);
    #[signal]
    fn on_game(kind: i32, payload: PackedByteArray);

    /// Connects and logs in; emits `connected` or `error`. Never blocks.
    #[func]
    fn connect_to_server(&mut self, host: GString, tcp_port: i32, udp_port: i32, name: GString) {
        self.close();
        let (Ok(tcp_port), Ok(udp_port)) = (u16::try_from(tcp_port), u16::try_from(udp_port))
        else {
            self.signals().error().emit(&GString::from("invalid port"));
            return;
        };
        self.connection = Some(Connection::open(
            host.to_string(),
            tcp_port,
            udp_port,
            name.to_string(),
        ));
    }

    #[func]
    fn disconnect_from_server(&mut self) {
        if self.connection.is_some() {
            self.close();
            self.signals().disconnected().emit();
        }
    }

    /// Logged in.
    #[func]
    fn is_connected_to_server(&self) -> bool {
        self.connection.is_some() && self.player.is_some()
    }

    #[func]
    fn is_udp_ready(&self) -> bool {
        self.connection.as_ref().is_some_and(Connection::udp_ready)
    }

    #[func]
    pub fn is_on_a_match(&self) -> bool {
        self.is_connected_to_server() && self.room.is_some()
    }

    /// The logged-in player; id -1 before login.
    #[func]
    pub fn get_player(&self) -> Gd<Player> {
        match &self.player {
            Some((id, name)) => Player::new(*id, name),
            None => Player::new(-1, ""),
        }
    }

    #[func]
    fn get_room_id(&self) -> i32 {
        self.room.as_ref().map_or(-1, |room| room.id)
    }

    /// The current match, or null.
    #[func]
    fn get_match(&self) -> Option<Gd<Match>> {
        self.room
            .as_ref()
            .map(|room| Match::new(room.id, room.owner_id, &room.name, -1))
    }

    // ------------------------------------------------------------- lobby

    #[func]
    fn search_for_matches(&mut self) {
        self.send(&Packet::ListMatches);
    }

    #[func]
    fn stop_search_for_matches(&mut self) {
        self.send(&Packet::RemoveFromListMatches);
    }

    #[func]
    fn create_match(&mut self, room_name: GString) {
        self.send(&Packet::NewMatch {
            room_name: room_name.to_string(),
        });
    }

    #[func]
    fn join_match(&mut self, room_id: i32) {
        self.joining.clear();
        self.send(&Packet::JoinMatch { room_id });
    }

    /// Leaves the current match (deletes it when we own it).
    #[func]
    fn leave_match(&mut self) {
        if let Some(room) = self.room.take() {
            self.send(&Packet::LeaveMatch { room_id: room.id });
        }
    }

    /// Deletes the current match; only its owner may.
    #[func]
    fn delete_match(&mut self) {
        if let Some(room) = self.room.take() {
            self.send(&Packet::DeleteMatch { room_id: room.id });
        }
    }

    #[func]
    fn start_match(&mut self, map: GString) {
        if let Some(room_id) = self.room.as_ref().map(|room| room.id) {
            self.send(&Packet::StartMatch {
                room_id,
                map: map.to_string(),
            });
        }
    }

    #[func]
    fn spawn_players(&mut self, positions: Array<Vector3>) {
        if let Some(room_id) = self.room.as_ref().map(|room| room.id) {
            self.send(&Packet::SpawnPlayers {
                room_id,
                positions: positions.iter_shared().map(tuple).collect(),
            });
        }
    }

    #[func]
    fn send_chat(&mut self, text: GString) {
        let (id, name) = self.player.clone().unwrap_or((-1, String::new()));
        // The server overwrites id and name with the real sender's.
        self.send(&Packet::Message {
            id,
            name,
            text: text.to_string(),
        });
    }

    #[func]
    fn send_game(&mut self, kind: i32, payload: PackedByteArray) {
        self.send(&Packet::Game {
            kind: kind as u16,
            payload: payload.to_vec(),
        });
    }

    // --------------------------------------------------- remote objects

    #[func]
    pub fn spawn_remote_object(&mut self, object_id: i32, position: Vector3, rotation: Vector3) {
        let id = self.player_id();
        self.send(&Packet::SpawnRemoteObject {
            id,
            object_id,
            position: tuple(position),
            rotation: tuple(rotation),
        });
    }

    #[func]
    pub fn despawn_remote_object(&mut self, object_id: i32) {
        let id = self.player_id();
        self.send(&Packet::DespawnRemoteObject { id, object_id });
    }

    /// Unreliable, newest-wins update over UDP; reliable TCP until UDP is up.
    #[func]
    pub fn send_location(&mut self, object_id: i32, position: Vector3, rotation: Vector3) {
        let packet = Packet::RemoteObjectLocation {
            id: self.player_id(),
            object_id,
            position: tuple(position),
            rotation: tuple(rotation),
        };
        let Some(connection) = self.connection.as_mut() else {
            return;
        };
        if !connection.send_state(&packet) {
            connection.send(&packet);
        }
    }

    /// Calls `method` on object (`player_id`, `object_id`). With `broadcast`
    /// it must be one of our objects and reaches everyone else in the match;
    /// otherwise it is delivered only to `player_id`.
    #[func]
    pub fn remote_object_call(
        &mut self,
        player_id: i32,
        object_id: i32,
        method: GString,
        params: VarArray,
        broadcast: bool,
    ) {
        self.send(&Packet::RemoteObjectCall {
            id: player_id,
            object_id,
            method: method.to_string(),
            params: params
                .iter_shared()
                .map(|v| variant_to_raw_packet(&v))
                .collect(),
            broadcast,
        });
    }

    /// Sends an unreliable datagram on a game channel (16..=255).
    #[func]
    fn send_datagram(&mut self, channel: i32, payload: PackedByteArray) {
        if let (Some(connection), Ok(channel)) = (&self.connection, u8::try_from(channel))
            && channel >= mw_protocol::udp::CHANNEL_GAME
        {
            connection.send_udp(channel, payload.as_slice());
        }
    }

    /// Sends one encoded voice frame (e.g. Opus) to the match over UDP.
    #[func]
    fn send_voice(&mut self, frame: PackedByteArray) {
        if let Some(connection) = &self.connection {
            connection.send_voice(frame.as_slice());
        }
    }
}

impl NetworkClient {
    pub fn player_id(&self) -> i32 {
        self.player.as_ref().map_or(-1, |(id, _)| *id)
    }

    fn send(&self, packet: &Packet) {
        if let Some(connection) = &self.connection {
            connection.send(packet);
        }
    }

    fn close(&mut self) {
        if let Some(mut connection) = self.connection.take() {
            connection.close();
        }
        self.player = None;
        self.room = None;
        self.joining.clear();
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Packet(packet) => self.handle_packet(packet),
            Event::Closed(reason) => {
                let was_logged_in = self.player.is_some();
                self.close();
                if was_logged_in {
                    self.signals().disconnected().emit();
                } else {
                    self.signals().error().emit(&GString::from(reason.as_str()));
                }
            }
            Event::UdpReady(socket) => {
                if let Some(connection) = self.connection.as_mut() {
                    connection.set_udp(socket);
                }
                self.signals().udp_ready().emit();
            }
            Event::UdpFailed(reason) => {
                godot_warn!("godot_network: {reason}; locations fall back to TCP, no voice");
            }
            Event::Location {
                sender,
                object_id,
                position,
                rotation,
            } => {
                self.signals().on_remote_object_location().emit(
                    sender,
                    object_id,
                    vec3(position),
                    vec3(rotation),
                );
            }
            Event::Voice { sender, frame } => {
                self.signals()
                    .on_voice()
                    .emit(sender, &PackedByteArray::from(frame.as_slice()));
            }
            Event::Datagram {
                sender,
                channel,
                payload,
            } => {
                self.signals().on_datagram().emit(
                    sender,
                    i32::from(channel),
                    &PackedByteArray::from(payload.as_slice()),
                );
            }
        }
    }

    fn handle_packet(&mut self, packet: Packet) {
        match packet {
            Packet::Welcome { udp_token, .. } => {
                if let Some(connection) = &self.connection {
                    connection.start_udp(udp_token);
                }
            }
            Packet::Login { id, name } => {
                self.player = Some((id, name));
                self.signals().connected().emit(id);
            }
            Packet::Error { code, message } => {
                self.signals()
                    .server_error()
                    .emit(code as i32, &GString::from(message.as_str()));
            }
            Packet::Disconnect => {
                self.close();
                self.signals().disconnected().emit();
            }
            Packet::MatchList { matches } => {
                let list: Array<Gd<Match>> = matches
                    .iter()
                    .map(|(id, name, players)| Match::new(*id, -1, name, *players))
                    .collect();
                self.signals().on_match_list().emit(&list);
            }
            Packet::MatchCreated {
                id,
                owner_id,
                room_name,
            } => {
                self.room = Some(RoomState {
                    id,
                    owner_id,
                    name: room_name.clone(),
                });
                self.signals()
                    .on_match_created()
                    .emit(&Match::new(id, owner_id, &room_name, 1));
            }
            Packet::MatchJoined {
                id,
                user_id,
                user_name,
                room_name,
            } => {
                self.signals()
                    .on_player_joined()
                    .emit(&Player::new(user_id, &user_name));
                if self.room.is_none() {
                    // The joiner gets one MatchJoined per member in join
                    // order: the owner first, itself last.
                    self.joining.push((user_id, user_name));
                    if user_id == self.player_id() {
                        let owner_id = self.joining.first().map_or(-1, |(id, _)| *id);
                        let players = self.joining.len() as i32;
                        self.joining.clear();
                        self.room = Some(RoomState {
                            id,
                            owner_id,
                            name: room_name.clone(),
                        });
                        self.signals()
                            .on_match_joined()
                            .emit(&Match::new(id, owner_id, &room_name, players));
                    }
                }
            }
            Packet::MatchLeaved { user_id, user_name } => {
                self.signals()
                    .on_player_leaved()
                    .emit(&Player::new(user_id, &user_name));
            }
            Packet::MatchDeleted => {
                self.room = None;
                self.signals().on_match_deleted().emit();
            }
            Packet::StartMatch { map, .. } => {
                self.signals()
                    .on_match_started()
                    .emit(&GString::from(map.as_str()));
            }
            Packet::Spawn { position } => {
                self.signals().on_spawn().emit(vec3(position));
            }
            Packet::SpawnRemoteObject {
                id,
                object_id,
                position,
                rotation,
            } => {
                self.signals().on_spawn_remote_object().emit(
                    id,
                    object_id,
                    vec3(position),
                    vec3(rotation),
                );
            }
            Packet::DespawnRemoteObject { id, object_id } => {
                self.signals()
                    .on_despawn_remote_object()
                    .emit(id, object_id);
            }
            Packet::RemoteObjectLocation {
                id,
                object_id,
                position,
                rotation,
            } => {
                self.signals().on_remote_object_location().emit(
                    id,
                    object_id,
                    vec3(position),
                    vec3(rotation),
                );
            }
            Packet::RemoteObjectCall {
                id,
                object_id,
                method,
                params,
                ..
            } => {
                let params: VarArray = params.iter().map(raw_packet_to_variant).collect();
                self.signals().on_remote_call().emit(
                    id,
                    object_id,
                    &GString::from(method.as_str()),
                    &params,
                );
            }
            Packet::Message { id, name, text } => {
                self.signals().on_chat().emit(
                    id,
                    &GString::from(name.as_str()),
                    &GString::from(text.as_str()),
                );
            }
            Packet::Game { kind, payload } => {
                self.signals()
                    .on_game()
                    .emit(i32::from(kind), &PackedByteArray::from(payload.as_slice()));
            }
            Packet::OwnerChanged { room_id, owner_id } => {
                if let Some(room) = self.room.as_mut().filter(|room| room.id == room_id) {
                    room.owner_id = owner_id;
                    self.signals().on_owner_changed().emit(owner_id);
                }
            }
            // Client-to-server packets and pings.
            _ => {}
        }
    }
}
