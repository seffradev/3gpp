use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone)]
pub struct Header {
    pub version: u8,
    pub piggybacking_flag: bool,
    pub teid_flag: bool,
    pub message_type: u8,
    pub teid: Option<u32>,
    pub sequence_number: u32,
}

impl Header {
    pub fn parse(buf: &mut Bytes) -> Result<Self, GtpError> {
        let b0 = buf.get_u8();
        let version = b0 >> 5;
        let piggybacking_flag = (b0 & 0b0001_0000) != 0;
        let teid_flag = (b0 & 0b0000_1000) != 0;
        let message_type = buf.get_u8();
        let _length = buf.get_u16();
        let teid = if teid_flag { Some(buf.get_u32()) } else { None };
        let seq_hi = buf.get_uint(3) as u32;
        buf.get_u8();

        Ok(Header {
            version,
            piggybacking_flag,
            teid_flag,
            message_type,
            teid,
            sequence_number: seq_hi,
        })
    }

    pub fn write(&self, dst: &mut BytesMut, ie_body_len: u16) {
        let fixed_len: u16 = if self.teid_flag { 4 + 3 + 1 } else { 3 + 1 };
        let message_length = fixed_len + ie_body_len;

        let mut b0 = self.version << 5;

        if self.piggybacking_flag {
            b0 |= 0b0001_0000;
        }

        if self.teid_flag {
            b0 |= 0b0000_1000;
        }

        dst.put_u8(b0);
        dst.put_u8(self.message_type);
        dst.put_u16(message_length);

        if let Some(teid) = self.teid {
            dst.put_u32(teid);
        }

        dst.put_uint(self.sequence_number as u64, 3);
        dst.put_u8(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_util::bytes::{Bytes, BytesMut};

    #[test]
    fn roundtrip_without_teid() {
        let header = Header {
            version: 2,
            piggybacking_flag: false,
            teid_flag: false,
            message_type: 32,
            teid: None,
            sequence_number: 0x00_01_02,
        };

        let mut dst = BytesMut::new();
        header.write(&mut dst, 0);

        assert_eq!(dst.len(), 8);

        let mut src = dst.freeze();
        let (_len_field, parsed) = parse_with_length(&mut src);

        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.teid_flag, false);
        assert_eq!(parsed.teid, None);
        assert_eq!(parsed.message_type, 32);
        assert_eq!(parsed.sequence_number, 0x00_01_02);
    }

    #[test]
    fn roundtrip_with_teid() {
        let header = Header {
            version: 2,
            piggybacking_flag: false,
            teid_flag: true,
            message_type: 33,
            teid: Some(0xDEAD_BEEF),
            sequence_number: 0x00_AB_CD,
        };

        let mut dst = BytesMut::new();

        header.write(&mut dst, 0);

        assert_eq!(dst.len(), 12);

        let mut src = dst.freeze();
        let (_len_field, parsed) = parse_with_length(&mut src);

        assert_eq!(parsed.teid, Some(0xDEAD_BEEF));
        assert_eq!(parsed.sequence_number, 0x00_AB_CD);
    }

    #[test]
    fn message_length_excludes_first_four_bytes() {
        let header = Header {
            version: 2,
            piggybacking_flag: false,
            teid_flag: true,
            message_type: 33,
            teid: Some(1),
            sequence_number: 1,
        };

        let mut dst = BytesMut::new();

        header.write(&mut dst, 10);

        let msg_len = u16::from_be_bytes([dst[2], dst[3]]);

        assert_eq!(msg_len, 18);
    }

    #[test]
    fn flags_byte_encodes_version_and_bits() {
        let header = Header {
            version: 2,
            piggybacking_flag: true,
            teid_flag: true,
            message_type: 1,
            teid: Some(0),
            sequence_number: 0,
        };

        let mut dst = BytesMut::new();
        header.write(&mut dst, 0);

        assert_eq!(dst[0], 0b0101_1000);
    }

    fn parse_with_length(src: &mut Bytes) -> (u16, Header) {
        let _flags_and_type = &src[0..2];
        let len = u16::from_be_bytes([src[2], src[3]]);
        let header = Header::parse(src).unwrap();

        (len, header)
    }
}
