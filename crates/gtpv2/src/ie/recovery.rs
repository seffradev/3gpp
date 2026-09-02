use crate::error::GtpError;
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recovery {
    pub restart_counter: u8,
}

impl Recovery {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }

        Ok(Recovery {
            restart_counter: value.get_u8(),
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(self.restart_counter);
    }
}
