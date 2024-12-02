use crate::plot;
use log::Record;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

// see https://serde.rs/remote-derive.html
// and https://docs.rs/log/latest/src/log/lib.rs.html#429-453
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
#[serde(remote = "log::Level")]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct Log {
    pub source: String,
    #[serde(with = "Level")]
    pub level: log::Level,
    pub msg: String,
    pub timestamp: SystemTime,
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

impl ClientInfo {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RobotInfo {
    pub name: String,
}

impl RobotInfo {
    pub fn new<T: Into<String>>(name: T) -> Self {
        Self { name: name.into() }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[repr(u8)]
pub enum ToClient {
    Log(Log) = 0,
    Pong = 1,
    Path = 2,
    PointBuffer(String, String, plot::Buffer) = 3,
    Odometry(String, [f64; 2], f64) = 4,
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
}

#[derive(Debug)]
pub enum FromMain {
    Log(Log),
    Path,
    // plot name, subplot name, point
    Point(String, String, crate::plot::Point),
    Odometry([f64; 2], f64),
}

impl From<&Record<'_>> for FromMain {
    fn from(rec: &Record<'_>) -> Self {
        Self::Log(rec.into())
    }
}
