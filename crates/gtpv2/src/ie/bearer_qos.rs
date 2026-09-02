use crate::error::GtpError;
use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BearerQos {
    pub pci: bool,
    pub priority_level: u8,
    pub pvi: bool,
    pub qci: u8,
    pub max_bitrate_uplink_kbps: u64,
    pub max_bitrate_downlink_kbps: u64,
    pub guaranteed_bitrate_uplink_kbps: u64,
    pub guaranteed_bitrate_downlink_kbps: u64,
}

impl Default for BearerQos {
    fn default() -> Self {
        BearerQos {
            pci: false,
            priority_level: 8,
            pvi: false,
            qci: 9,
            max_bitrate_uplink_kbps: 0,
            max_bitrate_downlink_kbps: 0,
            guaranteed_bitrate_uplink_kbps: 0,
            guaranteed_bitrate_downlink_kbps: 0,
        }
    }
}

impl BearerQos {
    pub fn parse(mut value: Bytes) -> Result<Self, GtpError> {
        if value.remaining() < 22 {
            return Err(GtpError::Truncated);
        }

        let flags = value.get_u8();
        let pci = (flags & 0b0100_0000) != 0;
        let priority_level = (flags & 0b0011_1100) >> 2;
        let pvi = (flags & 0b0000_0001) != 0;

        let qci = value.get_u8();

        let max_bitrate_uplink_kbps = read_uint40(&mut value)?;
        let max_bitrate_downlink_kbps = read_uint40(&mut value)?;
        let guaranteed_bitrate_uplink_kbps = read_uint40(&mut value)?;
        let guaranteed_bitrate_downlink_kbps = read_uint40(&mut value)?;

        Ok(BearerQos {
            pci,
            priority_level,
            pvi,
            qci,
            max_bitrate_uplink_kbps,
            max_bitrate_downlink_kbps,
            guaranteed_bitrate_uplink_kbps,
            guaranteed_bitrate_downlink_kbps,
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let mut flags = 0u8;

        if self.pci {
            flags |= 0b0100_0000;
        }

        flags |= (self.priority_level & 0b0000_1111) << 2;

        if self.pvi {
            flags |= 0b0000_0001;
        }

        dst.put_u8(flags);

        dst.put_u8(self.qci);

        write_uint40(dst, self.max_bitrate_uplink_kbps);
        write_uint40(dst, self.max_bitrate_downlink_kbps);
        write_uint40(dst, self.guaranteed_bitrate_uplink_kbps);
        write_uint40(dst, self.guaranteed_bitrate_downlink_kbps);
    }
}

fn read_uint40(buf: &mut Bytes) -> Result<u64, GtpError> {
    if buf.remaining() < 5 {
        return Err(GtpError::Truncated);
    }

    Ok(buf.get_uint(5))
}

fn write_uint40(dst: &mut BytesMut, value: u64) {
    dst.put_uint(value, 5);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_all_fields() {
        let qos = BearerQos {
            pci: true,
            priority_level: 5,
            pvi: true,
            qci: 6,
            max_bitrate_uplink_kbps: 100_000,
            max_bitrate_downlink_kbps: 200_000,
            guaranteed_bitrate_uplink_kbps: 50_000,
            guaranteed_bitrate_downlink_kbps: 80_000,
        };

        let mut buf = BytesMut::new();

        qos.write(&mut buf);

        assert_eq!(buf.len(), 22);

        let parsed = BearerQos::parse(buf.freeze()).unwrap();

        assert_eq!(parsed, qos);
    }

    #[test]
    fn large_bitrate_near_40bit_max_roundtrips() {
        let max40 = (1u64 << 40) - 1;

        let qos = BearerQos {
            pci: false,
            priority_level: 1,
            pvi: false,
            qci: 1,
            max_bitrate_uplink_kbps: max40,
            max_bitrate_downlink_kbps: max40,
            guaranteed_bitrate_uplink_kbps: max40,
            guaranteed_bitrate_downlink_kbps: max40,
        };

        let mut buf = BytesMut::new();

        qos.write(&mut buf);

        let parsed = BearerQos::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.max_bitrate_uplink_kbps, max40);
    }

    #[test]
    fn truncated_value_errors() {
        let bytes = Bytes::from_static(&[0, 9, 0, 0, 0, 0, 0]);
        let err = BearerQos::parse(bytes).unwrap_err();

        assert!(matches!(err, GtpError::Truncated));
    }

    #[test]
    fn flags_byte_roundtrips_correctly() {
        let qos = BearerQos {
            pci: true,
            priority_level: 15,
            pvi: false,
            qci: 9,
            max_bitrate_uplink_kbps: 0,
            max_bitrate_downlink_kbps: 0,
            guaranteed_bitrate_uplink_kbps: 0,
            guaranteed_bitrate_downlink_kbps: 0,
        };

        let mut buf = BytesMut::new();

        qos.write(&mut buf);

        assert_eq!(buf[0], 0b0111_1100);
    }

    #[test]
    fn default_produces_expected_baseline_values() {
        let qos = BearerQos::default();

        assert_eq!(qos.qci, 9);
        assert_eq!(qos.priority_level, 8);
        assert!(!qos.pci);
        assert!(!qos.pvi);
        assert_eq!(qos.max_bitrate_uplink_kbps, 0);
        assert_eq!(qos.guaranteed_bitrate_uplink_kbps, 0);
    }

    #[test]
    fn struct_update_syntax_overrides_only_specified_fields() {
        let qos = BearerQos {
            qci: 5,
            ..Default::default()
        };

        assert_eq!(qos.qci, 5);
        assert_eq!(qos.priority_level, 8);
    }
}
