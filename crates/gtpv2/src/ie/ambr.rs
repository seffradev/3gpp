use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone, Copy)]
pub struct Ambr {
    pub uplink_kbps: u32,
    pub downlink_kbps: u32,
}

impl Ambr {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 8 {
            return Err(GtpError::Truncated);
        }

        Ok(Ambr {
            uplink_kbps: value.get_u32(),
            downlink_kbps: value.get_u32(),
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u32(self.uplink_kbps);
        dst.put_u32(self.downlink_kbps);
    }
}
