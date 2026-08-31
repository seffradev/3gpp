use tokio_util::{
    bytes::BytesMut,
    codec::{Decoder, Encoder},
};

use crate::{error::GtpError, message::GtpMessage};

pub struct GtpCodec;

impl Decoder for GtpCodec {
    type Item = GtpMessage;
    type Error = GtpError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let msg_len = u16::from_be_bytes([src[2], src[3]]) as usize;
        let total_len = 4 + msg_len;

        if src.len() < total_len {
            src.reserve(total_len - src.len());
            return Ok(None);
        }

        let frame = src.split_to(total_len).freeze();
        GtpMessage::parse(frame).map(Some)
    }

    fn decode_eof(
        &mut self,
        buf: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        match self.decode(buf)? {
            Some(frame) => Ok(Some(frame)),
            None => {
                if buf.is_empty() {
                    Ok(None)
                } else {
                    Err(std::io::Error::other("bytes remaining on stream").into())
                }
            }
        }
    }
}

impl Encoder<GtpMessage> for GtpCodec {
    type Error = GtpError;

    fn encode(&mut self, item: GtpMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        item.write(dst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Header;
    use crate::ie::InformationElement;
    use tokio_util::bytes::Bytes;

    fn sample_message(seq: u32) -> GtpMessage {
        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: true,
                message_type: 33,
                teid: Some(0xAAAA_BBBB),
                sequence_number: seq,
            },
            information_elements: vec![InformationElement {
                ie_type: 2,
                instance: 0,
                value: Bytes::from_static(&[16, 0]),
            }],
        }
    }

    #[test]
    fn decode_returns_none_on_insufficient_length_bytes() {
        let mut codec = GtpCodec;
        let mut buf = BytesMut::from(&[0x48, 33][..]);
        let result = codec.decode(&mut buf).unwrap();

        assert!(result.is_none());
        assert_eq!(
            buf.len(),
            2,
            "decoder must not consume bytes when returning None"
        );
    }

    #[test]
    fn decode_returns_none_when_body_incomplete() {
        let mut codec = GtpCodec;
        let mut full = BytesMut::new();

        sample_message(1).write(&mut full);

        let partial = full[..full.len() - 2].to_vec();
        let mut buf = BytesMut::from(&partial[..]);
        let result = codec.decode(&mut buf).unwrap();

        assert!(result.is_none());
        assert_eq!(
            buf.len(),
            partial.len(),
            "no bytes should be consumed on incomplete frame"
        );
    }

    #[test]
    fn decode_succeeds_at_exact_boundary() {
        let mut codec = GtpCodec;
        let mut buf = BytesMut::new();

        sample_message(7).write(&mut buf);

        let msg = codec
            .decode(&mut buf)
            .unwrap()
            .expect("should decode a full frame");

        assert_eq!(msg.header.sequence_number, 7);
        assert_eq!(
            buf.len(),
            0,
            "exactly one frame's worth of bytes should be fully consumed"
        );
    }

    #[test]
    fn decode_leaves_extra_bytes_for_next_call() {
        let mut codec = GtpCodec;
        let mut buf = BytesMut::new();

        sample_message(1).write(&mut buf);
        sample_message(2).write(&mut buf);

        let first = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(first.header.sequence_number, 1);
        assert!(
            !buf.is_empty(),
            "second frame should still be sitting in the buffer"
        );

        let second = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(second.header.sequence_number, 2);
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn decode_handles_byte_by_byte_arrival() {
        let mut codec = GtpCodec;
        let mut full = BytesMut::new();

        sample_message(99).write(&mut full);

        let full = full.freeze();

        let mut buf = BytesMut::new();
        let mut result = None;

        for &byte in full.iter() {
            buf.extend_from_slice(&[byte]);
            result = codec.decode(&mut buf).unwrap();
            if result.is_some() {
                break;
            }
        }

        let msg = result.expect("should eventually decode once all bytes arrive");

        assert_eq!(msg.header.sequence_number, 99);
    }

    #[test]
    fn encode_then_decode_roundtrip() {
        let mut codec = GtpCodec;
        let original = sample_message(555);

        let mut buf = BytesMut::new();

        codec.encode(original.clone(), &mut buf).unwrap();

        let decoded = codec.decode(&mut buf).unwrap().unwrap();

        assert_eq!(
            decoded.header.sequence_number,
            original.header.sequence_number
        );
        assert_eq!(decoded.header.teid, original.header.teid);
        assert_eq!(
            decoded.information_elements.len(),
            original.information_elements.len()
        );
    }

    #[test]
    fn malformed_ie_length_propagates_as_error_not_panic() {
        let mut codec = GtpCodec;
        let mut buf = BytesMut::new();

        buf.extend_from_slice(&[0x48, 33, 0x00, 0x0A]);
        buf.extend_from_slice(&[0xAA, 0xAA, 0xAA, 0xAA]);
        buf.extend_from_slice(&[0x00, 0x00, 0x00]);
        buf.extend_from_slice(&[0x02, 0x00, 0xFF, 0x00]);

        let result = codec.decode(&mut buf);

        assert!(
            result.is_err(),
            "should error rather than panic on malformed nested IE"
        );
    }
}
