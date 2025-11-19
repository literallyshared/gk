use std::time::{Duration, Instant};

use shared::core::Core;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{networking_core::ConnectionId, realm::realm_state::RealmState};

pub enum RealmEvent {
    PlayerSpawned {
        connection_id: ConnectionId,
        account: String,
        current_map: String,
        entity_identifier: Uuid,
    },
}

pub enum RealmCommand {
    Stop,
    Tick { elapsed_time_ms: f32 },
}

#[derive(Default)]
pub struct RealmCore {}

impl RealmCore {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start(&mut self) -> Core<RealmCommand, RealmEvent> {
        info!("Starting Realm Core");
        let (tx, rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let state = RealmState::new(None, event_tx.clone());
        let tick_handle = RealmCore::tick_loop(tx.clone(), cancellation_token.clone());
        let handle = RealmCore::control_loop(rx, state, tick_handle, cancellation_token.clone());
        Core::new(tx.clone(), handle).with_events(event_rx)
    }

    fn control_loop(
        mut rx: mpsc::UnboundedReceiver<RealmCommand>,
        mut state: RealmState,
        tick_handle: JoinHandle<()>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Some(command) => match command {
                        RealmCommand::Stop => {
                            info!("Realm Core: Stopping");
                            cancellation_token.cancel();
                            let _ = tick_handle.await;
                            break;
                        }
                        RealmCommand::Tick { elapsed_time_ms } => {
                            state.tick(elapsed_time_ms);
                        }
                    },
                    None => {
                        warn!("RealmCommand closed channel");
                        break;
                    }
                }
            }
        })
    }

    fn tick_loop(
        tx: mpsc::UnboundedSender<RealmCommand>,
        cancellation_token: CancellationToken,
    ) -> JoinHandle<()> {
        let tick_length = Duration::from_millis(50);
        let mut last_tick = Instant::now();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        info!("Tick loop closing");
                        break;
                    },
                    else => {
                        let now = Instant::now();
                        if now - last_tick >= tick_length {
                            let elapsed_time_ms = (now - last_tick).as_millis() as f32;
                            last_tick = now;
                            if let Err(e) = tx.send(RealmCommand::Tick{ elapsed_time_ms }) {
                                warn!("Realm Command channel closed: [{e}]");
                                break;
                            }
                        }
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                }
            }
        })
    }
}
