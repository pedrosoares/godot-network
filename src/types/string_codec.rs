use crate::types::buffer_codec::BufferCodec;

impl BufferCodec for String {
    const TYPE_ID: u8 = 1;

    fn to_bytes(&self) -> Vec<u8> {
        self.as_bytes().to_vec()
    }

    fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Ok(String::from_utf8_lossy(data).to_string())
    }
}
