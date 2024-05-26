use crate::packets::{ClientInfo, ToClient, ToRobot};
use async_channel::{bounded, Receiver, Sender};

#[derive(thiserror::Error, Debug)]
pub enum Error {}

pub struct Listener {
    send: Sender<ToRobot>,
    recv: Receiver<ToClient>,
}

impl Listener {
    pub fn new<A: std::net::ToSocketAddrs + Clone>(
        addr: A,
        client_info: ClientInfo,
    ) -> Result<Self, Error> {
        todo!()
    }
}
