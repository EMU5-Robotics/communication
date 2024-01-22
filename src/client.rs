use crate::packet::{self, ToClient, ToRobot};
use std::net::{TcpStream, ToSocketAddrs};

// NOTE: Client will be in a codebase that doesn't use the same logging framework
// that is in lib.rs so log::info will not be routed through Mediator

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error")]
    Io(#[from] std::io::Error),
    #[error("packet error")]
    Packet(#[from] packet::Error),
}

pub struct Client {
    stream: TcpStream,
}

impl Client {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr)?;
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
