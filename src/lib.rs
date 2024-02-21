use log::{Log, Metadata, Record};
use packet::{FromMediator, ToMediator};
use std::sync::mpsc::{self, RecvError, SendError};

pub mod client;
pub mod listener;
pub mod mediator;
pub mod packet;
pub mod path;
pub mod plot;

use listener::Listener;
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

// also handles plotting
pub struct Logger {
    sender: mpsc::Sender<FromMediator>,
    local_logger: env_logger::Logger,
}

impl Logger {
    pub fn init() -> Result<Mediator, log::SetLoggerError> {
        let (thread_tx, main_rx) = mpsc::channel();
        let (main_tx, thread_rx) = mpsc::channel();

        Listener::spawn(thread_tx, thread_rx);

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

        unsafe {
            crate::plot::PLOTTER = Some(main_tx.clone());
        };

        Ok(Mediator::new(main_tx, main_rx))
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
    use crate::packet::ToRobot;

    use super::*;

    #[test]
    fn logging() {
        let mut mediator = Logger::init().unwrap();

        // START LOGGING TESTS
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
        // END LOGGING TESTS

        // START PLOTTING TESTS
        let client = std::thread::spawn(|| {
            // create client after logs have been sent
            let mut client = Client::new("127.0.0.1:8733").unwrap();

            // give time for robot to respond
            std::thread::sleep(std::time::Duration::from_millis(150));

            let pkts = client.receive_data().unwrap();

            assert_eq!(pkts.len(), 4); // including disconnect log
        });

        std::thread::sleep(std::time::Duration::from_millis(10));

        // check logging
        plot!("test_plot", 5);
        plot!("test_plot_2", [3.2, 2.1, -2.3]);
        plot!("test_plot_2", [3.2, 2.7, -2.5]);
        plot!("test_plot_2", [1.2, 8.1, 2.3]);
        plot!("test_plot_2", [-3.1, 1.1, -2.7]);
        plot!("test_plot", 7.2);
        plot!("test_plot", 9.2);
        plot!("test_plot", -2.1);
        plot!("test_plot", 3);
        plot!("test_plot_3", [3., 1.]);

        for _ in 0..100 {
            if mediator.poll_events().is_err() {
                break;
            };

            // fancy busy loop simulation
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        client.join().unwrap();
    }
}
