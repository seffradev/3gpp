use tokio_util::bytes::{BufMut, Bytes, BytesMut};

use crate::error::GtpError;

#[derive(Debug, Clone)]
pub struct Imsi(pub String);

impl Imsi {
    pub fn parse(value: Bytes) -> Result<Self, GtpError> {
        let mut digits = String::new();

        for byte in value.iter() {
            let lo = byte & 0x0F;
            let hi = (byte & 0xF0) >> 4;

            if lo == 0x0F {
                break;
            }

            digits.push((b'0' + lo) as char);

            if hi == 0x0F {
                break;
            }

            digits.push((b'0' + hi) as char);
        }

        Ok(Imsi(digits))
    }

    pub fn write(&self, dst: &mut BytesMut) {
        let digits: Vec<u8> = self.0.bytes().map(|b| b - b'0').collect();

        for pair in digits.chunks(2) {
            let lo = pair[0];
            let hi = pair.get(1).copied().unwrap_or(0x0F);

            dst.put_u8(lo | (hi << 4));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_digit_count_roundtrip() {
        let imsi = Imsi("123456789012".to_string());
        let mut buf = BytesMut::new();

        imsi.write(&mut buf);

        assert_eq!(buf.len(), 6);

        let parsed = Imsi::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.0, "123456789012");
    }

    #[test]
    fn odd_digit_count_roundtrip() {
        let imsi = Imsi("12345678901".to_string());
        let mut buf = BytesMut::new();

        imsi.write(&mut buf);

        assert_eq!(buf.len(), 6);

        let parsed = Imsi::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.0, "12345678901");
    }

    #[test]
    fn known_vector() {
        let imsi = Imsi("001010123456789".to_string());
        let mut buf = BytesMut::new();

        imsi.write(&mut buf);

        let parsed = Imsi::parse(buf.freeze()).unwrap();

        assert_eq!(parsed.0, "001010123456789");
    }
}
