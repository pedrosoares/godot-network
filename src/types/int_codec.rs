use crate::types::buffer_codec::BufferCodec;

impl BufferCodec for i32 {
    const TYPE_ID: u8 = 2;

    fn to_bytes(&self) -> Vec<u8> {
        self.to_be_bytes().to_vec()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        if data.len() != 4 {
            return Err("Invalid Foo length".into());
        }
        Ok(i32::from_be_bytes(data.try_into().unwrap()))
    }
}
