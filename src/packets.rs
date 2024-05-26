use log::Record;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// see https://serde.rs/remote-derive.html
// and https://docs.rs/log/latest/src/log/lib.rs.html#429-453
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
#[serde(remote = "log::Level")]
enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Log {
    source: String,
    #[serde(with = "Level")]
    level: log::Level,
    msg: String,
    timestamp: SystemTime,
}

impl From<&Record<'_>> for Log {
    fn from(rec: &Record<'_>) -> Self {
        Self {
            source: rec.target().to_owned(),
            level: rec.level(),
            msg: rec.args().to_string(),
            timestamp: SystemTime::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ClientInfo {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RobotInfo {
    pub name: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[repr(u8)]
pub enum ToClient {
    Log(Log) = 0,
    Pong = 1,
    Path = 2,
    PointBuffer = 3,
    Odometry = 4,
    RobotInfo(RobotInfo) = 5,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[repr(u8)]
pub enum ToRobot {
    ClientInfo(ClientInfo) = 0,
    Ping = 1,
    Path = 2,
    Pid = 3,
}

#[derive(Debug)]
pub enum ToMain {
    Path,
    Pid,
    Ping,
}

#[derive(Debug)]
pub enum FromMain {
    Log(Log),
    Path,
    Pong,
    PollEvents,
    Point,
    Odometry,
}

impl From<&Record<'_>> for FromMain {
    fn from(rec: &Record<'_>) -> Self {
        Self::Log(rec.into())
    }
}
