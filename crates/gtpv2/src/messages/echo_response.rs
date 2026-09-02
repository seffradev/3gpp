use tokio_util::bytes::BytesMut;

use crate::{
    GtpError, GtpMessage, Header, InformationElement,
    ie::{recovery::Recovery, types::ie_type},
};

#[derive(Debug, Clone)]
pub struct EchoResponse {
    pub recovery: Recovery,
    pub sequence_number: u32,
}

impl TryFrom<&GtpMessage> for EchoResponse {
    type Error = GtpError;

    fn try_from(msg: &GtpMessage) -> Result<Self, Self::Error> {
        let recovery_ie = msg
            .information_elements
            .iter()
            .find(|ie| ie.ie_type == ie_type::RECOVERY)
            .ok_or(GtpError::MissingIe(ie_type::RECOVERY))?;
        let recovery = Recovery::parse(recovery_ie.value.clone())?;

        Ok(EchoResponse {
            recovery,
            sequence_number: msg.header.sequence_number,
        })
    }
}

impl From<EchoResponse> for GtpMessage {
    fn from(echo_response: EchoResponse) -> Self {
        let mut v = BytesMut::new();
        echo_response.recovery.write(&mut v);

        GtpMessage {
            header: Header {
                version: 2,
                piggybacking_flag: false,
                teid_flag: false,
                message_type: 2,
                teid: None,
                sequence_number: echo_response.sequence_number,
            },
            information_elements: vec![InformationElement {
                ie_type: ie_type::RECOVERY,
                instance: 0,
                value: v.freeze(),
            }],
        }
    }
}
