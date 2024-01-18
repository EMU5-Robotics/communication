use log::Record;

pub struct SimpleLog;

impl SimpleLog {
    pub fn to_be_bytes(data: &[Self]) -> &[u8] {
        todo!();
    }
    pub fn from_be_bytes(data: &[u8]) -> Vec<Self> {
        todo!();
    }
}

impl From<&Record<'_>> for SimpleLog {
    fn from(val: &Record<'_>) -> Self {
        todo!()
    }
}

// TCP PACKETS
pub enum ToClient {
    Log(SimpleLog),
}

pub enum ToRobot {
    RequestLogs,
    SayHi,
}

// THREAD PACKETS
pub enum ToMediator {}

pub enum FromMediator {
    Log(SimpleLog),
}

impl From<&Record<'_>> for FromMediator {
    fn from(val: &Record<'_>) -> Self {
        todo!();
    }
}
