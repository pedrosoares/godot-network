use godot::prelude::*;

use crate::types::buffer_codec::BufferCodec;

#[derive(Debug, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct ClientConnected {
    #[var]
    pub id: i32,
}

#[godot_api]
impl ClientConnected {
    pub fn new(id: i32) -> Self {
        Self { id }
    }

    #[func]
    pub fn init() -> Gd<Self> {
        Gd::from_object(Self::new(0))
    }
}

impl BufferCodec for ClientConnected {
    const TYPE_ID: u8 = 52;

    fn to_bytes(&self) -> Vec<u8> {
        self.id.to_bytes()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(ClientConnected {
            id: i32::from_bytes(&data[0..4])?,
        })
    }
}

impl BufferCodec for Gd<ClientConnected> {
    const TYPE_ID: u8 = 52;

    fn to_bytes(&self) -> Vec<u8> {
        self.bind().to_bytes()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(Gd::from_object(ClientConnected::from_bytes(data).unwrap()))
    }
}
