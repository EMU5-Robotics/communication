use async_channel::{bounded, Sender};
pub mod listener;
mod mediator;
pub mod packets;
pub mod plot;
mod processing;
use crate::mediator::Mediator;

use crate::packets::{FromMain, ToMain};

pub use crate::{
    listener::ClientListener,
    packets::{ClientInfo, RobotInfo, ToClient, ToRobot},
    plot::*,
};

pub struct Logger {
    sender: Sender<FromMain>,
    local_logger: env_logger::Logger,
}

use log::{Log, Metadata, Record, SetLoggerError};

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, rec: &Record) {
        self.local_logger.log(rec);
        let _ = self.sender.try_send(rec.into());
    }
    fn flush(&self) {
        self.local_logger.flush();
    }
}

impl Logger {
    pub fn try_init(
        robot_info: RobotInfo,
        enable_logging: bool,
    ) -> Result<Mediator, SetLoggerError> {
        // channels are set to be rather small since the limit
        // should not be reached under normal operating conditions
        let (thread_tx, main_rx) = bounded::<ToMain>(100);
        let (main_tx, thread_rx) = bounded::<FromMain>(100);

        // set default log level if environment variable isn't set
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug");
        }
        let local_logger = env_logger::Logger::from_default_env();
        log::set_max_level(local_logger.filter());
        if enable_logging {
            log::set_boxed_logger(Box::new(Logger {
                sender: main_tx.clone(),
                local_logger,
            }))?;

            unsafe {
                crate::plot::PLOTTER = Some(main_tx.clone());
            };
        }

        let mediator = Mediator {
            send: main_tx,
            recv: main_rx,
        };

        processing::spawn_processing_thread(robot_info, thread_tx, thread_rx);

        Ok(mediator)
    }
}

#[cfg(test)]
mod tests {
    use crate::{listener::ClientListener, packets::*};

    use super::*;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    #[test]
    fn establish_connection() {
        init();
        let mut mediator = Logger::try_init(RobotInfo::new("test1"), false).unwrap();

        let a = std::thread::spawn(|| {
            let client = ClientListener::new(
                std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(0, 0, 0, 0), 8733),
                ClientInfo::new("testclient1"),
            );
            client.send_packets(&mut vec![ToRobot::Ping].into());
            std::thread::sleep(std::time::Duration::from_millis(100));
            let pkts = client.get_packets();
            assert_eq!(pkts, vec![ToClient::Pong]);
        });

        let start = std::time::Instant::now();

        loop {
            let _ = mediator.poll_events();

            if start.elapsed() > std::time::Duration::from_millis(200) {
                break;
            }
        }
        a.join().unwrap();
    }
}
