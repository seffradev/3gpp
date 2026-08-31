use crate::error::GtpError;
use crate::ie::bearer_qos::BearerQos;
use crate::ie::cause::Cause;
use crate::ie::fteid::Fteid;
use crate::ie::{InformationElement, types::ie_type};
use tokio_util::bytes::Buf;
use tokio_util::bytes::{Bytes, BytesMut};

#[derive(Debug, Clone, Default)]
pub struct BearerContext {
    pub ebi: Option<u8>,
    pub cause: Option<Cause>,
    pub fteid: Option<Fteid>,
    pub bearer_qos: Option<BearerQos>,
}

impl BearerContext {
    pub fn parse(value: Bytes) -> Result<Self, GtpError> {
        let mut ctx = BearerContext::default();
        let mut buf = value;

        while buf.remaining() > 0 {
            let ie = InformationElement::parse(&mut buf)?;
            match ie.ie_type {
                ie_type::EBI => ctx.ebi = ie.value.first().copied(),
                ie_type::CAUSE => ctx.cause = Some(Cause::parse(ie.value)?),
                ie_type::FTEID => ctx.fteid = Some(Fteid::parse(ie.value)?),
                ie_type::BEARER_QOS => ctx.bearer_qos = Some(BearerQos::parse(ie.value)?),
                _ => {}
            }
        }

        Ok(ctx)
    }

    pub fn write(&self) -> BytesMut {
        let mut inner = BytesMut::new();

        if let Some(ebi) = self.ebi {
            InformationElement {
                ie_type: ie_type::EBI,
                instance: 0,
                value: Bytes::copy_from_slice(&[ebi]),
            }
            .write(&mut inner);
        }

        if let Some(fteid) = &self.fteid {
            let mut v = BytesMut::new();

            fteid.write(&mut v);

            InformationElement {
                ie_type: ie_type::FTEID,
                instance: 2,
                value: v.freeze(),
            }
            .write(&mut inner);
        }

        if let Some(qos) = &self.bearer_qos {
            let mut v = BytesMut::new();

            qos.write(&mut v);

            InformationElement {
                ie_type: ie_type::BEARER_QOS,
                instance: 0,
                value: v.freeze(),
            }
            .write(&mut inner);
        }

        inner
    }
}
