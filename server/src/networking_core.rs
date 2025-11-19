use std::{net::{SocketAddr}, sync::{atomic::{AtomicU64, Ordering}, Arc}};

use dashmap::DashMap;
use shared::{client_messages::{ClientOpcode, FromClient}, core::Core, frame::{MessageFrame, MsgCodec, Opcode}, server_messages::{FromServer, ServerOpcode}};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::{codec::{FramedRead, FramedWrite}, sync::CancellationToken};
use futures::{SinkExt, StreamExt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId(u64);

impl ConnectionId {
    fn next(counter: &AtomicU64) -> Self {
        let id = counter.fetch_add(1, Ordering::Relaxed) + 1;
        Self(id)
    }
}

pub enum NetEvent {
    NewConnection {
        connection_id: ConnectionId,
    },
    Disconnected {
        connection_id: ConnectionId,
    },
    IncomingMessage {
        connection_id: ConnectionId,
        message: FromClient,
    },
}

pub enum NetCommand {
    Stop,
    Send {
        connection: ConnectionId,
        message: FromServer,
    },
    Broadcast {
        message: FromServer,
    },
}

#[derive(Debug)]
pub struct ConnectionRecord {
    pub address: SocketAddr,
    outgoing: tokio::sync::mpsc::UnboundedSender<MessageFrame<ServerOpcode>>,
}

type Connections = Arc<DashMap<ConnectionId, ConnectionRecord>>;

#[derive(Default)]
pub struct NetworkingCore {
    address: Option<String>,
    connections: Connections,
    id_counter: Arc<AtomicU64>,
}

impl NetworkingCore {
    pub fn new(
    ) -> Self {
        Self {
            address: None,
            connections: Arc::default(),
            id_counter: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn start(
        &mut self,
        address: String,
        port: u32
    ) -> Core<NetCommand, NetEvent> {
        info!("Starting Networking Core");
        let cancellation_token = CancellationToken::new();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
        self.address = Some(format!("{address}:{port}"));

        let listener_handle = NetworkingCore::listener_loop(
            self.address.clone().unwrap(),
            event_tx.clone(),
            self.connections.clone(),
            self.id_counter.clone(),
            cancellation_token.clone()
        );

        let control_handle = NetworkingCore::control_loop(
            command_rx,
            self.connections.clone(),
            listener_handle,
            cancellation_token
        );
        Core::new(command_tx.clone(), control_handle)
            .with_events(event_rx)
    }

    fn control_loop(
        mut rx: mpsc::UnboundedReceiver<NetCommand>,
        connections: Connections,
        listener_handle: JoinHandle<()>,
        token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(command) => match command {
                        NetCommand::Stop => {
                            info!("NetworkingCore: Stopping");
                            NetworkingCore::stop(token, listener_handle, connections).await;
                            break;
                        }
                        NetCommand::Send { connection, message } => {
                            if let Some(connection) = connections.get(&connection) {
                                match message.serialize() {
                                    Ok(frame) => {
                                        let _ = connection.outgoing.send(frame);
                                    }
                                    Err(e) => warn!("Failed to encode message [{message:?}] [{e}]"),
                                }
                            }
                        },
                        NetCommand::Broadcast { message } => {
                            match message.serialize() {
                                Ok(frame) => {
                                    for connection in connections.iter() {
                                        let _ = connection.outgoing.send(frame.clone());
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to encode message [{message:?}] [{e}]");
                                    continue;
                                }
                            };
                        },
                    }
                    None => {
                        error!("NetworkingCore: Channel closed. Stopping");
                        NetworkingCore::stop(token, listener_handle, connections).await;
                        break;
                    },
                }
            }
        })
    }

    fn listener_loop(
        address: String,
        tx: mpsc::UnboundedSender<NetEvent>,
        connections: Connections,
        id_counter: Arc<AtomicU64>,
        token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let listener = match TcpListener::bind(&address).await {
                Ok(listener) => listener,
                Err(err) => {
                    error!("Failed to bind to address [{address}]: {err}");
                    return;
                }
            };
            info!("Listening on: [{address}]");
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("Listener closed by server");
                        break;
                    },
                    connection = listener.accept() => {
                        let (socket, remote_addr) = match connection {
                            Ok(pair) => pair,
                            Err(err) => {
                                error!("Error accepting connection: {err}");
                                continue;
                            }
                        };
                        let connection_id = ConnectionId::next(&id_counter);
                        let (out_tx, out_rx) = mpsc::unbounded_channel::<MessageFrame<ServerOpcode>>();
                        connections.insert(
                            connection_id,
                            ConnectionRecord {
                                address: remote_addr,
                                outgoing: out_tx.clone(),
                            },
                        );
                        let connection_map = connections.clone();
                        tokio::spawn(NetworkingCore::handle_connection(
                            socket,
                            tx.clone(),
                            connection_id,
                            remote_addr,
                            connection_map,
                            out_rx,
                            token.clone(),
                        ));
                    }
                }
            }
        })
    }

    async fn handle_connection(
        socket: TcpStream,
        tx: mpsc::UnboundedSender<NetEvent>,
        connection_id: ConnectionId,
        remote_addr: SocketAddr,
        connections: Connections,
        mut outgoing: mpsc::UnboundedReceiver<MessageFrame<ServerOpcode>>,
        cancel: CancellationToken,
    ) {
        info!("Incoming connection {connection_id:?} from [{remote_addr}]");
        let (read_half, write_half) = socket.into_split();
        let mut reader = FramedRead::new(read_half, MsgCodec::<ClientOpcode>::default());
        let mut writer = FramedWrite::new(write_half, MsgCodec::<ServerOpcode>::default());
        if let Err(e) = tx.send(NetEvent::NewConnection { connection_id }) {
            warn!("Event channel closed: [{e}]");
            return;
        }

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    info!("Connection closed by server for [{remote_addr}]");
                    break;
                },
                inbound = reader.next() => {
                    match inbound {
                        Some(Ok(frame)) => {
                            if let Ok(message) = FromClient::deserialize(frame.opcode, &frame.payload) {
                                if let Err(e) = tx.send(NetEvent::IncomingMessage{ connection_id, message }) {
                                    warn!("Failed to send message to [{connection_id:?}]: [{e}]");
                                    break;
                                }
                            } else {
                                warn!(
                                    "Failed to decode frame from {remote_addr} (id={connection_id:?}): opcode=0x{:#06x}, len={}",
                                    frame.opcode.into_raw(),
                                    frame.payload.len()
                                );
                            }
                        }
                        Some(Err(err)) => {
                            error!("Failed to decode frame from [{remote_addr}]: {err}");
                            break;
                        }
                        None => {
                            info!("Connection closed by client [{remote_addr}]");
                            break;
                        }
                    }
                }
                Some(payload) = outgoing.recv() => {
                    if let Err(err) = writer.send(payload).await {
                        error!("Failed to send outbound frame to [{remote_addr}]: {err}");
                        break;
                    }
                }
            }
        }

        connections.remove(&connection_id);
        let _ = tx.send(NetEvent::Disconnected { connection_id });
        info!("Connection removed for [{remote_addr}] (id={connection_id:?})");
    }

    async fn stop(
        cancel: CancellationToken,
        listener_handle: JoinHandle<()>,
        _connections: Connections,
    ) {
        cancel.cancel();
        let _ = listener_handle.await;
    }
}
