//! `RemoteObjectCall` parameters <-> Godot variants.

use godot::prelude::*;
use mw_protocol::RawPacket;

pub fn raw_packet_to_variant(raw: &RawPacket) -> Variant {
    match raw {
        RawPacket::String(text) => GString::from(text.as_str()).to_variant(),
        RawPacket::Int(int) => int.to_variant(),
        RawPacket::Bool(boolean) => boolean.to_variant(),
        RawPacket::Float(float) => float.to_variant(),
        RawPacket::Vector3((x, y, z)) => Vector3::new(*x, *y, *z).to_variant(),
        RawPacket::Array(items) => items
            .iter()
            .map(raw_packet_to_variant)
            .collect::<VarArray>()
            .to_variant(),
        RawPacket::Null => Variant::nil(),
    }
}

pub fn variant_to_raw_packet(value: &Variant) -> RawPacket {
    match value.get_type() {
        VariantType::NIL => RawPacket::Null,
        VariantType::INT => RawPacket::Int(value.to::<i64>() as i32),
        VariantType::FLOAT => RawPacket::Float(value.to::<f64>() as f32),
        VariantType::STRING | VariantType::STRING_NAME => RawPacket::String(value.to_string()),
        VariantType::BOOL => RawPacket::Bool(value.to::<bool>()),
        VariantType::VECTOR3 => {
            let v = value.to::<Vector3>();
            RawPacket::Vector3((v.x, v.y, v.z))
        }
        VariantType::PACKED_FLOAT32_ARRAY => RawPacket::Array(
            value
                .to::<PackedFloat32Array>()
                .as_slice()
                .iter()
                .map(|v| RawPacket::Float(*v))
                .collect(),
        ),
        VariantType::PACKED_BYTE_ARRAY => RawPacket::Array(
            value
                .to::<PackedByteArray>()
                .as_slice()
                .iter()
                .map(|v| RawPacket::Int(i32::from(*v)))
                .collect(),
        ),
        VariantType::PACKED_INT32_ARRAY => RawPacket::Array(
            value
                .to::<PackedInt32Array>()
                .as_slice()
                .iter()
                .map(|v| RawPacket::Int(*v))
                .collect(),
        ),
        VariantType::ARRAY => {
            // Typed and untyped arrays alike: convert element by element.
            let size = value.call("size", &[]).to::<i64>();
            let items = (0..size)
                .map(|i| variant_to_raw_packet(&value.call("get", &[i.to_variant()])))
                .collect();
            RawPacket::Array(items)
        }
        other => {
            godot_error!("godot_network: {other:?} can't be sent as a RemoteObjectCall parameter");
            RawPacket::Null
        }
    }
}
