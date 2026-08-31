use std::net::{Ipv4Addr, Ipv6Addr};
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

pub enum InterfaceType {
    S5S8SgwGtpU,
    S5S8PgwGtpU,
    S5S8SgwGtpC,
    S5S8PgwGtpC,
}

impl From<InterfaceType> for u8 {
    fn from(value: InterfaceType) -> Self {
        match value {
            InterfaceType::S5S8SgwGtpU => 4,
            InterfaceType::S5S8PgwGtpU => 5,
            InterfaceType::S5S8SgwGtpC => 6,
            InterfaceType::S5S8PgwGtpC => 7,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Fteid {
    pub interface_type: u8,
    pub teid_or_gre_key: u32,
    pub ipv4: Option<Ipv4Addr>,
    pub ipv6: Option<Ipv6Addr>,
}

impl Fteid {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 5 {
            return Err(GtpError::Truncated);
        }

        let flags = value.get_u8();
        let has_v4 = flags & 0b1000_0000 != 0;
        let has_v6 = flags & 0b0100_0000 != 0;
        let interface_type = flags & 0b0011_1111;

        let teid_or_gre_key = value.get_u32();

        let ipv4 = if has_v4 {
            if value.remaining() < 4 {
                return Err(GtpError::Truncated);
            }

            let mut octets = [0u8; 4];

            value.copy_to_slice(&mut octets);

            Some(Ipv4Addr::from(octets))
        } else {
            None
        };

        let ipv6 = if has_v6 {
            if value.remaining() < 16 {
                return Err(GtpError::Truncated);
            }

            let mut octets = [0u8; 16];

            value.copy_to_slice(&mut octets);

            Some(Ipv6Addr::from(octets))
        } else {
            None
        };

        Ok(Fteid {
            interface_type,
            teid_or_gre_key,
            ipv4,
            ipv6,
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let mut flags = self.interface_type & 0b0011_1111;

        if self.ipv4.is_some() {
            flags |= 0b1000_0000;
        }

        if self.ipv6.is_some() {
            flags |= 0b0100_0000;
        }

        dst.put_u8(flags);
        dst.put_u32(self.teid_or_gre_key);

        if let Some(v4) = self.ipv4 {
            dst.put_slice(&v4.octets());
        }

        if let Some(v6) = self.ipv6 {
            dst.put_slice(&v6.octets());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_only_roundtrip() {
        let fteid = Fteid {
            interface_type: 7,
            teid_or_gre_key: 0x1234_5678,
            ipv4: Some("192.168.1.1".parse().unwrap()),
            ipv6: None,
        };

        let mut buf = BytesMut::new();

        fteid.write(&mut buf);

        let parsed = Fteid::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.interface_type, 7);
        assert_eq!(parsed.teid_or_gre_key, 0x1234_5678);
        assert_eq!(parsed.ipv4, Some("192.168.1.1".parse().unwrap()));
        assert_eq!(parsed.ipv6, None);
    }

    #[test]
    fn ipv6_only_roundtrip() {
        let fteid = Fteid {
            interface_type: 0,
            teid_or_gre_key: 1,
            ipv4: None,
            ipv6: Some("2001:db8::1".parse().unwrap()),
        };

        let mut buf = BytesMut::new();

        fteid.write(&mut buf);

        let parsed = Fteid::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.ipv6, Some("2001:db8::1".parse().unwrap()));
        assert_eq!(parsed.ipv4, None);
    }

    #[test]
    fn dual_stack_roundtrip() {
        let fteid = Fteid {
            interface_type: 10,
            teid_or_gre_key: 0xFFFF_FFFF,
            ipv4: Some("10.0.0.1".parse().unwrap()),
            ipv6: Some("::1".parse().unwrap()),
        };

        let mut buf = BytesMut::new();

        fteid.write(&mut buf);

        let parsed = Fteid::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.ipv4, fteid.ipv4);
        assert_eq!(parsed.ipv6, fteid.ipv6);
    }

    #[test]
    fn interface_type_masks_to_six_bits() {
        let fteid = Fteid {
            interface_type: 0b1111_1111,
            teid_or_gre_key: 0,
            ipv4: None,
            ipv6: None,
        };

        let mut buf = BytesMut::new();

        fteid.write(&mut buf);

        assert_eq!(buf[0] & 0b0011_1111, 0b0011_1111);
        assert_eq!(buf[0] & 0b1100_0000, 0);
    }

    #[test]
    fn truncated_errors() {
        let bytes = Bytes::from_static(&[0x80, 0x00, 0x00]);
        let err = Fteid::parse(bytes).unwrap_err();

        assert!(matches!(err, GtpError::Truncated));
    }
}
