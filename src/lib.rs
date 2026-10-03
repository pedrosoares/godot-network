//! GDExtension client for mw-server.
//!
//! - `NetworkClient`: connection, lobby, remote objects, voice.
//! - `NetworkSyncClientNode3d`: replicates a node's transform.
//! - `Player`, `Match`: data passed to signals.

mod client;
mod connection;
mod convert;
mod sync;
mod types;

use godot::prelude::*;

struct NetworkExtension;

#[gdextension]
unsafe impl ExtensionLibrary for NetworkExtension {
    fn on_level_deinit(level: InitLevel) {
        // Network threads must be gone before the library is unloaded.
        if level == InitLevel::Scene {
            connection::shutdown_all();
        }
    }
}
