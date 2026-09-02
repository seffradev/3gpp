use crate::error::GtpError;
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdnType {
    Ipv4 = 1,
    Ipv6 = 2,
    Ipv4v6 = 3,
    NonIp = 4,
}

impl PdnType {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }
        let raw = value.get_u8() & 0b0000_0111; // low 3 bits
        PdnType::try_from(raw)
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(*self as u8);
    }
}

impl TryFrom<u8> for PdnType {
    type Error = GtpError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        match raw {
            1 => Ok(PdnType::Ipv4),
            2 => Ok(PdnType::Ipv6),
            3 => Ok(PdnType::Ipv4v6),
            4 => Ok(PdnType::NonIp),
            other => Err(GtpError::InvalidPdnType(other)),
        }
    }
}
