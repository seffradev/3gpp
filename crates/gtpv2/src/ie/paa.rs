use crate::error::GtpError;
use crate::ie::pdn_type::PdnType;
use std::net::{Ipv4Addr, Ipv6Addr};
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone)]
pub enum Paa {
    V4 {
        address: Ipv4Addr,
    },
    V6 {
        prefix_length: u8,
        address: Ipv6Addr,
    },
    V4V6 {
        prefix_length: u8,
        v6_address: Ipv6Addr,
        v4_address: Ipv4Addr,
    },
}

impl Paa {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 1 {
            return Err(GtpError::Truncated);
        }

        let pdn_type_raw = value.get_u8() & 0b0000_0111;
        let pdn_type = PdnType::try_from(pdn_type_raw)?;

        match pdn_type {
            PdnType::Ipv4 => {
                if value.remaining() < 4 {
                    return Err(GtpError::Truncated);
                }

                let mut octets = [0u8; 4];

                value.copy_to_slice(&mut octets);

                Ok(Paa::V4 {
                    address: Ipv4Addr::from(octets),
                })
            }
            PdnType::Ipv6 => {
                if value.remaining() < 17 {
                    return Err(GtpError::Truncated);
                }

                let prefix_len = value.get_u8();
                let mut octets = [0u8; 16];

                value.copy_to_slice(&mut octets);

                Ok(Paa::V6 {
                    prefix_length: prefix_len,
                    address: Ipv6Addr::from(octets),
                })
            }
            PdnType::Ipv4v6 => {
                if value.remaining() < 21 {
                    return Err(GtpError::Truncated);
                }

                let prefix_len = value.get_u8();
                let mut v6_octets = [0u8; 16];

                value.copy_to_slice(&mut v6_octets);

                let mut v4_octets = [0u8; 4];

                value.copy_to_slice(&mut v4_octets);

                Ok(Paa::V4V6 {
                    prefix_length: prefix_len,
                    v6_address: Ipv6Addr::from(v6_octets),
                    v4_address: Ipv4Addr::from(v4_octets),
                })
            }
            PdnType::NonIp => Err(GtpError::InvalidPdnType(pdn_type_raw)),
        }
    }

    pub fn write(&self, dst: &mut BytesMut) {
        match self {
            Paa::V4 { address } => {
                dst.put_u8(PdnType::Ipv4 as u8);
                dst.put_slice(&address.octets());
            }
            Paa::V6 {
                prefix_length,
                address,
            } => {
                dst.put_u8(PdnType::Ipv6 as u8);
                dst.put_u8(*prefix_length);
                dst.put_slice(&address.octets());
            }
            Paa::V4V6 {
                prefix_length,
                v6_address,
                v4_address,
            } => {
                dst.put_u8(PdnType::Ipv4v6 as u8);
                dst.put_u8(*prefix_length);
                dst.put_slice(&v6_address.octets());
                dst.put_slice(&v4_address.octets());
            }
        }
    }

    pub fn request_v4() -> Self {
        Paa::V4 {
            address: Ipv4Addr::UNSPECIFIED,
        }
    }
}
