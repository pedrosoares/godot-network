mod godot_types;
mod runtime;
mod tcp;
mod types;
mod udp;

use godot::prelude::*;

use crate::runtime::Runtime;

struct NetworkExtension;

#[gdextension]
unsafe impl ExtensionLibrary for NetworkExtension {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                godot_print!("Initializing Engine");
            }
            _ => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                Runtime::free();
            }
            _ => (),
        }
    }
}
