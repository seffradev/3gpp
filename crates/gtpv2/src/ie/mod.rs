use tokio_util::bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::GtpError;

pub mod ambr;
pub mod apn;
pub mod bearer_context;
pub mod bearer_qos;
pub mod cause;
pub mod fteid;
pub mod imsi;
pub mod rat_type;
pub mod selection_mode;
pub mod types;

#[derive(Clone, Debug)]
pub struct InformationElement {
    pub ie_type: u8,
    pub instance: u8,
    pub value: Bytes,
}

impl InformationElement {
    pub fn parse(buf: &mut Bytes) -> Result<Self, GtpError> {
        if buf.remaining() < 4 {
            return Err(GtpError::Truncated);
        }

        let ie_type = buf.get_u8();
        let len = buf.get_u16() as usize;
        let instance = buf.get_u8() & 0x0F;

        if buf.remaining() < len {
            return Err(GtpError::Truncated);
        }

        let value = buf.split_to(len);

        Ok(InformationElement {
            ie_type,
            instance,
            value,
        })
    }

    pub fn write(&self, dst: &mut BytesMut) {
        dst.put_u8(self.ie_type);
        dst.put_u16(self.value.len() as u16);
        dst.put_u8(self.instance & 0x0F);
        dst.put_slice(&self.value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_ie() {
        let bytes = Bytes::from_static(&[0x01, 0x00, 0x03, 0x00, 0x11, 0x22, 0x33]);
        let mut buf = bytes.clone();
        let ie = InformationElement::parse(&mut buf).unwrap();

        assert_eq!(ie.ie_type, 1);
        assert_eq!(ie.instance, 0);
        assert_eq!(&ie.value[..], &[0x11, 0x22, 0x33]);
        assert_eq!(buf.remaining(), 0, "parser should consume the whole IE");
    }

    #[test]
    fn instance_masks_out_spare_bits() {
        let bytes = Bytes::from_static(&[0x02, 0x00, 0x01, 0xF3, 0xAA]);
        let mut buf = bytes.clone();
        let ie = InformationElement::parse(&mut buf).unwrap();

        assert_eq!(ie.instance, 0x3);
    }

    #[test]
    fn zero_length_value_is_valid() {
        let bytes = Bytes::from_static(&[0x02, 0x00, 0x00, 0x00]);
        let mut buf = bytes.clone();
        let ie = InformationElement::parse(&mut buf).unwrap();

        assert_eq!(ie.value.len(), 0);
    }

    #[test]
    fn truncated_header_errors() {
        let bytes = Bytes::from_static(&[0x01, 0x00]);
        let mut buf = bytes.clone();
        let err = InformationElement::parse(&mut buf).unwrap_err();

        assert!(matches!(err, GtpError::Truncated));
    }

    #[test]
    fn truncated_value_errors() {
        let bytes = Bytes::from_static(&[0x01, 0x00, 0x05, 0x00, 0xAA, 0xBB]);
        let mut buf = bytes.clone();
        let err = InformationElement::parse(&mut buf).unwrap_err();

        assert!(matches!(err, GtpError::Truncated));
    }

    #[test]
    fn write_roundtrip() {
        let ie = InformationElement {
            ie_type: 87,
            instance: 2,
            value: Bytes::from_static(&[1, 2, 3, 4]),
        };

        let mut dst = BytesMut::new();

        ie.write(&mut dst);

        let mut reparsed_src = dst.freeze();
        let parsed = InformationElement::parse(&mut reparsed_src).unwrap();

        assert_eq!(parsed.ie_type, ie.ie_type);
        assert_eq!(parsed.instance, ie.instance);
        assert_eq!(parsed.value, Bytes::from_static(&[1, 2, 3, 4]));
    }

    #[test]
    fn sequential_ies_in_one_buffer() {
        let mut bytes = BytesMut::new();

        InformationElement {
            ie_type: 1,
            instance: 0,
            value: Bytes::from_static(&[0xAA]),
        }
        .write(&mut bytes);

        InformationElement {
            ie_type: 2,
            instance: 0,
            value: Bytes::from_static(&[0xBB, 0xCC]),
        }
        .write(&mut bytes);

        let mut buf = bytes.freeze();
        let first = InformationElement::parse(&mut buf).unwrap();
        let second = InformationElement::parse(&mut buf).unwrap();

        assert_eq!(first.ie_type, 1);
        assert_eq!(second.ie_type, 2);
        assert_eq!(buf.remaining(), 0);
    }
}
