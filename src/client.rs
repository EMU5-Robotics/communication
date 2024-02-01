use crate::packet::{self, ToClient, ToRobot};
use std::net::{AddrParseError, TcpStream};
use std::time::Duration;

// NOTE: Client will be in a codebase that doesn't use the same logging framework
// that is in lib.rs so log::info will not be routed through Mediator

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("packet error")]
    Packet(#[from] packet::Error),
    #[error("invalid ip")]
    AddrParse(#[from] AddrParseError),
}

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new(addr: &str) -> Result<Self, Error> {
        let stream = TcpStream::connect_timeout(&addr.parse()?, Duration::from_millis(10))?;
        stream.set_nonblocking(true)?;
        let mut a = Self { stream };
        a.send_request(&ToRobot::RequestLogs)?;
        Ok(a)
    }

    pub fn receive_data(&mut self) -> Result<Vec<ToClient>, packet::Error> {
        let mut pkts = Vec::new();
        let mut pkt_fn = |_: &mut _, pkt| -> Result<(), packet::Error> {
            pkts.push(pkt);
            Ok(())
        };
        packet::recieve_multiple(&mut self.stream, &mut pkt_fn)?;
        Ok(pkts)
    }

    pub fn send_request(&mut self, pkt: &ToRobot) -> Result<(), packet::Error> {
        packet::send(&mut self.stream, &pkt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonblocking() {
        let thread = std::thread::spawn(|| {
            // listener will never be on port 9999
            // so this will fail to connect
            let _ = Client::new("127.0.0.1:9999");
        });
        std::thread::sleep(Duration::from_millis(15));
        assert!(thread.is_finished());
    }
}
