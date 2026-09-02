use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatType {
    Utran = 1,
    Geran = 2,
    Wlan = 3,
    Gan = 4,
    HspaEvolution = 5,
    Eutran = 6,
    Virtual = 7,
    Nbiot = 8,
    Ltem = 9,
    Nr = 10,
}

impl RatType {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }
        let raw = value.get_u8();
        RatType::try_from(raw)
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(*self as u8);
    }
}

impl TryFrom<u8> for RatType {
    type Error = GtpError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        match raw {
            1 => Ok(RatType::Utran),
            2 => Ok(RatType::Geran),
            3 => Ok(RatType::Wlan),
            4 => Ok(RatType::Gan),
            5 => Ok(RatType::HspaEvolution),
            6 => Ok(RatType::Eutran),
            7 => Ok(RatType::Virtual),
            8 => Ok(RatType::Nbiot),
            9 => Ok(RatType::Ltem),
            10 => Ok(RatType::Nr),
            other => Err(GtpError::InvalidRatType(other)),
        }
    }
}
