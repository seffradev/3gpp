use tokio_util::bytes::BytesMut;

use crate::{
    error::GtpError,
    header::Header,
    ie::{
        InformationElement, ambr::Ambr, bearer_context::BearerContext, cause::Cause, fteid::Fteid,
        types::ie_type,
    },
    message::GtpMessage,
};

#[derive(Debug, Clone)]
pub struct CreateSessionResponse {
    pub cause: Cause,
    pub sender_fteid: Fteid,
    pub ambr: Option<Ambr>,
    pub bearer_context: BearerContext,
    pub teid: u32,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for CreateSessionResponse {
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
        let sender_fteid = Fteid::parse(
            find(ie_type::FTEID, 1)
                .ok_or(GtpError::MissingIe(ie_type::FTEID))?
                .value
                .clone(),
        )?;
        let ambr = find(ie_type::AMBR, 0)
            .map(|ie| Ambr::parse(ie.value.clone()))
            .transpose()?;
        let bearer_ie =
            find(ie_type::BEARER_CONTEXT, 0).ok_or(GtpError::MissingIe(ie_type::BEARER_CONTEXT))?;
        let bearer_context = BearerContext::parse(bearer_ie.value.clone())?;

        Ok(CreateSessionResponse {
            cause,
            sender_fteid,
            ambr,
            bearer_context,
            teid: message.header.teid.ok_or(GtpError::MissingTeid)?,
            sequence_number: message.header.sequence_number,
        })
    }
}

impl From<CreateSessionResponse> for GtpMessage {
    fn from(create_session_response: CreateSessionResponse) -> Self {
        let mut ies = Vec::new();

        let mut v = BytesMut::new();
        create_session_response.cause.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::CAUSE,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_response.sender_fteid.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::FTEID,
            instance: 1,
            value: v.freeze(),
        });

        if let Some(ambr) = &create_session_response.ambr {
            let mut v = BytesMut::new();
            ambr.write(&mut v);
            ies.push(InformationElement {
                ie_type: ie_type::AMBR,
                instance: 0,
                value: v.freeze(),
            });
        }

        let bearer_value = create_session_response.bearer_context.write().freeze();
        ies.push(InformationElement {
            ie_type: ie_type::BEARER_CONTEXT,
            instance: 0,
            value: bearer_value,
        });

        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: true,
                message_type: 33, // Create Session Response
                teid: Some(create_session_response.teid),
                sequence_number: create_session_response.sequence_number,
            },
            information_elements: ies,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        GtpError, GtpMessage,
        ie::types::ie_type,
        messages::CreateSessionResponse,
        test_helpers::{
            ambr_bytes, bearer_context_bytes, cause_bytes, fteid_bytes, header,
            information_elements,
        },
    };
    use tokio_util::bytes::BytesMut;

    fn valid_csresp_message() -> GtpMessage {
        GtpMessage {
            header: header(33, Some(0x1000_0001), 200),
            information_elements: vec![
                information_elements(ie_type::CAUSE, 0, &cause_bytes(16)),
                information_elements(
                    ie_type::FTEID,
                    1,
                    &fteid_bytes(7, 0xDEAD_BEEF, [172, 16, 0, 1]),
                ),
                information_elements(ie_type::AMBR, 0, &ambr_bytes(50_000, 100_000)),
                information_elements(
                    ie_type::BEARER_CONTEXT,
                    0,
                    &bearer_context_bytes(5, Some(16), None, None),
                ),
            ],
        }
    }

    #[test]
    fn parses_valid_message() {
        let message = valid_csresp_message();
        let response = CreateSessionResponse::try_from(&message).expect("should parse");

        assert!(response.cause.is_accepted());
        assert_eq!(response.sender_fteid.teid_or_gre_key, 0xDEAD_BEEF);
        assert_eq!(
            response.sender_fteid.ipv4,
            Some("172.16.0.1".parse().unwrap())
        );
        assert_eq!(response.ambr.unwrap().uplink_kbps, 50_000);
        assert_eq!(response.bearer_context.ebi, Some(5));
        assert_eq!(response.teid, 0x1000_0001);
        assert_eq!(response.sequence_number, 200);
    }

    #[test]
    fn ambr_is_optional() {
        let mut message = valid_csresp_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::AMBR);

        let response =
            CreateSessionResponse::try_from(&message).expect("AMBR absence should not error");

        assert!(response.ambr.is_none());
    }

    #[test]
    fn rejected_cause_still_parses_successfully() {
        let mut message = valid_csresp_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::CAUSE {
                *ie =
                    crate::test_helpers::information_elements(ie_type::CAUSE, 0, &cause_bytes(73)); // e.g. "No resources available"
            }
        }

        let response = CreateSessionResponse::try_from(&message).unwrap();

        assert!(!response.cause.is_accepted());
        assert_eq!(response.cause.value, 73);
    }

    #[test]
    fn missing_cause_errors() {
        let mut message = valid_csresp_message();
        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::CAUSE);

        let error = CreateSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::CAUSE));
    }

    #[test]
    fn missing_fteid_errors() {
        let mut message = valid_csresp_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::FTEID);

        let error = CreateSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::FTEID));
    }

    #[test]
    fn wrong_fteid_instance_is_treated_as_missing() {
        let mut message = valid_csresp_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::FTEID {
                ie.instance = 0;
            }
        }

        let error = CreateSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::FTEID));
    }

    #[test]
    fn missing_bearer_context_errors() {
        let mut message = valid_csresp_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::BEARER_CONTEXT);

        let error = CreateSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::BEARER_CONTEXT));
    }

    #[test]
    fn missing_teid_in_header_errors() {
        let mut message = valid_csresp_message();
        message.header.teid = None;
        message.header.teid_flag = false;

        let error = CreateSessionResponse::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingTeid));
    }

    #[test]
    fn into_message_sets_correct_header() {
        let response = CreateSessionResponse::try_from(&valid_csresp_message()).unwrap();
        let message: GtpMessage = response.into();

        assert_eq!(message.header.message_type, 33);
        assert_eq!(message.header.teid_flag, true);
        assert_eq!(message.header.teid, Some(0x1000_0001));
    }

    #[test]
    fn into_message_omits_ambr_when_none() {
        let mut message = valid_csresp_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::AMBR);

        let response = CreateSessionResponse::try_from(&message).unwrap();

        assert!(response.ambr.is_none());

        let rebuilt: GtpMessage = response.into();

        assert!(
            !rebuilt
                .information_elements
                .iter()
                .any(|ie| ie.ie_type == ie_type::AMBR)
        );
    }

    #[test]
    fn full_roundtrip_through_wire_bytes() {
        let original = valid_csresp_message();
        let response = CreateSessionResponse::try_from(&original).unwrap();
        let rebuilt_message: GtpMessage = response.into();

        let mut wire = BytesMut::new();

        rebuilt_message.write(&mut wire);

        let reparsed_message = GtpMessage::parse(wire.freeze()).unwrap();

        let reparsed = CreateSessionResponse::try_from(&reparsed_message).unwrap();

        assert!(reparsed.cause.is_accepted());
        assert_eq!(reparsed.sender_fteid.teid_or_gre_key, 0xDEAD_BEEF);
        assert_eq!(reparsed.teid, 0x1000_0001);
    }
}
