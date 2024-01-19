use crate::packet::ToClient;
use crate::SimpleLog;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::SystemTime;

pub struct Client {
    stream: TcpStream,
    logs: Vec<(SystemTime, SimpleLog)>,
}

impl Client {
    pub fn new<A: ToSocketAddrs>(addr: A) -> std::io::Result<Self> {
        Ok(Self {
            stream: TcpStream::connect(addr)?,
            logs: Vec::new(),
        })
    }

    fn receive_data(&mut self) {
        let _: ToClient = bincode::deserialize_from(&self.stream).unwrap();
        todo!()
    }
}
