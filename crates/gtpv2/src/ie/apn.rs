use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone)]
pub struct Apn(pub String);

impl Apn {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        let mut labels = Vec::new();
        while value.has_remaining() {
            let len = value.get_u8() as usize;
            if value.remaining() < len {
                return Err(GtpError::Truncated);
            }
            let label = value.split_to(len);
            labels.push(String::from_utf8_lossy(&label).into_owned());
        }
        Ok(Apn(labels.join(".")))
    }

    pub fn write(&self, dst: &mut BytesMut) {
        for label in self.0.split('.') {
            dst.put_u8(label.len() as u8);
            dst.put_slice(label.as_bytes());
        }
    }
}
