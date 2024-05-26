use crate::packets::{FromMain, ToMain};
use async_channel::{Receiver, Sender};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Try Recv Error:\n{0}")]
    TryRecv(#[from] async_channel::TryRecvError),
    #[error("Try Send Error:\n{0}")]
    TrySend(#[from] async_channel::TrySendError<FromMain>),
}

pub struct Mediator {
    pub(crate) send: Sender<FromMain>,
    pub(crate) recv: Receiver<ToMain>,
}

impl Mediator {
    pub fn poll_events(&mut self) -> Result<Vec<ToMain>, Error> {
        let mut events = Vec::new();
        while let Ok(event) = self.recv.try_recv() {
            events.push(event);
        }
        Ok(events)
    }
    pub fn send_events(&mut self, events: Vec<FromMain>) -> Result<(), Error> {
        for event in events {
            self.send.try_send(event)?;
        }
        Ok(())
    }
    pub fn send_event(&mut self, event: FromMain) -> Result<(), Error> {
        Ok(self.send.try_send(event)?)
    }
}
