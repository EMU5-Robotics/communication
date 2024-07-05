use std::collections::VecDeque;

use crate::packets::{ClientInfo, ToClient, ToRobot};
use async_channel::{bounded, Receiver, Sender};
use async_executor::LocalExecutor;
use async_net::TcpStream;
use futures_lite::prelude::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    //#[error("bincode serialise/deserialise error:\n{0}")]
    //Bincode(#[from] bincode::Error),
    #[error("IO Error:\n{0}")]
    Io(#[from] std::io::Error),
    #[error("Async Receive Error:\n{0}")]
    AsyncRecv(#[from] async_channel::RecvError),
    #[error("Async Send Error:\n{0}")]
    AsyncSend(#[from] async_channel::SendError<String>),
    #[error("Other Error:\n{0}")]
    Other(String),
}

// the goal of the client listener is communicate with a thread it spawns
// which manages the connection between the client and the robot without
// blocking and also can regenerate the connection when needed.
pub struct ClientListener {
    send: Sender<ToRobot>,
    recv: Receiver<ToClient>,
}

impl ClientListener {
    pub fn new(addr: std::net::SocketAddrV4, client_info: ClientInfo) -> Self {
        let (thread_tx, listener_rx) = bounded::<ToClient>(100);
        let (listener_tx, thread_rx) = bounded::<ToRobot>(100);

        let _ = std::thread::spawn(move || {
            let ex = LocalExecutor::new();
            futures_lite::future::block_on(ex.run(async {
                log::info!("ClinetListener initialised");
                loop {
                    let Ok(mut write) = TcpStream::connect(addr).await else {
                        continue;
                    };

                    log::info!("ClinetListener connected to {addr}");

                    let read = write.clone();

                    if let Err(e) = tcp_handshake(&mut write, &client_info).await {
                        log::warn!("tcp_handshake got: {e}");
                    }

                    log::info!("ClinetListener handshake finished");

                    if let Err(e) = futures_lite::future::try_zip(
                        ex.spawn(write_tcp_packets(write, thread_rx.clone())),
                        ex.spawn(read_tcp_packets(read, thread_tx.clone())),
                    )
                    .await
                    {
                        log::warn!("handle_tcp got: {e}");
                    }
                }
            }));
        });

        Self {
            send: listener_tx,
            recv: listener_rx,
        }
    }
    pub fn get_packets(&self) -> Vec<ToClient> {
        let mut packets = Vec::new();
        loop {
            match self.recv.try_recv() {
                Ok(packet) => packets.push(packet),
                Err(async_channel::TryRecvError::Closed) => {
                    log::error!("ClientListener Receiver closed. This is a bug.")
                }
                Err(async_channel::TryRecvError::Empty) => break,
            }
        }
        packets
    }
    pub fn send_packets(&self, packets: &mut VecDeque<ToRobot>) {
        while let Some(v) = packets.pop_front() {
            match self.send.try_send(v) {
                Ok(_) => continue,
                Err(async_channel::TrySendError::Full(v)) => {
                    packets.push_front(v);
                    return;
                }
                Err(async_channel::TrySendError::Closed(v)) => {
                    log::error!("ClientListener Sender closed. This is a bug.");
                    packets.push_front(v);
                    return;
                }
            }
        }
    }
}

async fn write_tcp_packets(
    mut stream: TcpStream,
    recv_proc: Receiver<ToRobot>,
) -> Result<(), Error> {
    loop {
        let pkt = recv_proc.recv().await?;

        // todo: map error properly here
        let data = rmp_serde::encode::to_vec(&pkt).unwrap();

        let len = u32::try_from(data.len()).map_err(|_| {
            Error::Other(String::from("Packet length greater then 2^32-1 bytes?!?"))
        })?;

        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&data).await?;
    }
}

async fn read_tcp_packets(mut stream: TcpStream, send_proc: Sender<ToClient>) -> Result<(), Error> {
    loop {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;

        let len = u32::from_be_bytes(len_buf);
        let len = usize::try_from(len)
            .map_err(|_| Error::Other(String::from("Packet larger then pointer size?!?")))?;

        let mut data = vec![0u8; len];
        stream.read_exact(&mut data).await?;

        // todo: map error properly here
        let pkt = rmp_serde::decode::from_slice(&data).unwrap();

        send_proc
            .send(pkt)
            .await
            .map_err(|e| async_channel::SendError(e.to_string()))?;
    }
}

async fn tcp_handshake(stream: &mut TcpStream, client_info: &ClientInfo) -> Result<(), Error> {
    // todo: map error properly here
    let data = rmp_serde::encode::to_vec(&ToRobot::ClientInfo(client_info.clone())).unwrap();

    let len = u32::try_from(data.len())
        .map_err(|_| Error::Other(String::from("Packet length greater then 2^32-1 bytes?!?")))?;

    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&data).await?;

    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf);
    let len = usize::try_from(len)
        .map_err(|_| Error::Other(String::from("Packet larger then pointer size?!?")))?;

    let mut data = vec![0u8; len];
    stream.read_exact(&mut data).await?;

    let pkt = rmp_serde::decode::from_slice(&data);

    if let Ok(ToClient::RobotInfo(client_info)) = pkt {
        log::info!("Connection established with robot - {}", client_info.name);
    }
    Ok(())
}
