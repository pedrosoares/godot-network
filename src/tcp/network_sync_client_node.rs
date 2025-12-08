use godot::prelude::*;
use network_types::connection::Packet;

use crate::tcp::{TcpNetworkClient, UdpNetworkClientNode};

#[derive(GodotClass)]
#[class(base=Node3D)]
struct NetworkSyncClientNode3d {
    #[var]
    id: i32,
    #[export]
    object_id: i32,
    #[export]
    sync_using_udp_protocol: bool,
    #[export]
    sync_position: bool,
    #[export]
    sync_rotation: bool,
    base: Base<Node3D>,
    #[export]
    network_client_node: Option<Gd<TcpNetworkClient>>,
    #[export]
    udp_network_client_node: Option<Gd<UdpNetworkClientNode>>,
    #[export]
    sync_position_from: Option<Gd<Node3D>>,
    #[export]
    sync_rotation_from: Option<Gd<Node3D>>,
    #[export]
    sync_position_offset: Vector3,
}

#[godot_api]
impl INode3D for NetworkSyncClientNode3d {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            id: 0,
            object_id: 0,
            sync_using_udp_protocol: false,
            sync_position: false,
            sync_rotation: false,
            base,
            network_client_node: None,
            udp_network_client_node: None,
            sync_position_from: None,
            sync_rotation_from: None,
            sync_position_offset: Vector3::ZERO,
        }
    }

    fn ready(&mut self) {
        let position = self.get_position();
        let rotation = self.get_rotation();
        if let Some(ncn) = self.network_client_node.as_mut() {
            self.id = ncn.bind().get_player().bind().id;
            ncn.bind_mut()
                .spawn_remote_object(self.object_id, position, rotation);
        }
    }

    fn process(&mut self, _delta: f64) {
        let position = self.get_position();
        let rotation = self.get_rotation();
        if let Some(ncn) = self.network_client_node.as_mut() {
            if self.sync_position || self.sync_rotation {
                if self.sync_using_udp_protocol {
                    if let Some(ncn) = self.udp_network_client_node.as_mut() {
                        ncn.bind_mut().send_message(Packet::RemoteObjectLocation {
                            id: self.id,
                            object_id: self.object_id,
                            position: (
                                position.x - self.sync_position_offset.x,
                                position.y - self.sync_position_offset.y,
                                position.z - self.sync_position_offset.z,
                            ),
                            rotation: (rotation.x, rotation.y, rotation.z),
                        });
                    }
                } else {
                    ncn.bind_mut().send_message(Packet::RemoteObjectLocation {
                        id: self.id,
                        object_id: self.object_id,
                        position: (
                            position.x - self.sync_position_offset.x,
                            position.y - self.sync_position_offset.y,
                            position.z - self.sync_position_offset.z,
                        ),
                        rotation: (rotation.x, rotation.y, rotation.z),
                    });
                }
            }
        }
    }
}

#[godot_api]
impl NetworkSyncClientNode3d {
    #[func]
    fn remote_object_call(&mut self, method: GString, params: Array<Variant>, broadcast: bool) {
        if self.sync_using_udp_protocol {
            if let Some(ncn) = self.udp_network_client_node.as_mut() {
                ncn.bind_mut().remote_object_call(
                    self.id,
                    self.object_id,
                    method,
                    params,
                    broadcast,
                );
            }
        } else {
            if let Some(ncn) = self.network_client_node.as_mut() {
                ncn.bind_mut().remote_object_call(
                    self.id,
                    self.object_id,
                    method,
                    params,
                    broadcast,
                );
            }
        }
    }

    #[func]
    fn despawn(&mut self) {
        godot_print!("node:despawn");
        if let Some(ncn) = self.network_client_node.as_mut() {
            godot_print!(
                "Packet::DespawnRemoteObject: id: {}, object_id: {}",
                self.id,
                self.object_id,
            );
            ncn.bind_mut().send_message(Packet::DespawnRemoteObject {
                id: self.id,
                object_id: self.object_id,
            });
        } else {
            godot_error!("No TcpNetworkClientNode set");
        }
    }
}

impl NetworkSyncClientNode3d {
    fn get_position(&self) -> Vector3 {
        if self.sync_position {
            if let Some(spf) = &self.sync_position_from {
                spf.get_position()
            } else {
                self.base().get_position()
            }
        } else {
            Vector3::ZERO
        }
    }

    fn get_rotation(&self) -> Vector3 {
        if self.sync_rotation {
            if let Some(spf) = &self.sync_position_from {
                spf.get_rotation()
            } else {
                self.base().get_rotation()
            }
        } else {
            Vector3::ZERO
        }
    }
}
