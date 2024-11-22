use async_channel::{bounded, Receiver, Sender};
use async_executor::LocalExecutor;
use async_net::{TcpListener, TcpStream};
use futures_lite::prelude::*;
use std::rc::Rc;

use crate::packets::*;
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

// The job of the processing thread is to accept and process data from the main thread
// and also accept and process data from tcp connection threads of which the
// proccessing thread spawns.
pub(crate) fn spawn_processing_thread(
    robot_info: RobotInfo,
    send_main: Sender<ToMain>,
    recv_main_handler: Receiver<FromMain>,
) {
    let _ = std::thread::spawn(|| -> Result<(), Error> {
        let ex = Rc::new(LocalExecutor::new());

        // channel for sending between (incoming) TCP handler and main client packet processing loop
        // small bounded channel, if the limit is reached something went seriously wrong
        let (send_tcp_handler, recv_tcp_handler) = bounded::<ToClient>(100);
        // channel for sending between (outgoing) TCP handler and main client packet processing loop
        let (send_client_handler, recv_client_handler) = bounded::<ToRobot>(100);

        let incoming_main = ex.spawn::<Result<(), Error>>(handle_incoming_main(
            recv_main_handler,
            send_tcp_handler.clone(),
        ));
        let tcp = ex.spawn::<Result<(), Error>>(handle_tcp(
            robot_info,
            send_client_handler,
            recv_tcp_handler,
            ex.clone(),
        ));
        let incoming_client = ex.spawn::<Result<(), Error>>(handle_incoming_client(
            send_main,
            send_tcp_handler,
            recv_client_handler,
        ));

        futures_lite::future::block_on(ex.run(futures_lite::future::try_zip(
            futures_lite::future::try_zip(incoming_main, tcp),
            incoming_client,
        )))?;
        Ok(())
    });
}

async fn handle_incoming_main(
    recv_main: Receiver<FromMain>,
    to_tcp: Sender<ToClient>,
) -> Result<(), Error> {
    loop {
        let pkt = recv_main.recv().await?;
        match pkt {
            FromMain::Log(log) => to_tcp
                .send(ToClient::Log(log))
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            FromMain::Path => to_tcp
                .send(ToClient::Path)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            FromMain::Odometry => to_tcp
                .send(ToClient::Odometry)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            FromMain::Point => to_tcp
                .send(ToClient::PointBuffer)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
        }
    }
}

async fn handle_incoming_client(
    send_main: Sender<ToMain>,
    send_tcp_handler: Sender<ToClient>,
    recv_client: Receiver<ToRobot>,
) -> Result<(), Error> {
    loop {
        let pkt = recv_client.recv().await?;
        match pkt {
            ToRobot::Ping => send_tcp_handler
                .send(ToClient::Pong)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            ToRobot::Pid => send_main
                .send(ToMain::Pid)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            ToRobot::Path => send_main
                .send(ToMain::Path)
                .await
                .map_err(|e| async_channel::SendError(e.to_string()))?,
            ToRobot::ClientInfo(c) => log::info!("Client connected: {c:?}"),
        }
    }
}

// TCP:
// 1. establish connection
// 2. perform handshake
// 3. loop { read + write }
async fn handle_tcp(
    robot_info: RobotInfo,
    send_processing: Sender<ToRobot>,
    recv_processing: Receiver<ToClient>,
    ex: Rc<LocalExecutor<'_>>,
) -> Result<(), Error> {
    log::info!("trying to bind on 0.0.0.0:8733");
    let listener = TcpListener::bind("0.0.0.0:8733").await?;
    let mut incoming = listener.incoming();
    log::info!("listening on 0.0.0.0:8733");

    loop {
        // handle concurrent streams
        while let Some(stream) = incoming.next().await {
            let Ok(mut write) = stream else { continue };
            let read = write.clone();

            let _ = tcp_handshake(&mut write, &robot_info).await;

            // detach read write since we don't care about
            // handing errors since they aren't really recoverable
            // and if connection closes the listener should open it again
            if let Err(e) = futures_lite::future::try_zip(
                ex.spawn(write_tcp_packets(write, recv_processing.clone())),
                ex.spawn(read_tcp_packets(read, send_processing.clone())),
            )
            .await
            {
                log::warn!("handle_tcp got: {e}");
            }
        }
    }
}

async fn read_tcp_packets(mut stream: TcpStream, send_proc: Sender<ToRobot>) -> Result<(), Error> {
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

async fn write_tcp_packets(
    mut stream: TcpStream,
    recv_proc: Receiver<ToClient>,
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
async fn tcp_handshake(stream: &mut TcpStream, robot_info: &RobotInfo) -> Result<(), Error> {
    // todo: map error properly here
    let data = rmp_serde::encode::to_vec(&ToClient::RobotInfo(robot_info.clone())).unwrap();

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

    if let Ok(ToRobot::ClientInfo(client_info)) = pkt {
        log::info!("Connection established with client - {}", client_info.name);
    }
    Ok(())
}
