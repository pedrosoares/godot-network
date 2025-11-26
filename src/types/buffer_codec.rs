use godot::classes::class_macros::private::virtuals::Os::{Vector2, Vector3};

use crate::godot_types::{ChatMessage, ClientConnected, Location, PlayerSpaw};

pub trait BufferCodec: Sized {
    const TYPE_ID: u8;

    /// Serialize self → Vec<u8> (not including the type ID)
    fn to_bytes(&self) -> Vec<u8>;

    /// Deserialize from &[u8] (not including the type ID)
    fn from_bytes(data: &[u8]) -> Result<Self, String>;
}

#[derive(Debug, PartialEq)]
pub enum Message {
    String(String),
    Int(i32),
    Float(f32),
    Vector3(Vector3),
    Vector2(Vector2),
    Location(Location),
    ChatMessage(ChatMessage),
    ClientConnected(ClientConnected),
    PlayerSpaw(PlayerSpaw),
}

pub fn encode<T: BufferCodec>(value: &T) -> Vec<u8> {
    let mut out = vec![T::TYPE_ID];
    out.extend_from_slice(&value.to_bytes());
    out
}

pub fn decode(buf: &[u8]) -> Result<Message, String> {
    if buf.is_empty() {
        return Err("Empty buffer".into());
    }

    let id = buf[0];
    let data = &buf[1..];

    match id {
        String::TYPE_ID => Ok(Message::String(String::from_bytes(data)?)),
        i32::TYPE_ID => Ok(Message::Int(i32::from_bytes(data)?)),
        f32::TYPE_ID => Ok(Message::Float(f32::from_bytes(data)?)),
        Vector3::TYPE_ID => Ok(Message::Vector3(Vector3::from_bytes(data)?)),
        Vector2::TYPE_ID => Ok(Message::Vector2(Vector2::from_bytes(data)?)),
        Location::TYPE_ID => Ok(Message::Location(Location::from_bytes(data)?)),
        ChatMessage::TYPE_ID => Ok(Message::ChatMessage(ChatMessage::from_bytes(data)?)),
        ClientConnected::TYPE_ID => {
            Ok(Message::ClientConnected(ClientConnected::from_bytes(data)?))
        }
        PlayerSpaw::TYPE_ID => Ok(Message::PlayerSpaw(PlayerSpaw::from_bytes(data)?)),
        _ => Err(format!("Unknown type ID: {}", id)),
    }
}
