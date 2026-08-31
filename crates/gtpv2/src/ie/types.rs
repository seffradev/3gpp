pub mod message_type {
    pub const CREATE_SESSION_REQUEST: u8 = 32;
    pub const DELETE_SESSION_REQUEST: u8 = 36;
    pub const DELETE_SESSION_RESPONSE: u8 = 37;
}

pub mod ie_type {
    pub const IMSI: u8 = 1;
    pub const CAUSE: u8 = 2;
    pub const RECOVERY: u8 = 3;
    pub const APN: u8 = 71;
    pub const AMBR: u8 = 72;
    pub const EBI: u8 = 73;
    pub const BEARER_CONTEXT: u8 = 93;
    pub const FTEID: u8 = 87;
    pub const RAT_TYPE: u8 = 82;
    pub const PDN_TYPE: u8 = 99;
    pub const PAA: u8 = 79;
    pub const SELECTION_MODE: u8 = 128;
    pub const BEARER_QOS: u8 = 80;
}
