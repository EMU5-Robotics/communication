use async_channel::{bounded, Sender};
mod listener;
mod mediator;
mod packets;
mod processing;
use crate::mediator::Mediator;

use crate::packets::{FromMain, RobotInfo, ToMain};

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
    pub fn try_init(robot_info: RobotInfo) -> Result<Mediator, SetLoggerError> {
        // channels are set to be rather small since the limit
        // should not be reached under normal operating conditions
        let (thread_tx, main_rx) = bounded::<ToMain>(100);
        let (main_tx, thread_rx) = bounded::<FromMain>(100);

        // set default log level if environment variable isn't set
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug");
        }

        log::set_boxed_logger(Box::new(Logger {
            sender: main_tx.clone(),
            local_logger: env_logger::Logger::from_default_env(),
        }))?;

        let mediator = Mediator {
            send: main_tx,
            recv: main_rx,
        };

        processing::spawn_processing_thread(robot_info, thread_tx, thread_rx);

        Ok(mediator)
    }
}
