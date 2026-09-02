use tokio_util::bytes::BytesMut;

use crate::error::GtpError;
use crate::ie::recovery::Recovery;
use crate::ie::types::ie_type;
use crate::{GtpMessage, Header, InformationElement};

#[derive(Debug, Clone)]
pub struct EchoRequest {
    pub recovery: Recovery,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for EchoRequest {
    type Error = GtpError;

    fn try_from(message: &GtpMessage) -> Result<Self, Self::Error> {
        let recovery_ie = message
            .information_elements
            .iter()
            .find(|ie| ie.ie_type == ie_type::RECOVERY)
            .ok_or(GtpError::MissingIe(ie_type::RECOVERY))?;
        let recovery = Recovery::parse(recovery_ie.value.clone())?;

        Ok(EchoRequest {
            recovery,
            sequence_number: message.header.sequence_number,
        })
    }
}

impl From<EchoRequest> for GtpMessage {
    fn from(echo_request: EchoRequest) -> Self {
        let mut v = BytesMut::new();
        echo_request.recovery.write(&mut v);

        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: false,
                message_type: 1,
                teid: None,
                sequence_number: echo_request.sequence_number,
            },
            information_elements: vec![InformationElement {
                ie_type: ie_type::RECOVERY,
                instance: 0,
                value: v.freeze(),
            }],
        }
    }
}
