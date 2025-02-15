use crate::packet::{self, ToClient, ToRobot};
use std::net::{AddrParseError, TcpStream, ToSocketAddrs};

// NOTE: Client will be in a codebase that doesn't use the same logging framework
// that is in lib.rs so log::info will not be routed through Mediator

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error:\n{0}")]
    Io(#[from] std::io::Error),
    #[error("packet error:\n{0}")]
    Packet(#[from] packet::Error),
    #[error("invalid ip:\n{0}")]
    AddrParse(#[from] AddrParseError),
}
pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new<A: ToSocketAddrs + Clone>(addr: A) -> Result<Self, Error> {
        let stream;
        loop {
            match TcpStream::connect(addr.clone()) {
                Ok(s) => stream = s,
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => continue,
                Err(e) => return Err(e)?,
            }
            break;
        }
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
    fn blocking() {
        let thread = std::thread::spawn(|| {
            // listener will never be on port 9999
            // so this will fail to connect
            Client::new("127.0.0.1:9999").unwrap();
        });
        std::thread::sleep(std::time::Duration::from_millis(15));
        assert!(!thread.is_finished());
    }
}
