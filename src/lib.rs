use log::{Log, Metadata, Record};
use packet::{FromMediator, ToMediator};
use std::{io::Write, net::*, sync::mpsc};

pub mod client;
pub mod mediator;
pub mod packet;

pub use mediator::Mediator;
pub use packet::SimpleLog;

pub struct Logger {
    sender: mpsc::Sender<FromMediator>,
}

impl Logger {
    pub fn init() -> Mediator {
        let (thread_tx, main_rx) = mpsc::channel();
        let (main_tx, thread_rx) = mpsc::channel();
        std::thread::spawn(move || {
            Self::processing_thread(thread_tx, thread_rx);
        });

        log::set_boxed_logger(Box::new(Self {
            sender: main_tx.clone(),
        }))
        .unwrap();

        Mediator::new(main_tx, main_rx)
    }

    // sends logs if needed to stream and update log index
    fn send_logs(logs: &[SimpleLog], stream: &mut Option<TcpStream>, last_idx: &mut usize) {
        if let Some(s) = stream {
            if s.write(SimpleLog::to_be_bytes(&logs[(*last_idx + 1)..]))
                .is_ok()
            {
                *last_idx = logs.len() - 1;
            }
        }
    }

    fn processing_thread(tx: mpsc::Sender<ToMediator>, rx: mpsc::Receiver<FromMediator>) {
        let mut last_log = 0;
        let mut logs = Vec::new();
        let mut stream = None;

        let listener = TcpListener::bind("127.0.0.1:8733")
            .expect("Failed to create TcpListener in logging thread.");
        listener
            .set_nonblocking(true)
            .expect("Failed to set TcpListener to nonblocking in logging thread.");

        while let Ok(from_mediator) = rx.recv() {
            // check for new a connection if there isn't a current connection (nonblocking)
            if stream.is_none() {
                if let Ok((s, _)) = listener.accept() {
                    stream = Some(s);
                }
            }

            match from_mediator {
                FromMediator::Log(log) => {
                    logs.push(log);
                    Self::send_logs(&logs, &mut stream, &mut last_log);
                }
            }
        }
        eprintln!("WARNING: processing thread has exited. The application can no longer communicate with clients.");
        eprintln!("This message should be unreachable as the sender should never be dropped.");
    }
}

impl Log for Logger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        self.sender.send(record.into()).unwrap();
    }
    fn flush(&self) {}
}
