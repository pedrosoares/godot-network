use std::str::FromStr;

use godot::prelude::*;

use crate::types::buffer_codec::BufferCodec;

#[derive(Debug, PartialEq, GodotClass)]
#[class(no_init, base=RefCounted)]
pub struct ChatMessage {
    #[var]
    pub name: GString,
    #[var]
    pub text: GString,
}

#[godot_api]
impl ChatMessage {
    pub fn new() -> Self {
        Self {
            name: GString::new(),
            text: GString::new(),
        }
    }

    #[func]
    pub fn init() -> Gd<Self> {
        Gd::from_object(Self::new())
    }
}

impl BufferCodec for ChatMessage {
    const TYPE_ID: u8 = 51;

    fn to_bytes(&self) -> Vec<u8> {
        [
            vec![self.name.len() as u8],
            self.name.to_string().to_bytes(),
            vec![self.text.len() as u8],
            self.text.to_string().to_bytes(),
        ]
        .concat()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let first_string_size = data[0] as usize + 1;
        Ok(ChatMessage {
            name: GString::from_str(
                String::from_bytes(&data[1..first_string_size])
                    .unwrap()
                    .as_str(),
            )
            .unwrap(),
            text: GString::from_str(
                String::from_bytes(&data[first_string_size..])
                    .unwrap()
                    .as_str(),
            )
            .unwrap(),
        })
    }
}

impl BufferCodec for Gd<ChatMessage> {
    const TYPE_ID: u8 = 51;

    fn to_bytes(&self) -> Vec<u8> {
        self.bind().to_bytes()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(Gd::from_object(ChatMessage::from_bytes(data).unwrap()))
    }
}
