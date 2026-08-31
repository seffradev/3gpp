use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cause {
    pub value: u8,
}

impl Cause {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }

        let cause_value = value.get_u8();

        Ok(Cause { value: cause_value })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(self.value);
        dst.put_u8(0);
    }

    pub fn is_accepted(&self) -> bool {
        self.value == 16
    }
}
