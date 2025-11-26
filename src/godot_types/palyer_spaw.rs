use godot::prelude::*;

use crate::types::buffer_codec::BufferCodec;

#[derive(Debug, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct PlayerSpaw {
    #[var]
    pub id: i32,
    #[var]
    pub position: Vector3,
    #[var]
    pub rotation: Vector3,
}

#[godot_api]
impl PlayerSpaw {
    pub fn new() -> Self {
        Self {
            id: 0,
            position: Vector3::ZERO,
            rotation: Vector3::ZERO,
        }
    }

    #[func]
    pub fn init() -> Gd<Self> {
        Gd::from_object(Self::new())
    }
}

impl BufferCodec for PlayerSpaw {
    const TYPE_ID: u8 = 53;

    fn to_bytes(&self) -> Vec<u8> {
        [
            self.id.to_bytes(),
            self.position.to_bytes(),
            self.rotation.to_bytes(),
        ]
        .concat()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(PlayerSpaw {
            id: i32::from_bytes(&data[0..4])?,
            position: Vector3::from_bytes(&data[4..16])?,
            rotation: Vector3::from_bytes(&data[16..28])?,
        })
    }
}

impl BufferCodec for Gd<PlayerSpaw> {
    const TYPE_ID: u8 = 53;

    fn to_bytes(&self) -> Vec<u8> {
        self.bind().to_bytes()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(Gd::from_object(PlayerSpaw::from_bytes(data).unwrap()))
    }
}
