use crossbeam_channel::{Receiver, Sender, TrySendError};

use crate::packet::{self, FromMediator, ToMediator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error:\n{0}")]
    Io(#[from] std::io::Error),
    #[error("packet error:\n{0}")]
    Packet(#[from] packet::Error),
    #[error("send error:\n{0}")]
    Send(#[from] TrySendError<FromMediator>),
}

pub struct Mediator {
    send: Sender<FromMediator>,
    recv: Receiver<ToMediator>,
}

impl Mediator {
    pub(crate) fn new(send: Sender<FromMediator>, recv: Receiver<ToMediator>) -> Self {
        Self { send, recv }
    }
    pub fn poll_events(&mut self) -> Result<Vec<ToMediator>, Error> {
        self.send_event(FromMediator::PollEvents)?;

        let mut events = Vec::new();
        while let Ok(event) = self.recv.try_recv() {
            events.push(event);
        }
        Ok(events)
    }
    pub fn send_events(&mut self, events: Vec<FromMediator>) -> Result<(), Error> {
        for event in events {
            self.send.try_send(event)?;
        }
        Ok(())
    }
    pub fn send_event(&mut self, event: FromMediator) -> Result<(), Error> {
        Ok(self.send.try_send(event)?)
    }
}
