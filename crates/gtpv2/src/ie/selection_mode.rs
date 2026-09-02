use crate::error::GtpError;
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionMode {
    MsOrNetworkProvidedSubscriptionVerified = 0,
    MsProvidedSubscriptionNotVerified = 1,
    NetworkProvidedSubscriptionNotVerified = 2,
}

impl SelectionMode {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }
        let raw = value.get_u8() & 0b0000_0011; // low 2 bits
        SelectionMode::try_from(raw)
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(*self as u8);
    }
}

impl TryFrom<u8> for SelectionMode {
    type Error = GtpError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        match raw {
            0 => Ok(SelectionMode::MsOrNetworkProvidedSubscriptionVerified),
            1 => Ok(SelectionMode::MsProvidedSubscriptionNotVerified),
            2 => Ok(SelectionMode::NetworkProvidedSubscriptionNotVerified),
            other => Err(GtpError::InvalidSelectionMode(other)),
        }
    }
}
