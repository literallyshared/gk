use bevy_ecs::prelude::*;

use crate::ecs::components::{AttachedCamera, Position};

pub fn update_camera(
    query: Query<(&Position, &AttachedCamera)>,
) {
    if let Ok(query) = query.single() {

    } else {
        warn!("More than one entities have an attached camera");
    }
}
