use std::sync::Arc;

use dashmap::{DashMap, mapref::one::RefMut};
use shared::core::Core;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{session::SessionState, networking_core::{ConnectionId, NetCommand, NetEvent}, realm::realm_core::{RealmCommand, RealmEvent}};

pub enum NexusCommand {
    Stop,
    RegisterConnection {
        connection_id: ConnectionId,
    },
    UnregisterConnection {
        connection_id: ConnectionId,
    },
    RegisterPlayer {
        connection_id: ConnectionId,
        entity_identifier: Uuid,
        account: String,
        current_map: String,
    },
    SetDisplayName {
        connection_id: ConnectionId,
        display_name: String,
    },
    SetCurrentMap {
        entity_identifier: Uuid,
        new_map: String,
    },
}

#[derive(Default)]
pub struct NexusCore {
    sessions: Arc<DashMap<ConnectionId, SessionState>>,
    identifiers: Arc<DashMap<Uuid, ConnectionId>>,
}

impl NexusCore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::default(),
            identifiers: Arc::default(),
        }
    }

    pub fn start(
        &mut self,
        mut networking_core: Core<NetCommand, NetEvent>,
        mut realm_core: Core<RealmCommand, RealmEvent>,
    ) -> Core<NexusCommand> {
        info!("Starting Nexus Core");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let net_event_handle = NexusCore::net_event_loop(
            tx.clone(),
            networking_core.take_rx().unwrap(),
            cancellation_token.clone(),
        );
        let realm_event_handle = NexusCore::realm_event_loop(
            tx.clone(),
            realm_core.take_rx().unwrap(),
            cancellation_token.clone(),
        );
        let handle = NexusCore::control_loop(
            rx,
            self.sessions.clone(),
            self.identifiers.clone(),
            cancellation_token.clone(),
            networking_core,
            net_event_handle,
            realm_core,
            realm_event_handle,
        );
        Core::new(tx, handle)
    }

    fn control_loop(
        mut rx: tokio::sync::mpsc::UnboundedReceiver<NexusCommand>,
        sessions: Arc<DashMap<ConnectionId, SessionState>>,
        identifiers: Arc<DashMap<Uuid, ConnectionId>>,
        cancellation_token: CancellationToken,
        networking_core: Core<NetCommand, NetEvent>,
        net_event_handle: JoinHandle<()>,
        realm_core: Core<RealmCommand, RealmEvent>,
        realm_event_handle: JoinHandle<()>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(command) => match command {
                        NexusCommand::Stop => {
                            info!("NexusCore: Stopping");
                            cancellation_token.cancel();
                            let _ = networking_core.stop(Some(NetCommand::Stop));
                            let _ = net_event_handle.await;
                            let _ = realm_core.stop(Some(RealmCommand::Stop));
                            let _ = realm_event_handle.await;
                            break;
                        }
                        NexusCommand::RegisterConnection { connection_id } => {
                            sessions.insert(connection_id, SessionState::AwaitingLogin);
                        }
                        NexusCommand::UnregisterConnection { connection_id } => {
                            info!("Unregistering connection [{connection_id:?}]");
                            if let Some((_, session)) = sessions.remove(&connection_id) {
                                if let SessionState::Playing {
                                    entity_identifier,
                                    account,
                                    display_name,
                                    current_map: _,
                                } = session
                                {
                                    info!(
                                        "Removing registered entity for [{account}][{display_name}]"
                                    );
                                    identifiers.remove(&entity_identifier);
                                }
                            }
                        }
                        NexusCommand::RegisterPlayer {
                            entity_identifier,
                            account,
                            current_map,
                            connection_id,
                        } => {
                            if let Some(mut session) = sessions.get_mut(&connection_id) {
                                info!("Registered player [{account}]");
                                *session = SessionState::Playing {
                                    entity_identifier,
                                    account,
                                    display_name: "unknown".to_string(),
                                    current_map,
                                };
                                identifiers.insert(entity_identifier, connection_id);
                            } else {
                                warn!(
                                    "No registered session for player [{account}]"
                                );
                            }
                        }
                        NexusCommand::SetDisplayName { connection_id, display_name } => {
                            if let Some(mut session) = sessions.get_mut(&connection_id) {
                                session.set_display_name(display_name);
                            }
                        }
                        NexusCommand::SetCurrentMap { entity_identifier, new_map } => {
                            match NexusCore::get_session_for_identifier(
                                &sessions,
                                &identifiers,
                                &entity_identifier,
                            ) {
                                Some(mut session) => {
                                    session.set_current_map(new_map);
                                }
                                None => {
                                    warn!("Unable to find session for entity [{entity_identifier}]");
                                }
                            }
                        }
                    },
                    None => {
                        warn!("NexusCore: Closed channel");
                        break;
                    }
                }
            }
        })
    }

    fn net_event_loop(
        tx: tokio::sync::mpsc::UnboundedSender<NexusCommand>,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<NetEvent>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        info!("NetEvent listener closing");
                        break;
                    },
                    incoming = rx.recv() => {
                        match incoming {
                            Some(event) => match event {
                                NetEvent::NewConnection { connection_id } => {
                                    if let Err(_) = tx.send(NexusCommand::RegisterConnection { connection_id }) {
                                        break;
                                    }
                                }
                                NetEvent::Disconnected { connection_id } => {
                                    if let Err(_) = tx.send(NexusCommand::UnregisterConnection { connection_id }) {
                                        break;
                                    }
                                }
                                NetEvent::IncomingMessage { connection_id, message } => {
                                    // TODO: dispatch messages
                                    debug!("Incoming message from [{connection_id:?}] [{message:?}]");
                                }
                            }
                            None => {
                                warn!("Net event channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    fn realm_event_loop(
        tx: tokio::sync::mpsc::UnboundedSender<NexusCommand>,
        mut rx: tokio::sync::mpsc::UnboundedReceiver<RealmEvent>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        info!("NetEvent listener closing");
                        break;
                    },
                    incoming = rx.recv() => {
                        match incoming {
                            Some(event) => match event {
                                RealmEvent::PlayerSpawned { connection_id, account, current_map, entity_identifier } => {
                                    if let Err(e) = tx.send(NexusCommand::RegisterPlayer { connection_id, account, current_map, entity_identifier }) {
                                        warn!("Failed to send NexusCommand: [{e}]");
                                        break;
                                    }
                                }
                            }
                            None => {
                                warn!("Realm event channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    fn get_session_for_identifier<'a>(
        sessions: &'a DashMap<ConnectionId, SessionState>,
        identifiers: &DashMap<Uuid, ConnectionId>,
        identifier: &Uuid,
    ) -> Option<RefMut<'a, ConnectionId, SessionState>> {
        let connection_id = identifiers.get(identifier)?;
        sessions.get_mut(connection_id.value())
    }
}
