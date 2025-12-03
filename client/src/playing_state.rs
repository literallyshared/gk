use bevy_ecs::prelude::*;

use crate::{
    ecs::resources::{CurrentMap, Viewport},
    map::Map,
};

pub struct PlayingState {
    world: World,
    systems: Schedule,
    debug_systems: Option<Schedule>,
}

impl PlayingState {
    pub async fn new(map: String) -> Option<Self> {
        let mut world = World::new();

        let map = Map::load(&map).await?;
        world.insert_resource(CurrentMap(map));
        world.insert_resource(Viewport::default());

        let systems = Schedule::default();

        Some(Self {
            world,
            systems,
            debug_systems: None,
        })
    }

    pub fn tick(&mut self) {
        self.systems.run(&mut self.world);
        if let Some(systems) = &mut self.debug_systems {
            systems.run(&mut self.world);
        }
    }

    pub fn set_offline_mode(&mut self) {
        self.debug_systems = Some(Schedule::default());
        // TODO: populate the schedule
    }
}
