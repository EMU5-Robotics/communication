use derive_new::new;

use std::sync::mpsc;

use crate::packet::{FromMediator, ToMediator};

#[derive(new)]
pub struct Mediator {
    send: mpsc::Sender<FromMediator>,
    recv: mpsc::Receiver<ToMediator>,
}
