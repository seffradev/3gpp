use crate::error::GtpError;
use crate::ie::types::ie_type;
use crate::{GtpMessage, Header, InformationElement};
use tokio_util::bytes::Bytes;

#[derive(Debug, Clone)]
pub struct DeleteSessionRequest {
    pub linked_ebi: u8,
    pub teid: u32,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for DeleteSessionRequest {
    type Error = GtpError;

    fn try_from(message: &GtpMessage) -> Result<Self, Self::Error> {
        let find = |ie_type: u8, instance: u8| {
            message
                .information_elements
                .iter()
                .find(|ie| ie.ie_type == ie_type && ie.instance == instance)
        };

        let ebi_ie = find(ie_type::EBI, 0).ok_or(GtpError::MissingIe(ie_type::EBI))?;
        let linked_ebi = *ebi_ie
            .value
            .first()
            .ok_or(GtpError::InvalidIeValue(ie_type::EBI))?;

        Ok(DeleteSessionRequest {
            linked_ebi,
            teid: message.header.teid.ok_or(GtpError::MissingTeid)?,
            sequence_number: message.header.sequence_number,
        })
    }
}

impl From<DeleteSessionRequest> for GtpMessage {
    fn from(delete_session_request: DeleteSessionRequest) -> Self {
        let ies = vec![InformationElement {
            ie_type: ie_type::EBI,
            instance: 0,
            value: Bytes::copy_from_slice(&[delete_session_request.linked_ebi]),
        }];

        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: true,
                message_type: 36, // Delete Session Request
                teid: Some(delete_session_request.teid),
                sequence_number: delete_session_request.sequence_number,
            },
            information_elements: ies,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::bytes::{Bytes, BytesMut};

    use crate::{
        GtpError, GtpMessage,
        ie::types::ie_type,
        messages::DeleteSessionRequest,
        test_helpers::{header, information_elements},
    };

    fn valid_delete_session_request_message() -> GtpMessage {
        GtpMessage {
            header: header(36, Some(0x2000_0002), 300),
            information_elements: vec![information_elements(ie_type::EBI, 0, &[5])],
        }
    }

    #[test]
    fn parses_valid_message() {
        let message = valid_delete_session_request_message();
        let delete_session_request =
            DeleteSessionRequest::try_from(&message).expect("should parse");

        assert_eq!(delete_session_request.linked_ebi, 5);
        assert_eq!(delete_session_request.teid, 0x2000_0002);
        assert_eq!(delete_session_request.sequence_number, 300);
    }

    #[test]
    fn missing_ebi_errors() {
        let mut message = valid_delete_session_request_message();
        message.information_elements.clear();

        let error = DeleteSessionRequest::try_from(&message).unwrap_err();
        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::EBI));
    }

    #[test]
    fn empty_ebi_value_errors() {
        let mut message = valid_delete_session_request_message();
        message.information_elements[0].value = Bytes::new(); // zero-length value, no EBI byte to read

        let error = DeleteSessionRequest::try_from(&message).unwrap_err();
        assert!(matches!(error, GtpError::InvalidIeValue(t) if t == ie_type::EBI));
    }

    #[test]
    fn missing_teid_in_header_errors() {
        let mut message = valid_delete_session_request_message();
        message.header.teid = None;
        message.header.teid_flag = false;

        let error = DeleteSessionRequest::try_from(&message).unwrap_err();
        assert!(matches!(error, GtpError::MissingTeid));
    }

    #[test]
    fn into_message_sets_correct_header() {
        let delete_session_request =
            DeleteSessionRequest::try_from(&valid_delete_session_request_message()).unwrap();
        let message: GtpMessage = delete_session_request.into();

        assert_eq!(message.header.message_type, 36);
        assert_eq!(message.header.teid, Some(0x2000_0002));
        assert_eq!(message.header.teid_flag, true);
        assert_eq!(message.information_elements.len(), 1);
        assert_eq!(message.information_elements[0].ie_type, ie_type::EBI);
    }

    #[test]
    fn full_roundtrip_through_wire_bytes() {
        let original = valid_delete_session_request_message();
        let delete_session_request = DeleteSessionRequest::try_from(&original).unwrap();
        let rebuilt_message: GtpMessage = delete_session_request.into();

        let mut wire = BytesMut::new();
        rebuilt_message.write(&mut wire);
        let reparsed_message = GtpMessage::parse(wire.freeze()).unwrap();

        let reparsed = DeleteSessionRequest::try_from(&reparsed_message).unwrap();
        assert_eq!(reparsed.linked_ebi, 5);
        assert_eq!(reparsed.teid, 0x2000_0002);
        assert_eq!(reparsed.sequence_number, 300);
    }
}
