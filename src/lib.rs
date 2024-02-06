use log::{Log, Metadata, Record};
use packet::{FromMediator, ToMediator, ToRobot};
use std::{
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, RecvError, SendError},
};

pub mod client;
pub mod mediator;
pub mod packet;
pub mod path;

pub use mediator::Mediator;
pub use packet::{SimpleLog, ToClient};

#[derive(thiserror::Error, Debug)]
enum Error {
    #[error("IO error:\n{0}")]
    Io(#[from] std::io::Error),
    #[error("error with packet reading/writing:\n{0}")]
    Packet(#[from] packet::Error),
    #[error("error reading from main thread:\n{0}")]
    Recv(#[from] RecvError),
    #[error("error sending to main thread:\n{0}")]
    Send(#[from] SendError<ToMediator>),
    #[error("mediator error:\n{0}")]
    Mediator(#[from] mediator::Error),
}

pub struct Logger {
    sender: mpsc::Sender<FromMediator>,
    local_logger: env_logger::Logger,
}

impl Logger {
    pub fn init() -> Result<Mediator, log::SetLoggerError> {
        let (thread_tx, main_rx) = mpsc::channel();
        let (main_tx, thread_rx) = mpsc::channel();

        // note that we don't get the thread handle to join later since
        // the listener thread can stall due to the static global logger
        // having a Sender<FromMediator> which can prevent the listener
        // loop from exiting
        std::thread::spawn(move || {
            if let Err(e) = Self::listener_thread(&thread_tx, &thread_rx) {
                // this will log using only env_logger
                log::error!("Listener thread errored with:\n{e}\nCommunication with clients is no longer possible. Is another instance running?");
            }
        });

        // set default log level
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug,client::coprocessor::serial=info");
        }

        let local_logger = env_logger::Logger::from_default_env();

        let filter = local_logger.filter();

        log::set_boxed_logger(Box::new(Self {
            sender: main_tx.clone(),
            local_logger,
        }))
        .map(|()| log::set_max_level(filter))?;

        Ok(Mediator::new(main_tx, main_rx))
    }
    // sends logs if needed to stream and update log index
    fn send_logs(
        logs: &[ToClient],
        stream: &mut TcpStream,
        last_idx: &mut usize,
    ) -> Result<(), packet::Error> {
        let unsent = &logs[*last_idx..];
        for log in unsent {
            packet::send(stream, log)?;
            *last_idx += 1;
        }
        Ok(())
    }
    fn read_from_mediator(
        tx: &mpsc::Sender<ToMediator>,
        rx: &mpsc::Receiver<FromMediator>,
        listener: &TcpListener,
        last_log: &mut usize,
        was_connected: &mut bool,
        logs: &mut Vec<ToClient>,
        packet_buffer: &mut std::collections::VecDeque<FromMediator>,
    ) -> Result<(), Error> {
        let mut stream = listener.accept()?.0;
        stream.set_nonblocking(true)?;

        loop {
            let from_mediator = rx.recv()?;

            packet_buffer.push_back(from_mediator);

            *was_connected = true;
            while let Some(pkt) = packet_buffer.pop_front() {
                Self::process_packet(tx, &mut stream, pkt, logs, last_log)?;
            }
        }
    }
    fn listener_thread(
        tx: &mpsc::Sender<ToMediator>,
        rx: &mpsc::Receiver<FromMediator>,
    ) -> Result<(), Error> {
        let listener = TcpListener::bind("0.0.0.0:8733")?;
        listener.set_nonblocking(true)?;

        // it should be common for the client to start after the
        // robot so don't warn about that
        let mut was_connected = false;

        let mut packet_buffer = std::collections::VecDeque::new();
        let mut last_log = 0;
        let mut logs = Vec::new();

        loop {
            match Self::read_from_mediator(
                tx,
                rx,
                &listener,
                &mut last_log,
                &mut was_connected,
                &mut logs,
                &mut packet_buffer,
            ) {
                Err(Error::Recv(_) | Error::Send(_)) => break,
                Err(_) => {
                    if was_connected {
                        was_connected = false;
                        log::warn!("Client disconnected since last packet.");
                    }
                }
                _ => {}
            }
        }

        log::error!(
            "listener thread has exited. The application can no longer communicate with clients.\n\
             This message should be unreachable as the sender should never be dropped."
        );
        Ok(())
    }
    fn process_packet(
        tx: &mpsc::Sender<ToMediator>,
        stream: &mut TcpStream,
        pkt: FromMediator,
        logs: &mut Vec<ToClient>,
        last_log: &mut usize,
    ) -> Result<(), Error> {
        match pkt {
            FromMediator::Log(log) => {
                logs.push(ToClient::Log(log));
                Self::send_logs(logs, stream, last_log)?;
            }
            FromMediator::Pong => packet::send(stream, &ToClient::Pong)?,
            FromMediator::Path(p) => packet::send(stream, &ToClient::Path(p))?,
            FromMediator::PollEvents => Self::poll_tcp_events(tx, stream, logs, last_log)?,
        }
        Ok(())
    }
    fn poll_tcp_events(
        tx: &mpsc::Sender<ToMediator>,
        stream: &mut TcpStream,
        logs: &mut [ToClient],
        last_log: &mut usize,
    ) -> Result<(), Error> {
        let mut pkt_fn = |stream: &mut _, pkt| -> Result<(), Error> {
            match pkt {
                ToRobot::Ping => tx.send(ToMediator::Ping)?,
                ToRobot::Path(p) => tx.send(ToMediator::Path(p))?,
                ToRobot::RequestLogs => {
                    Self::send_logs(logs, stream, last_log)?;
                }
            }
            Ok(())
        };
        packet::recieve_multiple(stream, &mut pkt_fn)
    }
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        let _ = self.sender.send(record.into());
        self.local_logger.log(record);
    }
    fn flush(&self) {
        self.local_logger.flush();
    }
}

#[cfg(test)]
mod tests {
    use crate::client::Client;

    use super::*;

    #[test]
    fn logging() {
        let mut mediator = Logger::init().unwrap();

        let client = std::thread::spawn(|| {
            // to make sure that the TcpListener is bound to port
            std::thread::sleep(std::time::Duration::from_millis(20));

            // create client after logs have been sent
            let mut client = Client::new("127.0.0.1:8733").unwrap();

            // give time for robot to respond
            // also see https://github.com/EMU5-Robotics/communication/issues/3
            std::thread::sleep(std::time::Duration::from_millis(50));

            let pkts = client.receive_data().unwrap();

            assert_eq!(pkts.len(), 5);

            client.send_request(&ToRobot::Ping).unwrap();

            // give ping chain time to complete
            std::thread::sleep(std::time::Duration::from_millis(50));

            // client should of received only Pong packet
            let pkts = client.receive_data().unwrap();
            assert_eq!(pkts.len(), 1);
            assert_eq!(pkts[0], ToClient::Pong);
        });

        // check logging
        log::info!("These");
        log::debug!("are some");
        log::info!("example");
        log::warn!("logs");
        log::error!("");

        for _ in 0..100 {
            let Ok(events) = mediator.poll_events() else {
                break;
            };

            for event in events {
                match event {
                    ToMediator::Ping => mediator.send_event(FromMediator::Pong).unwrap(),
                    _ => {}
                }
            }
            // fancy busy loop simulation
            std::thread::sleep(std::time::Duration::from_millis(3));
        }

        client.join().unwrap();
    }
}
