use bevy_ecs::{schedule::Schedule, world::World};
use tokio::sync::mpsc;

use crate::realm::{realm_core::RealmEvent, ecs::resources::{ElapsedTimeMs, RealmEventSender}};

pub struct RealmState {
    pub world: World,
    pub systems: Schedule,
}

impl RealmState {
    pub fn new(_asset_path: Option<&str>, event_tx: mpsc::UnboundedSender<RealmEvent>) -> Self {
        let mut world = World::new();

        world.insert_resource(RealmEventSender(event_tx));
        world.insert_resource(ElapsedTimeMs(0.0));

        let mut _systems= Schedule::default();

        Self {
            world,
            systems: _systems,
        }
    }

    pub fn tick(&mut self, elapsed_time_ms: f32) {
        self.world
            .get_resource_mut::<ElapsedTimeMs>()
            .unwrap()
            .0 = elapsed_time_ms;
        self.systems.run(&mut self.world);
    }
}
