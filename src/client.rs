use crate::packet::{self, ToClient, ToRobot};
use crate::SimpleLog;
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
    pub logs: Vec<SimpleLog>,
}

impl Client {
    pub fn new<A: ToSocketAddrs>(addr: A) -> Result<Self, Error> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        let mut a = Self {
            stream,
            logs: Vec::new(),
        };
        a.send_request(&ToRobot::RequestLogs)?;
        Ok(a)
    }

    pub fn receive_data(&mut self) -> Result<(), packet::Error> {
        // pkt_fn should be a parameter in the future
        let mut pkt_fn = |_: &mut _, pkt| {
            match pkt {
                ToClient::Log(l) => self.logs.push(l),
                ToClient::Pong => println!("recieved pong (client)"),
            };
            Ok(())
        };
        packet::recieve_multiple(&mut self.stream, &mut pkt_fn)
    }

    pub fn send_request(&mut self, pkt: &ToRobot) -> Result<(), packet::Error> {
        packet::send(&mut self.stream, &pkt)
    }
}
