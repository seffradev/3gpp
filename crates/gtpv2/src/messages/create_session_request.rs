use tokio_util::bytes::BytesMut;

use crate::GtpMessage;
use crate::error::GtpError;
use crate::header::Header;
use crate::ie::InformationElement;
use crate::ie::paa::Paa;
use crate::ie::pdn_type::PdnType;
use crate::ie::rat_type::RatType;
use crate::ie::selection_mode::SelectionMode;
use crate::ie::types::message_type;
use crate::ie::{
    ambr::Ambr, apn::Apn, bearer_context::BearerContext, fteid::Fteid, imsi::Imsi, types::ie_type,
};

#[derive(Debug, Clone)]
pub struct CreateSessionRequest {
    pub imsi: Imsi,
    pub sender_fteid: Fteid,
    pub apn: Apn,
    pub ambr: Ambr,
    pub rat_type: RatType,
    pub pdn_type: PdnType,
    pub paa: Paa,
    pub selection_mode: SelectionMode,
    pub bearer_context: BearerContext,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for CreateSessionRequest {
    type Error = GtpError;

    fn try_from(message: &GtpMessage) -> Result<Self, Self::Error> {
        if message.header.teid != Some(0) {
            return Err(GtpError::InvalidTeid {
                expected: 0,
                actual: message.header.teid,
            });
        }

        let find = |t: u8, inst: u8| {
            message
                .information_elements
                .iter()
                .find(|ie| ie.ie_type == t && ie.instance == inst)
        };

        let imsi = Imsi::parse(
            find(ie_type::IMSI, 0)
                .ok_or(GtpError::MissingIe(ie_type::IMSI))?
                .value
                .clone(),
        )?;
        let sender_fteid = Fteid::parse(
            find(ie_type::FTEID, 0)
                .ok_or(GtpError::MissingIe(ie_type::FTEID))?
                .value
                .clone(),
        )?;
        let apn = Apn::parse(
            find(ie_type::APN, 0)
                .ok_or(GtpError::MissingIe(ie_type::APN))?
                .value
                .clone(),
        )?;
        let ambr = Ambr::parse(
            find(ie_type::AMBR, 0)
                .ok_or(GtpError::MissingIe(ie_type::AMBR))?
                .value
                .clone(),
        )?;
        let rat_type = RatType::parse(
            find(ie_type::RAT_TYPE, 0)
                .ok_or(GtpError::MissingIe(ie_type::RAT_TYPE))?
                .value
                .clone(),
        )?;
        let pdn_type = PdnType::parse(
            find(ie_type::PDN_TYPE, 0)
                .ok_or(GtpError::MissingIe(ie_type::PDN_TYPE))?
                .value
                .clone(),
        )?;
        let paa = Paa::parse(
            find(ie_type::PAA, 0)
                .ok_or(GtpError::MissingIe(ie_type::PAA))?
                .value
                .clone(),
        )?;
        let selection_mode = SelectionMode::parse(
            find(ie_type::SELECTION_MODE, 0)
                .ok_or(GtpError::MissingIe(ie_type::SELECTION_MODE))?
                .value
                .clone(),
        )?;
        let bearer_ie =
            find(ie_type::BEARER_CONTEXT, 0).ok_or(GtpError::MissingIe(ie_type::BEARER_CONTEXT))?;
        let bearer_context = BearerContext::parse(bearer_ie.value.clone())?;

        Ok(CreateSessionRequest {
            imsi,
            sender_fteid,
            apn,
            ambr,
            rat_type,
            pdn_type,
            paa,
            selection_mode,
            bearer_context,
            sequence_number: message.header.sequence_number,
        })
    }
}

impl From<CreateSessionRequest> for GtpMessage {
    fn from(create_session_request: CreateSessionRequest) -> Self {
        let mut ies = Vec::new();

        let mut v = BytesMut::new();
        create_session_request.imsi.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::IMSI,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.sender_fteid.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::FTEID,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.apn.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::APN,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.ambr.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::AMBR,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.rat_type.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::RAT_TYPE,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.pdn_type.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::PDN_TYPE,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.paa.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::PAA,
            instance: 0,
            value: v.freeze(),
        });

        let mut v = BytesMut::new();
        create_session_request.selection_mode.write(&mut v);
        ies.push(InformationElement {
            ie_type: ie_type::SELECTION_MODE,
            instance: 0,
            value: v.freeze(),
        });

        let bearer_value = create_session_request.bearer_context.write().freeze();
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
                message_type: message_type::CREATE_SESSION_REQUEST,
                teid: Some(0),
                sequence_number: create_session_request.sequence_number,
            },
            information_elements: ies,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        GtpError, GtpMessage,
        ie::{
            paa::Paa,
            pdn_type::PdnType,
            rat_type::RatType,
            selection_mode::SelectionMode,
            types::{ie_type, message_type},
        },
        messages::CreateSessionRequest,
        test_helpers::{
            ambr_bytes, apn_bytes, bearer_context_bytes, fteid_bytes, header, imsi_bytes,
            information_elements, paa_v4_bytes, pdn_type_bytes, rat_type_bytes,
            selection_mode_bytes,
        },
    };
    use tokio_util::bytes::{Bytes, BytesMut};

    fn valid_create_session_request_message() -> GtpMessage {
        GtpMessage {
            header: header(message_type::CREATE_SESSION_REQUEST, Some(0), 100),
            information_elements: vec![
                information_elements(ie_type::IMSI, 0, &imsi_bytes("001010123456789")),
                information_elements(
                    ie_type::FTEID,
                    0,
                    &fteid_bytes(7, 0xAABB_CCDD, [10, 0, 0, 1]),
                ),
                information_elements(ie_type::APN, 0, &apn_bytes("internet")),
                information_elements(ie_type::AMBR, 0, &ambr_bytes(50_000, 100_000)),
                information_elements(ie_type::RAT_TYPE, 0, &rat_type_bytes(6)),
                information_elements(ie_type::PDN_TYPE, 0, &pdn_type_bytes(1)),
                information_elements(ie_type::PAA, 0, &paa_v4_bytes([0, 0, 0, 0])),
                information_elements(ie_type::SELECTION_MODE, 0, &selection_mode_bytes(0)),
                information_elements(
                    ie_type::BEARER_CONTEXT,
                    0,
                    &bearer_context_bytes(5, None, None, Some((50_000, 100_000))),
                ),
            ],
        }
    }

    #[test]
    fn parses_valid_message() {
        let message = valid_create_session_request_message();
        let create_session_request =
            CreateSessionRequest::try_from(&message).expect("should parse");

        assert_eq!(create_session_request.imsi.0, "001010123456789");
        assert_eq!(
            create_session_request.sender_fteid.teid_or_gre_key,
            0xAABB_CCDD
        );
        assert_eq!(
            create_session_request.sender_fteid.ipv4,
            Some("10.0.0.1".parse().unwrap())
        );
        assert_eq!(create_session_request.apn.0, "internet");
        assert_eq!(create_session_request.ambr.uplink_kbps, 50_000);
        assert_eq!(create_session_request.ambr.downlink_kbps, 100_000);
        assert_eq!(create_session_request.rat_type, RatType::Eutran);
        assert_eq!(create_session_request.pdn_type, PdnType::Ipv4);
        assert!(
            matches!(create_session_request.paa, Paa::V4 { address } if address == std::net::Ipv4Addr::UNSPECIFIED)
        );
        assert_eq!(
            create_session_request.selection_mode,
            SelectionMode::MsOrNetworkProvidedSubscriptionVerified
        );
        assert_eq!(create_session_request.bearer_context.ebi, Some(5));
        assert_eq!(create_session_request.sequence_number, 100);
    }

    #[test]
    fn missing_imsi_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::IMSI);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::IMSI));
    }

    #[test]
    fn missing_fteid_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::FTEID);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::FTEID));
    }

    #[test]
    fn missing_apn_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::APN);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::APN));
    }

    #[test]
    fn missing_ambr_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::AMBR);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::AMBR));
    }

    #[test]
    fn missing_bearer_context_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::BEARER_CONTEXT);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::BEARER_CONTEXT));
    }

    #[test]
    fn wrong_instance_is_treated_as_missing() {
        let mut message = valid_create_session_request_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::IMSI {
                ie.instance = 1;
            }
        }

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::IMSI));
    }

    #[test]
    fn into_message_sets_correct_header() {
        let create_session_request =
            CreateSessionRequest::try_from(&valid_create_session_request_message()).unwrap();

        let message: GtpMessage = create_session_request.into();

        assert_eq!(
            message.header.message_type,
            message_type::CREATE_SESSION_REQUEST
        );
        assert_eq!(message.header.teid_flag, true);
        assert_eq!(message.header.teid, Some(0), "CSR has zero as session TEID");
        assert_eq!(message.header.version, 2);
    }

    #[test]
    fn into_message_contains_all_ies() {
        let create_session_request =
            CreateSessionRequest::try_from(&valid_create_session_request_message()).unwrap();

        let message: GtpMessage = create_session_request.into();

        let types: Vec<u8> = message
            .information_elements
            .iter()
            .map(|ie| ie.ie_type)
            .collect();

        assert!(types.contains(&ie_type::IMSI));
        assert!(types.contains(&ie_type::FTEID));
        assert!(types.contains(&ie_type::APN));
        assert!(types.contains(&ie_type::AMBR));
        assert!(types.contains(&ie_type::BEARER_CONTEXT));
    }

    #[test]
    fn missing_rat_type_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::RAT_TYPE);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::RAT_TYPE));
    }

    #[test]
    fn invalid_rat_type_value_errors() {
        let mut message = valid_create_session_request_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::RAT_TYPE {
                ie.value = Bytes::copy_from_slice(&[255]); // not a defined RAT type
            }
        }

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::InvalidRatType(255)));
    }

    #[test]
    fn into_message_includes_rat_type() {
        let create_session_request =
            CreateSessionRequest::try_from(&valid_create_session_request_message()).unwrap();

        let message: GtpMessage = create_session_request.into();

        let rat_ie = message
            .information_elements
            .iter()
            .find(|ie| ie.ie_type == ie_type::RAT_TYPE)
            .expect("RAT Type IE should be present");

        assert_eq!(&rat_ie.value[..], &[6]);
    }

    #[test]
    fn full_roundtrip_through_wire_bytes() {
        let original = valid_create_session_request_message();
        let create_session_request = CreateSessionRequest::try_from(&original).unwrap();
        let rebuilt_message: GtpMessage = create_session_request.into();

        let mut wire = BytesMut::new();

        rebuilt_message.write(&mut wire);

        let reparsed_message = GtpMessage::parse(wire.freeze()).unwrap();

        let reparsed_create_session_request =
            CreateSessionRequest::try_from(&reparsed_message).unwrap();

        assert_eq!(reparsed_create_session_request.imsi.0, "001010123456789");
        assert_eq!(
            reparsed_create_session_request.sender_fteid.teid_or_gre_key,
            0xAABB_CCDD
        );
        assert_eq!(reparsed_create_session_request.apn.0, "internet");
        assert_eq!(reparsed_create_session_request.ambr.uplink_kbps, 50_000);
        assert_eq!(reparsed_create_session_request.rat_type, RatType::Eutran);
        assert_eq!(reparsed_create_session_request.bearer_context.ebi, Some(5));
    }

    #[test]
    fn nonzero_teid_in_csr_errors() {
        let mut message = valid_create_session_request_message();

        message.header.teid = Some(0x1234);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(
            error,
            GtpError::InvalidTeid {
                expected: 0,
                actual: Some(0x1234)
            }
        ));
    }

    #[test]
    fn missing_teid_flag_in_csr_errors() {
        let mut message = valid_create_session_request_message();

        message.header.teid = None;
        message.header.teid_flag = false;

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(
            error,
            GtpError::InvalidTeid {
                expected: 0,
                actual: None
            }
        ));
    }

    #[test]
    fn missing_pdn_type_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::PDN_TYPE);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::PDN_TYPE));
    }

    #[test]
    fn missing_paa_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::PAA);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::PAA));
    }

    #[test]
    fn missing_selection_mode_errors() {
        let mut message = valid_create_session_request_message();

        message
            .information_elements
            .retain(|ie| ie.ie_type != ie_type::SELECTION_MODE);

        let error = CreateSessionRequest::try_from(&message).unwrap_err();

        assert!(matches!(error, GtpError::MissingIe(t) if t == ie_type::SELECTION_MODE));
    }

    #[test]
    fn paa_ipv6_roundtrip() {
        let paa = Paa::V6 {
            prefix_length: 64,
            address: "2001:db8::1".parse().unwrap(),
        };

        let mut buf = BytesMut::new();

        paa.write(&mut buf);

        let parsed = Paa::parse(buf.freeze()).unwrap();

        match parsed {
            Paa::V6 {
                prefix_length: prefix_len,
                address,
            } => {
                assert_eq!(prefix_len, 64);
                assert_eq!(
                    address,
                    "2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap()
                );
            }
            _ => panic!("expected V6 variant"),
        }
    }

    #[test]
    fn paa_v4v6_roundtrip() {
        let paa = Paa::V4V6 {
            prefix_length: 64,
            v6_address: "2001:db8::1".parse().unwrap(),
            v4_address: "192.168.1.1".parse().unwrap(),
        };

        let mut buf = BytesMut::new();

        paa.write(&mut buf);

        let parsed = Paa::parse(buf.freeze()).unwrap();

        match parsed {
            Paa::V4V6 {
                v4_address: v4_addr,
                v6_address: v6_addr,
                ..
            } => {
                assert_eq!(
                    v4_addr,
                    "192.168.1.1".parse::<std::net::Ipv4Addr>().unwrap()
                );
                assert_eq!(
                    v6_addr,
                    "2001:db8::1".parse::<std::net::Ipv6Addr>().unwrap()
                );
            }
            _ => panic!("expected V4V6 variant"),
        }
    }

    #[test]
    fn invalid_pdn_type_value_errors() {
        let mut message = valid_create_session_request_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::PDN_TYPE {
                ie.value = Bytes::copy_from_slice(&[0]);
            }
        }

        let error = CreateSessionRequest::try_from(&message).unwrap_err();
        assert!(matches!(error, GtpError::InvalidPdnType(0)));
    }

    #[test]
    fn invalid_selection_mode_value_errors() {
        let mut message = valid_create_session_request_message();

        for ie in message.information_elements.iter_mut() {
            if ie.ie_type == ie_type::SELECTION_MODE {
                ie.value = Bytes::copy_from_slice(&[3]);
            }
        }

        let error = CreateSessionRequest::try_from(&message).unwrap_err();
        assert!(matches!(error, GtpError::InvalidSelectionMode(3)));
    }

    #[test]
    fn parses_bearer_qos_correctly() {
        let message = valid_create_session_request_message();
        let create_session_request = CreateSessionRequest::try_from(&message).unwrap();

        let qos = create_session_request
            .bearer_context
            .bearer_qos
            .expect("QoS should be present");

        assert_eq!(qos.qci, 9);
        assert_eq!(qos.priority_level, 8);
        assert_eq!(qos.max_bitrate_uplink_kbps, 50_000);
        assert_eq!(qos.max_bitrate_downlink_kbps, 100_000);
    }
}
