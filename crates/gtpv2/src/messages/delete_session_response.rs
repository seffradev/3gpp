use tokio_util::bytes::BytesMut;

use crate::error::GtpError;
use crate::ie::cause::Cause;
use crate::ie::types::{ie_type, message_type};
use crate::{GtpMessage, Header, InformationElement};

#[derive(Debug, Clone)]
pub struct DeleteSessionResponse {
    pub cause: Cause,
    pub teid: u32,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for DeleteSessionResponse {
    type Error = GtpError;

    fn try_from(message: &GtpMessage) -> Result<Self, Self::Error> {
        let find = |t: u8, inst: u8| {
            message
                .information_elements
                .iter()
                .find(|ie| ie.ie_type == t && ie.instance == inst)
        };

        let cause = Cause::parse(
            find(ie_type::CAUSE, 0)
                .ok_or(GtpError::MissingIe(ie_type::CAUSE))?
                .value
                .clone(),
        )?;

        Ok(DeleteSessionResponse {
            cause,
            teid: message.header.teid.ok_or(GtpError::MissingTeid)?,
            sequence_number: message.header.sequence_number,
        })
    }
}

impl From<DeleteSessionResponse> for GtpMessage {
    fn from(delete_session_response: DeleteSessionResponse) -> Self {
        let mut v = BytesMut::new();
        delete_session_response.cause.write(&mut v);

        let ies = vec![InformationElement {
            ie_type: ie_type::CAUSE,
            instance: 0,
            value: v.freeze(),
        }];

        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: true,
                message_type: message_type::DELETE_SESSION_RESPONSE,
                teid: Some(delete_session_response.teid),
                sequence_number: delete_session_response.sequence_number,
            },
            information_elements: ies,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::bytes::BytesMut;

    use crate::{
        GtpError, GtpMessage,
        ie::types::ie_type,
        messages::DeleteSessionResponse,
        test_helpers::{cause_bytes, header, information_elements},
    };

    fn valid_dsresp_message() -> GtpMessage {
        GtpMessage {
            header: header(37, Some(0x3000_0003), 400),
            information_elements: vec![
                information_elements(ie_type::CAUSE, 0, &cause_bytes(16)), // accepted
            ],
        }
    }

    #[test]
    fn parses_valid_message() {
        let message = valid_dsresp_message();
        let response = DeleteSessionResponse::try_from(&message).expect("should parse");

        assert!(response.cause.is_accepted());
        assert_eq!(response.teid, 0x3000_0003);
        assert_eq!(response.sequence_number, 400);
    }

    #[test]
    fn rejected_cause_still_parses_successfully() {
        let mut message = valid_dsresp_message();

        message.information_elements[0] = information_elements(ie_type::CAUSE, 0, &cause_bytes(2)); // e.g. "Context not found"

        let response = DeleteSessionResponse::try_from(&message).unwrap();

        assert!(!response.cause.is_accepted());
        assert_eq!(response.cause.value, 2);
    }

    #[test]
    fn missing_cause_errors() {
        let mut message = valid_dsresp_message();

        message.information_elements.clear();

        let error = DeleteSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::CAUSE));
    }

    #[test]
    fn missing_teid_in_header_errors() {
        let mut message = valid_dsresp_message();

        message.header.teid = None;
        message.header.teid_flag = false;

        let error = DeleteSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingTeid));
    }

    #[test]
    fn into_message_sets_correct_header() {
        let response = DeleteSessionResponse::try_from(&valid_dsresp_message()).unwrap();
        let message: GtpMessage = response.into();

        assert_eq!(message.header.message_type, 37);
        assert_eq!(message.header.teid, Some(0x3000_0003));
        assert_eq!(message.information_elements.len(), 1);
        assert_eq!(message.information_elements[0].ie_type, ie_type::CAUSE);
    }

    #[test]
    fn full_roundtrip_through_wire_bytes() {
        let original = valid_dsresp_message();
        let response = DeleteSessionResponse::try_from(&original).unwrap();
        let rebuilt_message: GtpMessage = response.into();

        let mut wire = BytesMut::new();

        rebuilt_message.write(&mut wire);

        let reparsed_message = GtpMessage::parse(wire.freeze()).unwrap();

        let reparsed = DeleteSessionResponse::try_from(&reparsed_message).unwrap();

        assert!(reparsed.cause.is_accepted());
        assert_eq!(reparsed.teid, 0x3000_0003);
    }
}
