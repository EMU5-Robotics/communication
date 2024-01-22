use derive_new::new;

use std::sync::mpsc::{self, SendError};

use crate::packet::{self, FromMediator, ToMediator};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("IO error")]
    Io(#[from] std::io::Error),
    #[error("packet error")]
    Packet(#[from] packet::Error),
    #[error("send error")]
    Send(#[from] SendError<FromMediator>),
}

#[derive(new)]
pub struct Mediator {
    send: mpsc::Sender<FromMediator>,
    recv: mpsc::Receiver<ToMediator>,
}

impl Mediator {
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
            self.send.send(event)?;
        }
        Ok(())
    }
    pub fn send_event(&mut self, event: FromMediator) -> Result<(), Error> {
        Ok(self.send.send(event)?)
    }
}
