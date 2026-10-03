use godot::prelude::*;

use crate::client::NetworkClient;

/// Replicates a 3D node: announces it on ready, streams its transform at
/// `sync_rate` Hz while it moves (plus a slow keepalive, since UDP may drop
/// the last update), and despawns it when it leaves the tree.
#[derive(GodotClass)]
#[class(base=Node3D)]
pub struct NetworkSyncClientNode3d {
    base: Base<Node3D>,
    /// Must be unique among this player's live objects.
    #[export]
    object_id: i32,
    #[export]
    sync_position: bool,
    #[export]
    sync_rotation: bool,
    #[export]
    network_client_node: Option<Gd<NetworkClient>>,
    #[export]
    sync_position_from: Option<Gd<Node3D>>,
    #[export]
    sync_rotation_from: Option<Gd<Node3D>>,
    #[export]
    sync_position_offset: Vector3,
    /// Updates per second while the object moves.
    #[export]
    sync_rate: f64,
    /// Seconds between updates while the object is still.
    #[export]
    keepalive: f64,
    spawned: bool,
    since_send: f64,
    last_position: Vector3,
    last_rotation: Vector3,
}

#[godot_api]
impl INode3D for NetworkSyncClientNode3d {
    fn init(base: Base<Node3D>) -> Self {
        Self {
            base,
            object_id: 0,
            sync_position: false,
            sync_rotation: false,
            network_client_node: None,
            sync_position_from: None,
            sync_rotation_from: None,
            sync_position_offset: Vector3::ZERO,
            sync_rate: 20.0,
            keepalive: 0.5,
            spawned: false,
            since_send: 0.0,
            last_position: Vector3::ZERO,
            last_rotation: Vector3::ZERO,
        }
    }

    fn ready(&mut self) {
        let (position, rotation) = (self.position(), self.rotation());
        let object_id = self.object_id;
        if let Some(mut client) = self.client() {
            client
                .bind_mut()
                .spawn_remote_object(object_id, position, rotation);
            self.spawned = true;
            self.last_position = position;
            self.last_rotation = rotation;
        }
    }

    fn physics_process(&mut self, delta: f64) {
        if !self.spawned || !(self.sync_position || self.sync_rotation) {
            return;
        }
        self.since_send += delta;
        if self.since_send < 1.0 / self.sync_rate.max(1.0) {
            return;
        }
        let (position, rotation) = (self.position(), self.rotation());
        let moved = position.distance_squared_to(self.last_position) > 1e-6
            || rotation.distance_squared_to(self.last_rotation) > 1e-6;
        if !moved && self.since_send < self.keepalive {
            return;
        }
        self.since_send = 0.0;
        self.last_position = position;
        self.last_rotation = rotation;
        let object_id = self.object_id;
        if let Some(mut client) = self.client() {
            client
                .bind_mut()
                .send_location(object_id, position, rotation);
        }
    }

    fn exit_tree(&mut self) {
        self.despawn();
    }
}

#[godot_api]
impl NetworkSyncClientNode3d {
    /// Removes the object for everyone else (also done automatically when
    /// this node leaves the tree).
    #[func]
    fn despawn(&mut self) {
        if !self.spawned {
            return;
        }
        self.spawned = false;
        let object_id = self.object_id;
        if let Some(mut client) = self.client() {
            client.bind_mut().despawn_remote_object(object_id);
        }
    }

    /// Calls `method` on every remote copy of this object.
    #[func]
    fn remote_object_call(&mut self, method: GString, params: VarArray) {
        let object_id = self.object_id;
        if let Some(mut client) = self.client() {
            let id = client.bind().player_id();
            client
                .bind_mut()
                .remote_object_call(id, object_id, method, params, true);
        }
    }
}

impl NetworkSyncClientNode3d {
    /// The client, if it is still alive and in a match.
    fn client(&self) -> Option<Gd<NetworkClient>> {
        let client = self.network_client_node.clone()?;
        (client.is_instance_valid() && client.bind().is_on_a_match()).then_some(client)
    }

    fn position(&self) -> Vector3 {
        if !self.sync_position {
            return Vector3::ZERO;
        }
        let position = match &self.sync_position_from {
            Some(node) => node.get_global_position(),
            None => self.base().get_global_position(),
        };
        position - self.sync_position_offset
    }

    fn rotation(&self) -> Vector3 {
        if !self.sync_rotation {
            return Vector3::ZERO;
        }
        match &self.sync_rotation_from {
            Some(node) => node.get_global_rotation(),
            None => self.base().get_global_rotation(),
        }
    }
}
