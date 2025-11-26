mod chat_message;
mod client_connected;
mod location;
mod palyer_spaw;
mod player;
mod room_match;

use std::str::FromStr;

pub use chat_message::ChatMessage;
pub use client_connected::ClientConnected;
use godot::prelude::*;
pub use location::Location;
use network_types::connection::RawPacket;
pub use palyer_spaw::PlayerSpaw;
pub use player::Player;
pub use room_match::Match;

pub fn raw_packet_to_variant(raw_packet: &RawPacket) -> Variant {
    match raw_packet {
        RawPacket::String(text) => Variant::from(GString::from_str(text.as_str()).unwrap()),
        RawPacket::Int(int) => Variant::from(*int),
        RawPacket::Bool(bolean) => Variant::from(*bolean),
        RawPacket::Float(float) => Variant::from(*float),
        RawPacket::Vector3((x, y, z)) => Variant::from(Vector3::new(*x, *y, *z)),
        RawPacket::Array(data) => Variant::from(
            data.iter()
                .map(raw_packet_to_variant)
                .collect::<Vec<Variant>>(),
        ),
        RawPacket::Null => Variant::nil(),
    }
}

pub fn cast_array_based_on_type(variant_type: VariantType, value: Variant) -> Vec<RawPacket> {
    match variant_type {
        VariantType::INT => value
            .try_to::<Array<i32>>()
            .unwrap()
            .iter_shared()
            .map(|v| RawPacket::Int(v))
            .collect(),
        VariantType::FLOAT => value
            .try_to::<Array<f32>>()
            .unwrap()
            .iter_shared()
            .map(|v| RawPacket::Float(v))
            .collect(),
        VariantType::STRING => value
            .try_to::<Array<GString>>()
            .unwrap()
            .iter_shared()
            .map(|v| RawPacket::String(v.to_string()))
            .collect(),
        VariantType::BOOL => value
            .try_to::<Array<bool>>()
            .unwrap()
            .iter_shared()
            .map(|v| RawPacket::Bool(v))
            .collect(),
        VariantType::VECTOR3 => value
            .try_to::<Array<Vector3>>()
            .unwrap()
            .iter_shared()
            .map(|v3| RawPacket::Vector3((v3.x, v3.y, v3.x)))
            .collect(),
        _ => {
            godot_error!("Variant type is not a primitive {}", value);
            Vec::new()
        }
    }
}

pub fn variant_to_raw_packet(value: Variant) -> RawPacket {
    match value.get_type() {
        VariantType::INT => RawPacket::Int(value.try_to::<i32>().unwrap()),
        VariantType::FLOAT => RawPacket::Float(value.try_to::<f32>().unwrap()),
        VariantType::STRING => RawPacket::String(value.to_string()),
        VariantType::BOOL => RawPacket::Bool(value.try_to::<bool>().unwrap()),
        VariantType::VECTOR3 => {
            let v3 = value.try_to::<Vector3>().unwrap();
            RawPacket::Vector3((v3.x, v3.y, v3.x))
        }
        // ------------- NEW: PackedFloat32Array support -------------
        VariantType::PACKED_FLOAT32_ARRAY => {
            let arr = value.try_to::<PackedFloat32Array>().unwrap();
            RawPacket::Array(arr.to_vec().iter().map(|v| RawPacket::Float(*v)).collect())
        }
        VariantType::ARRAY => {
            let array_content_type = value
                .call("get_typed_builtin", &[])
                .try_to::<i32>()
                .unwrap();

            RawPacket::Array(cast_array_based_on_type(
                VariantType {
                    ord: array_content_type,
                },
                value,
            ))
        }
        _ => {
            godot_error!("Variant type is not a primitive {}", value);
            RawPacket::Null
        }
    }
}
