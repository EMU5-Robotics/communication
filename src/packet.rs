use std::time::SystemTime;
use std::{
    convert::Into,
    io::{Read, Write},
    net::TcpStream,
};

use log::{Level, Record};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("bincode serialise/deserialise error")]
    Bincode(#[from] bincode::Error),
    #[error("read/write error")]
    Io(#[from] std::io::Error),
    #[error("unknown error")]
    Other(String),
}

// see https://serde.rs/remote-derive.html
// and https://docs.rs/log/latest/src/log/lib.rs.html#429-453
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Copy)]
#[serde(remote = "Level")]
enum LevelDef {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct SimpleLog {
    #[serde(with = "LevelDef")]
    level: Level,
    msg: String,
    target: String,
    timestamp: SystemTime,
}

impl From<&Record<'_>> for SimpleLog {
    fn from(rec: &Record<'_>) -> Self {
        Self {
            level: rec.level(),
            msg: rec.args().to_string(),
            target: rec.target().to_owned(),
            timestamp: SystemTime::now(),
        }
    }
}

// TCP PACKETS
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum ToClient {
    Log(SimpleLog),
    Pong,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum ToRobot {
    RequestLogs,
    Ping,
}

// THREAD PACKETS
#[derive(Debug)]
pub enum ToMediator {
    Ping,
}

#[derive(Debug)]
pub enum FromMediator {
    Log(SimpleLog),
    Pong,
    PollEvents,
}

impl From<&Record<'_>> for FromMediator {
    fn from(rec: &Record<'_>) -> Self {
        Self::Log(rec.into())
    }
}

pub(crate) fn send(stream: &mut std::net::TcpStream, pkt: &impl Serialize) -> Result<(), Error> {
    let data = bincode::serialize(pkt)?;
    let len = u32::try_from(data.len())
        .map_err(|_| Error::Other(String::from("Packet length greater then 2^32-1 bytes?!?")))?;
    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&data)?;
    Ok(())
}

pub(crate) fn recieve_multiple<
    T: DeserializeOwned,
    E: std::error::Error + From<Error>,
    F: FnMut(&mut TcpStream, T) -> Result<(), E>,
>(
    stream: &mut TcpStream,
    pkt_fn: &mut F,
) -> Result<(), E> {
    let mut len_buf = [0u8; 4];
    stream.set_nonblocking(true).map_err(Into::into)?;
    loop {
        let pkt: T = {
            match stream.read_exact(&mut len_buf) {
                Ok(()) => {}
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(e) => return Err(Error::Io(e).into()),
            }

            let len = u32::from_be_bytes(len_buf);
            let len = usize::try_from(len)
                .map_err(|_| Error::Other(String::from("Packet larger then pointer size?!?")))?;
            let mut buf = vec![0u8; len];
            stream.set_nonblocking(false).map_err(Into::into)?;
            stream.read_exact(&mut buf).map_err(Into::into)?;
            stream.set_nonblocking(true).map_err(Into::into)?;

            bincode::deserialize(&buf).map_err(Into::into)?
        };

        pkt_fn(stream, pkt)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize() {
        let test_val = ToClient::Log(SimpleLog {
            level: Level::Info,
            msg: String::from("test"),
            target: String::from("test2"),
            timestamp: std::time::SystemTime::now(),
        });
        let data = bincode::serialize(&test_val).unwrap();
        assert_eq!(test_val, bincode::deserialize(&data).unwrap());
    }
}
