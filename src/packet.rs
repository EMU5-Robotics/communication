use log::{Level, Record};
use serde::{Deserialize, Serialize};

// see https://serde.rs/remote-derive.html
// and https://docs.rs/log/latest/src/log/lib.rs.html#429-453
#[derive(Serialize, Deserialize)]
#[serde(remote = "Level")]
enum LevelDef {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Serialize, Deserialize)]
pub struct SimpleLog {
    #[serde(with = "LevelDef")]
    level: Level,
    msg: String,
    target: String,
}

impl From<&Record<'_>> for SimpleLog {
    fn from(rec: &Record<'_>) -> Self {
        Self {
            level: rec.level(),
            msg: rec.args().to_string(),
            target: rec.target().to_owned(),
        }
    }
}

// TCP PACKETS
#[derive(Serialize, Deserialize)]
pub enum ToClient {
    Log(SimpleLog),
}

#[derive(Serialize, Deserialize)]
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
