use bevy_ecs::prelude::*;
use tokio::sync::mpsc;

use crate::realm::realm_core::RealmEvent;

#[derive(Resource)]
pub struct RealmEventSender(pub mpsc::UnboundedSender<RealmEvent>);

#[derive(Resource)]
pub struct ElapsedTimeMs(pub f32);
