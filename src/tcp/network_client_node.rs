use std::str::FromStr;
use std::time::Duration;

use crate::godot_types::{Match, Player, raw_packet_to_variant, variant_to_raw_packet};
use crate::tcp::TcpClient;
use crossbeam::channel::Receiver;
use godot::prelude::*;
use network_types::connection::Packet;

#[derive(GodotClass)]
#[class(base=Node)]
pub struct TcpNetworkClient {
    tcp: TcpClient,
    rx: Option<Receiver<Vec<u8>>>,
    base: Base<Node>,
    player: Option<Player>,
    room: Option<Match>,
}

#[godot_api]
impl INode for TcpNetworkClient {
    fn init(base: Base<Node>) -> Self {
        Self {
            base,
            rx: None,
            tcp: TcpClient::new(),
            player: None,
            room: None,
        }
    }

    fn process(&mut self, _delta: f64) {
        let mut counter = 0;
        loop {
            if counter > 40 {
                break;
            }
            counter += 1;
            if let Some(rx) = &mut self.rx {
                match rx.recv_timeout(Duration::from_millis(1)) {
                    Ok(bytes) => {
                        let packet = Packet::from(bytes.as_slice());
                        match packet {
                            Packet::MatchList { matches } => {
                                let mut match_list: Array<Gd<Match>> = Array::new();

                                matches.into_iter().for_each(|(id, name, count)| {
                                    match_list.push(
                                        &Match {
                                            id,
                                            owner_id: -1,
                                            name: GString::from_str(name.as_str()).unwrap(),
                                            number_of_players: count,
                                        }
                                        .to_godot(),
                                    );
                                });

                                self.signals().on_match_list().emit(&match_list);
                            }
                            Packet::MatchCreated {
                                id,
                                owner_id,
                                room_name,
                            } => {
                                let room = Match {
                                    id,
                                    owner_id: owner_id,
                                    name: GString::from_str(room_name.as_str()).unwrap(),
                                    number_of_players: 1,
                                }
                                .to_godot();
                                self.room = Some(Match {
                                    id,
                                    owner_id: owner_id,
                                    name: GString::from_str(room_name.as_str()).unwrap(),
                                    number_of_players: 1,
                                });
                                self.signals().on_match_created().emit(&room);
                            }
                            Packet::MatchJoined {
                                id,
                                user_id,
                                user_name,
                                room_name,
                            } => {
                                godot_print!(
                                    "Player Joined {}, {}, {}, {}",
                                    id,
                                    user_id,
                                    user_name,
                                    room_name,
                                );
                                let player = Player {
                                    id: user_id,
                                    user_name: GString::from_str(user_name.as_str()).unwrap(),
                                }
                                .to_godot();
                                self.signals().on_player_joined().emit(&player);
                                godot_print!(
                                    "Is Player {} {}",
                                    self.player.clone().unwrap().id,
                                    user_id
                                );
                                if self.player.clone().unwrap().id == user_id {
                                    let room = Match {
                                        id,
                                        owner_id: -1,
                                        name: GString::from_str(room_name.as_str()).unwrap(),
                                        number_of_players: 0,
                                    }
                                    .to_godot();
                                    self.room = Some(Match {
                                        id,
                                        owner_id: -1,
                                        name: GString::from_str(room_name.as_str()).unwrap(),
                                        number_of_players: 0,
                                    });
                                    self.signals().on_match_joined().emit(&room);
                                }
                            }
                            Packet::MatchLeaved { user_id, user_name } => {
                                let player = Player {
                                    id: user_id,
                                    user_name: GString::from_str(user_name.as_str()).unwrap(),
                                }
                                .to_godot();
                                self.signals().on_player_leaved().emit(&player);
                            }
                            Packet::MatchDeleted => {
                                self.signals().on_match_deleted().emit();
                                let room = self.room.take().unwrap();
                                self.leave_match(room.id);
                            }
                            Packet::StartMatch { room_id: _, map } => {
                                self.signals().on_match_started().emit(map);
                            }
                            Packet::Spawn { position } => {
                                let (x, y, z) = position;
                                let position = Vector3::new(x, y, z);
                                self.signals().on_spawn().emit(position);
                            }
                            Packet::SpawnRemoteObject {
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
                                    .on_spawn_remote_object()
                                    .emit(id, object_id, vposition, vrotation);
                            }
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
                            Packet::DespawnRemoteObject { id, object_id } => {
                                self.signals()
                                    .on_despawn_remote_object()
                                    .emit(id, object_id);
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
                            // Packet::StartMatch { map } => todo!(),
                            // Packet::Message { id, name, text } => todo!(),
                            val => {
                                godot_print!("PASS: {:?}", val);
                            }
                        }

                        // let message = decode(&bytes[..]).unwrap();
                        // match message {
                        //     Message::String(s) => {
                        //         self.signals().on_message().emit(s.to_string().clone());
                        //         godot_print!("{}.Message from the Server -{:?}- ", self.count, s);
                        //     }
                        //     Message::Int(i) => {
                        //         godot_print!("{}.Int Message from the Server -{:?}- ", self.count, i);
                        //     }
                        //     t => {
                        //         godot_print!("Type not mapped: {:?}", t);
                        //     }
                        // }
                    }
                    _ => {
                        break;
                    }
                }
            }
        }
    }
}

#[godot_api]
impl TcpNetworkClient {
    #[func]
    fn is_on_a_match(&self) -> bool {
        return self.player.is_some() && self.room.is_some();
    }

    #[func]
    fn connect_to_server(&mut self, remote_addr: String, name: String) {
        let result = self.tcp.connect(remote_addr, name);
        match result {
            Ok(packet) => {
                godot_print!("Rust -> Connected {:?}", packet);
                match packet {
                    Packet::Login { id, name } => {
                        self.rx = Some(self.tcp.get_receiver());
                        self.player = Some(Player {
                            id,
                            user_name: GString::from_str(name.clone().as_str()).unwrap(),
                        });
                        self.signals().connected().emit(id);
                    }
                    _ => panic!("First Message Should be a Login, not {:?}", packet),
                };
            }
            Err(err) => {
                self.signals().error().emit(err.to_string());
            }
        }
    }

    pub fn send_message(&mut self, message: Packet) {
        self.tcp.send(message.serialize().as_slice());
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

    #[func]
    pub fn get_player(&self) -> Gd<Player> {
        self.player.clone().unwrap().to_godot()
    }

    #[func]
    fn delete_match(&mut self, room_id: i32) {
        let sfm = Packet::DeleteMatch { room_id };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn create_match(&mut self, room_name: GString) {
        let sfm = Packet::NewMatch {
            room_name: room_name.to_string(),
        };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn join_match(&mut self, room_id: i32) {
        let sfm = Packet::JoinMatch { room_id };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn leave_match(&mut self, room_id: i32) {
        let sfm = Packet::LeaveMatch { room_id };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn start_match(&mut self, room_id: i32, map: String) {
        let sfm = Packet::StartMatch { room_id, map: map };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn spawn_players(&mut self, room_id: i32, spaws: Array<Vector3>) {
        let sfm = Packet::SpawnPlayers {
            room_id,
            positions: spaws.iter_shared().map(|p| (p.x, p.y, p.z)).collect(),
        };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    pub fn spawn_remote_object(&mut self, object_id: i32, position: Vector3, rotation: Vector3) {
        let id = self.player.clone().unwrap().id;
        let sfm = Packet::SpawnRemoteObject {
            id,
            object_id,
            position: (position.x, position.y, position.z),
            rotation: (rotation.x, rotation.y, rotation.z),
        };
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn search_for_matches(&mut self) {
        let sfm = Packet::ListMatches;
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[func]
    fn stop_search_for_matches(&mut self) {
        let sfm = Packet::RemoveFromListMatches;
        self.tcp.send(sfm.serialize().as_slice());
    }

    #[signal]
    fn connected(id: i32);

    #[signal]
    fn disconnected();

    #[signal]
    fn error(error_message: String);

    #[signal]
    fn on_message(message: String);

    #[signal]
    fn on_match_list(macthes: Array<Gd<Match>>);

    #[signal]
    fn on_match_created(match_room: Gd<Match>);

    #[signal]
    fn on_player_joined(player: Gd<Player>);

    #[signal]
    fn on_player_leaved(player: Gd<Player>);

    #[signal]
    fn on_match_joined(match_room: Gd<Match>);

    #[signal]
    fn on_match_deleted();

    #[signal]
    fn on_match_started(map: String);

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
    fn on_remote_call(player_id: i32, object_id: i32, method: String, params: Array<Variant>);
}
