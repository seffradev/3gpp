use tokio_util::bytes::{Bytes, BytesMut};

use crate::{
    Header, InformationElement,
    ie::{
        ambr::Ambr, apn::Apn, bearer_context::BearerContext, bearer_qos::BearerQos, cause::Cause,
        fteid::Fteid, imsi::Imsi,
    },
};

pub fn information_elements(ie_type: u8, instance: u8, value: &[u8]) -> InformationElement {
    InformationElement {
        ie_type,
        instance,
        value: Bytes::copy_from_slice(value),
    }
}

pub fn imsi_bytes(digits: &str) -> Vec<u8> {
    let mut buf = BytesMut::new();
    Imsi(digits.to_string()).write(&mut buf);
    buf.to_vec()
}

pub fn apn_bytes(labels: &str) -> Vec<u8> {
    let mut buf = BytesMut::new();
    Apn(labels.to_string()).write(&mut buf);
    buf.to_vec()
}

pub fn fteid_bytes(interface_type: u8, teid: u32, ipv4: [u8; 4]) -> Vec<u8> {
    let mut buf = BytesMut::new();
    Fteid {
        interface_type,
        teid_or_gre_key: teid,
        ipv4: Some(std::net::Ipv4Addr::from(ipv4)),
        ipv6: None,
    }
    .write(&mut buf);
    buf.to_vec()
}

pub fn ambr_bytes(up: u32, down: u32) -> Vec<u8> {
    let mut buf = BytesMut::new();
    Ambr {
        uplink_kbps: up,
        downlink_kbps: down,
    }
    .write(&mut buf);
    buf.to_vec()
}

pub fn cause_bytes(value: u8) -> Vec<u8> {
    let mut buf = BytesMut::new();
    Cause { value }.write(&mut buf);
    buf.to_vec()
}

pub fn rat_type_bytes(rat: u8) -> Vec<u8> {
    vec![rat]
}

pub fn pdn_type_bytes(t: u8) -> Vec<u8> {
    vec![t]
}

pub fn paa_v4_bytes(octets: [u8; 4]) -> Vec<u8> {
    let mut v = vec![1u8];
    v.extend_from_slice(&octets);
    v
}

pub fn selection_mode_bytes(mode: u8) -> Vec<u8> {
    vec![mode]
}

pub fn bearer_context_bytes(
    ebi: u8,
    cause: Option<u8>,
    fteid: Option<(u8, u32, [u8; 4])>,
    qos: Option<(u64, u64)>,
) -> Vec<u8> {
    let bc = BearerContext {
        ebi: Some(ebi),
        cause: cause.map(|v| Cause { value: v }),
        fteid: fteid.map(|(it, t, ip)| Fteid {
            interface_type: it,
            teid_or_gre_key: t,
            ipv4: Some(std::net::Ipv4Addr::from(ip)),
            ipv6: None,
        }),
        bearer_qos: qos.map(|(mbr_up, mbr_down)| BearerQos {
            max_bitrate_uplink_kbps: mbr_up,
            max_bitrate_downlink_kbps: mbr_down,
            ..Default::default()
        }),
    };
    bc.write().to_vec()
}

pub fn header(message_type: u8, teid: Option<u32>, seq: u32) -> Header {
    Header {
        version: 2,
        piggybacking_flag: false,
        teid_flag: teid.is_some(),
        message_type,
        teid,
        sequence_number: seq,
    }
}
