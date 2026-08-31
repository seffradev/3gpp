use crate::error::GtpError;
use crate::header::Header;
use crate::ie::InformationElement;
use tokio_util::bytes::{Bytes, BytesMut};

#[derive(Debug, Clone)]
pub struct GtpMessage {
    pub header: Header,
    pub information_elements: Vec<InformationElement>,
}

impl GtpMessage {
    pub fn parse(mut frame: Bytes) -> Result<Self, GtpError> {
        let header = Header::parse(&mut frame)?;
        let mut ies = Vec::new();

        while !frame.is_empty() {
            ies.push(InformationElement::parse(&mut frame)?);
        }

        Ok(GtpMessage {
            header,
            information_elements: ies,
        })
    }

    pub fn write(&self, destination: &mut BytesMut) {
        let mut body = BytesMut::new();

        for ie in &self.information_elements {
            ie.write(&mut body);
        }

        self.header.write(destination, body.len() as u16);
        destination.extend_from_slice(&body);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ie::{InformationElement, types::ie_type};

    fn sample_message() -> GtpMessage {
        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: true,
                message_type: 33,
                teid: Some(0x1000_0001),
                sequence_number: 42,
            },
            information_elements: vec![
                InformationElement {
                    ie_type: ie_type::CAUSE,
                    instance: 0,
                    value: Bytes::from_static(&[16, 0]),
                },
                InformationElement {
                    ie_type: ie_type::EBI,
                    instance: 0,
                    value: Bytes::from_static(&[5]),
                },
            ],
        }
    }

    #[test]
    fn write_then_parse_roundtrip() {
        let message = sample_message();
        let mut destination = BytesMut::new();

        message.write(&mut destination);

        let parsed = GtpMessage::parse(destination.freeze()).unwrap();

        assert_eq!(parsed.header.message_type, 33);
        assert_eq!(parsed.header.teid, Some(0x1000_0001));
        assert_eq!(parsed.header.sequence_number, 42);
        assert_eq!(parsed.information_elements.len(), 2);
        assert_eq!(parsed.information_elements[0].ie_type, 2);
        assert_eq!(parsed.information_elements[1].ie_type, 73);
    }

    #[test]
    fn length_field_matches_actual_frame_size() {
        let message = sample_message();
        let mut destination = BytesMut::new();

        message.write(&mut destination);

        let declared_length = u16::from_be_bytes([destination[2], destination[3]]) as usize;

        assert_eq!(
            destination.len(),
            4 + declared_length,
            "message_length must describe everything after the length field"
        );
    }

    #[test]
    fn no_ies_roundtrip() {
        let message = GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: false,
                message_type: 3,
                teid: None,
                sequence_number: 7,
            },
            information_elements: vec![],
        };

        let mut destination = BytesMut::new();

        message.write(&mut destination);

        let parsed = GtpMessage::parse(destination.freeze()).unwrap();

        assert_eq!(parsed.information_elements.len(), 0);
    }

    #[test]
    fn trailing_garbage_after_declared_ies_is_still_parsed() {
        let mut destination = BytesMut::new();
        let message = sample_message();

        message.write(&mut destination);
        destination.truncate(destination.len() - 1);

        let err = GtpMessage::parse(destination.freeze());

        assert!(
            err.is_err(),
            "corrupted trailing IE should fail rather than silently succeed"
        );
    }
}
