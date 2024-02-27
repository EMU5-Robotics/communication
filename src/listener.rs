use crate::{
    packet::{self, FromMediator, ToClient, ToMediator, ToRobot},
    plot::PlotManager,
    Error,
};
use crossbeam_channel::{Receiver, Sender};
use std::{
    collections::VecDeque,
    net::{TcpListener, TcpStream},
};

pub(crate) struct Listener {
    tx: Sender<ToMediator>,
    rx: Receiver<FromMediator>,
    was_connected: bool,
    tcp: TcpListener,
    logs: Vec<ToClient>,
    last_log: usize,
    packet_buffer: VecDeque<FromMediator>,
    plot_manager: PlotManager,
}

impl Listener {
    pub(crate) fn spawn(tx: Sender<ToMediator>, rx: Receiver<FromMediator>) {
        // note that we don't get the thread handle to join later since
        // the listener thread can stall due to the static global logger
        // having a Sender<FromMediator> which can prevent the listener
        // loop from exiting
        std::thread::spawn(move || {
            if let Err(e) = Self::run(tx, rx) {
                // this will log using only env_logger
                log::error!("Listener thread errored with:\n{e}\nCommunication with clients is no longer possible. Is another instance running?");
            }
        });
    }
    fn new(tx: Sender<ToMediator>, rx: Receiver<FromMediator>) -> Result<Self, Error> {
        let tcp = TcpListener::bind("0.0.0.0:8733")?;
        tcp.set_nonblocking(true)?;

        Ok(Self {
            tx,
            rx,
            // it should be common for the client to start after the robot
            // so set was_connected=false so we don't warn about that
            was_connected: false,
            packet_buffer: VecDeque::new(),
            last_log: 0,
            logs: Vec::new(),
            tcp,
            plot_manager: PlotManager::default(),
        })
    }

    fn run(tx: Sender<ToMediator>, rx: Receiver<FromMediator>) -> Result<(), Error> {
        let mut s = Self::new(tx, rx)?;
        loop {
            match s.read_from_mediator() {
                Err(Error::Recv(_) | Error::Send(_)) => break,
                Err(_) if s.was_connected => {
                    s.was_connected = false;
                    log::warn!("Client disconnected since last packet.");
                }
                Ok(()) if !s.was_connected => {}
                _ => {}
            }
        }

        log::error!(
            "listener thread has exited. The application can no longer communicate with clients.\n\
             This message should be unreachable as the sender should never be dropped."
        );
        Ok(())
    }
    fn read_from_mediator(&mut self) -> Result<(), Error> {
        let mut stream = self.tcp.accept()?.0;
        stream.set_nonblocking(true)?;

        if !self.was_connected {
            log::info!("Client connected.");
        }
        self.was_connected = true;

        loop {
            let from_mediator = self.rx.recv()?;

            self.packet_buffer.push_back(from_mediator);

            while let Some(pkt) = self.packet_buffer.pop_front() {
                self.process_packet(&mut stream, pkt)?;
            }

            self.process_plot_points(&mut stream)?;
        }
    }
    fn process_plot_points(&mut self, stream: &mut TcpStream) -> Result<(), Error> {
        for buffer in self.plot_manager.buffers_to_send() {
            packet::send(stream, &ToClient::PointBuffer(buffer))?;
        }
        Ok(())
    }
    fn process_packet(&mut self, stream: &mut TcpStream, pkt: FromMediator) -> Result<(), Error> {
        match pkt {
            FromMediator::Log(log) => {
                self.logs.push(ToClient::Log(log));
                self.send_logs(stream)?;
            }
            FromMediator::Pong => packet::send(stream, &ToClient::Pong)?,
            FromMediator::Path(p) => packet::send(stream, &ToClient::Path(p))?,
            FromMediator::PollEvents => self.poll_tcp_events(stream)?,
            FromMediator::Point(p) => self.plot_manager.add_point(p),
        }
        Ok(())
    }
    // sends logs if needed to stream and update log index
    fn send_logs(&mut self, stream: &mut TcpStream) -> Result<(), packet::Error> {
        let unsent = &self.logs[self.last_log..];
        for log in unsent {
            packet::send(stream, log)?;
            self.last_log += 1;
        }
        Ok(())
    }
    fn poll_tcp_events(&mut self, stream: &mut TcpStream) -> Result<(), Error> {
        let mut pkt_fn = |stream: &mut _, pkt| -> Result<(), Error> {
            match pkt {
                ToRobot::Ping => self.tx.send(ToMediator::Ping)?,
                ToRobot::Path(p) => self.tx.send(ToMediator::Path(p))?,
                ToRobot::RequestLogs => {
                    self.send_logs(stream)?;
                }
            }
            Ok(())
        };
        packet::recieve_multiple(stream, &mut pkt_fn)
    }
}
