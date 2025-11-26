use godot::classes::class_macros::private::virtuals::Os::{Vector2, Vector3};

use crate::types::buffer_codec::BufferCodec;

impl BufferCodec for Vector3 {
    const TYPE_ID: u8 = 10;

    fn to_bytes(&self) -> Vec<u8> {
        [
            self.x.to_be_bytes().as_ref(),
            self.y.to_be_bytes().as_ref(),
            self.z.to_be_bytes().as_ref(),
        ]
        .concat()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() != 12 {
            return Err("Invalid Bar length".into());
        }
        Ok(Vector3 {
            x: f32::from_bytes(&data[0..4]).unwrap(),
            y: f32::from_bytes(&data[4..8]).unwrap(),
            z: f32::from_bytes(&data[8..12]).unwrap(),
        })
    }
}

impl BufferCodec for Vector2 {
    const TYPE_ID: u8 = 11;

    fn to_bytes(&self) -> Vec<u8> {
        [self.x.to_be_bytes().as_ref(), self.y.to_be_bytes().as_ref()].concat()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() != 8 {
            return Err("Invalid Bar length".into());
        }
        Ok(Vector2 {
            x: f32::from_bytes(&data[0..4]).unwrap(),
            y: f32::from_bytes(&data[4..8]).unwrap(),
        })
    }
}
